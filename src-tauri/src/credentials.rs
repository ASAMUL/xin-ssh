use std::{
    fs,
    path::{Path, PathBuf},
};

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use ring::rand::{SecureRandom, SystemRandom};
use tauri::{AppHandle, Manager};

use crate::models::{ReadCredentialRequest, RemoveCredentialRequest, SaveCredentialRequest};
use crate::util::error_message;
use crate::workspace::{
    active_workspace_dir, read_json_file, write_json_file, CURRENT_SCHEMA_VERSION,
};

const DEVICE_CREDENTIAL_KEY_FILE: &str = "device-credential-key.json";
const CREDENTIALS_FILE: &str = "credentials.json";
const DEVICE_CREDENTIAL_KEY_BYTES: usize = 32;
const CREDENTIAL_NONCE_BYTES: usize = 12;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceCredentialKeyFile {
    schema_version: u16,
    key: String,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsFile {
    schema_version: u16,
    credentials: Vec<EncryptedCredentialRecord>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncryptedCredentialRecord {
    credential_ref: String,
    kind: String,
    nonce: String,
    ciphertext: String,
}

#[tauri::command]
pub async fn save_credential(app: AppHandle, request: SaveCredentialRequest) -> Result<(), String> {
    validate_credential_ref(&request.credential_ref)?;
    let workspace = active_workspace_dir(&app)?;
    let key = device_credential_key(&app)?;
    let mut credentials_file = read_credentials_file(&workspace)?;

    save_secret_if_present(
        &mut credentials_file,
        &key,
        &request.credential_ref,
        "password",
        request.secret.password.as_deref(),
    )?;
    save_secret_if_present(
        &mut credentials_file,
        &key,
        &request.credential_ref,
        "passphrase",
        request.secret.passphrase.as_deref(),
    )?;

    write_credentials_file(&workspace, &credentials_file)
}

#[tauri::command]
pub async fn read_credential(app: AppHandle, request: ReadCredentialRequest) -> Result<Option<String>, String> {
    validate_credential_ref(&request.credential_ref)?;
    validate_credential_kind(&request.kind)?;

    let workspace = active_workspace_dir(&app)?;
    let credentials_file = read_credentials_file(&workspace)?;
    let Some(record) = credentials_file
        .credentials
        .iter()
        .find(|record| record.credential_ref == request.credential_ref && record.kind == request.kind)
    else {
        return Ok(None);
    };

    let key = device_credential_key(&app)?;
    decrypt_credential_record(&key, record).map(Some)
}

#[tauri::command]
pub async fn remove_credential(app: AppHandle, request: RemoveCredentialRequest) -> Result<(), String> {
    validate_credential_ref(&request.credential_ref)?;
    for kind in &request.kinds {
        validate_credential_kind(kind)?;
    }

    let workspace = active_workspace_dir(&app)?;
    let mut credentials_file = read_credentials_file(&workspace)?;
    credentials_file.credentials.retain(|record| {
        record.credential_ref != request.credential_ref || !request.kinds.contains(&record.kind)
    });
    write_credentials_file(&workspace, &credentials_file)
}

fn device_credential_key_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join(DEVICE_CREDENTIAL_KEY_FILE))
}

fn read_credentials_file(workspace: &Path) -> Result<CredentialsFile, String> {
    let path = workspace.join(CREDENTIALS_FILE);
    if !path.exists() {
        return Ok(CredentialsFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            credentials: Vec::new(),
        });
    }

    read_json_file(&path)
}

fn write_credentials_file(workspace: &Path, credentials_file: &CredentialsFile) -> Result<(), String> {
    write_json_file(&workspace.join(CREDENTIALS_FILE), credentials_file)
}

fn device_credential_key(app: &AppHandle) -> Result<[u8; DEVICE_CREDENTIAL_KEY_BYTES], String> {
    let path = device_credential_key_path(app)?;
    if path.exists() {
        let key_file: DeviceCredentialKeyFile = read_json_file(&path)?;
        if !key_file.key.trim().is_empty() {
            return decode_device_credential_key(&key_file.key);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_message)?;
    }

    let key = generate_device_credential_key()?;
    write_json_file(
        &path,
        &DeviceCredentialKeyFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            key: hex_encode(&key),
        },
    )?;

    Ok(key)
}

fn generate_device_credential_key() -> Result<[u8; DEVICE_CREDENTIAL_KEY_BYTES], String> {
    let rng = SystemRandom::new();
    let mut key = [0_u8; DEVICE_CREDENTIAL_KEY_BYTES];
    rng.fill(&mut key)
        .map_err(|_| "Failed to generate local credential key".to_string())?;

    Ok(key)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    let trimmed = value.trim();
    if trimmed.len() % 2 != 0 {
        return Err("Invalid hex value length".to_string());
    }

    (0..trimmed.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&trimmed[index..index + 2], 16)
                .map_err(|_| "Invalid hex value".to_string())
        })
        .collect()
}

fn decode_device_credential_key(value: &str) -> Result<[u8; DEVICE_CREDENTIAL_KEY_BYTES], String> {
    let bytes = hex_decode(value)?;
    bytes
        .try_into()
        .map_err(|_| "Invalid local credential key length".to_string())
}

fn save_secret_if_present(
    credentials_file: &mut CredentialsFile,
    key: &[u8; DEVICE_CREDENTIAL_KEY_BYTES],
    credential_ref: &str,
    kind: &str,
    secret: Option<&str>,
) -> Result<(), String> {
    let Some(secret) = secret.filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    validate_credential_kind(kind)?;
    let record = encrypt_credential_record(key, credential_ref, kind, secret)?;
    credentials_file
        .credentials
        .retain(|existing| existing.credential_ref != credential_ref || existing.kind != kind);
    credentials_file.credentials.push(record);
    Ok(())
}

fn encrypt_credential_record(
    key: &[u8; DEVICE_CREDENTIAL_KEY_BYTES],
    credential_ref: &str,
    kind: &str,
    secret: &str,
) -> Result<EncryptedCredentialRecord, String> {
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0_u8; CREDENTIAL_NONCE_BYTES];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| "Failed to generate credential nonce".to_string())?;

    let cipher_key = credential_cipher_key(key)?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut buffer = secret.as_bytes().to_vec();
    cipher_key
        .seal_in_place_append_tag(nonce, credential_aad(credential_ref, kind), &mut buffer)
        .map_err(|_| "Failed to encrypt credential".to_string())?;

    Ok(EncryptedCredentialRecord {
        credential_ref: credential_ref.to_string(),
        kind: kind.to_string(),
        nonce: hex_encode(&nonce_bytes),
        ciphertext: hex_encode(&buffer),
    })
}

fn decrypt_credential_record(
    key: &[u8; DEVICE_CREDENTIAL_KEY_BYTES],
    record: &EncryptedCredentialRecord,
) -> Result<String, String> {
    validate_credential_kind(&record.kind)?;
    let nonce_bytes: [u8; CREDENTIAL_NONCE_BYTES] = hex_decode(&record.nonce)?
        .try_into()
        .map_err(|_| "Invalid credential nonce length".to_string())?;
    let mut buffer = hex_decode(&record.ciphertext)?;
    let cipher_key = credential_cipher_key(key)?;
    let plaintext = cipher_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_bytes),
            credential_aad(&record.credential_ref, &record.kind),
            &mut buffer,
        )
        .map_err(|_| "Failed to decrypt credential".to_string())?;

    String::from_utf8(plaintext.to_vec()).map_err(error_message)
}

fn credential_cipher_key(key: &[u8; DEVICE_CREDENTIAL_KEY_BYTES]) -> Result<LessSafeKey, String> {
    let unbound_key = UnboundKey::new(&CHACHA20_POLY1305, key)
        .map_err(|_| "Failed to open credential cipher".to_string())?;
    Ok(LessSafeKey::new(unbound_key))
}

fn credential_aad(credential_ref: &str, kind: &str) -> Aad<Vec<u8>> {
    Aad::from(format!("xin-terminal-credential:{credential_ref}:{kind}").into_bytes())
}

fn validate_credential_ref(credential_ref: &str) -> Result<(), String> {
    if credential_ref.trim().is_empty() {
        return Err("Credential reference is required".to_string());
    }

    Ok(())
}

fn validate_credential_kind(kind: &str) -> Result<(), String> {
    match kind {
        "password" | "passphrase" => Ok(()),
        _ => Err("Unsupported credential kind".to_string()),
    }
}
