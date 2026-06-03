use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use ring::rand::{SecureRandom, SystemRandom};
use russh::{
    ChannelMsg,
    client,
    keys::{load_secret_key, ssh_key, PrivateKeyWithHashAlg, PublicKey},
    Disconnect,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration, Instant};

use crate::models::{
    AppStateFile, AppStatus, AuthMethod, ConnectionGroup, ConnectionProfile, ConnectionTestRequest,
    ConnectionTestResult, ProfilesFile, TerminalEventPayload, TerminalInputRequest,
    TerminalResizeRequest, TerminalStartRequest, WorkspaceSettings, WorkspaceSnapshot,
};

const APP_STATE_FILE: &str = "app-state.json";
const DEVICE_VAULT_KEY_FILE: &str = "device-vault-key.json";
const PROFILES_FILE: &str = "profiles.json";
const SETTINGS_FILE: &str = "settings.json";
const VAULT_FILE: &str = "vault.xinvault";
const CURRENT_SCHEMA_VERSION: u16 = 1;
const DEVICE_VAULT_KEY_BYTES: usize = 32;
const TERMINAL_EVENT: &str = "terminal-event";

#[derive(Clone, Default)]
pub struct TerminalRegistry {
    sessions: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<TerminalControl>>>>,
}

enum TerminalControl {
    Input(Vec<u8>),
    Resize { columns: u32, rows: u32 },
    Close,
}

impl TerminalRegistry {
    fn replace(
        &self,
        session_id: String,
        sender: mpsc::UnboundedSender<TerminalControl>,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(error_message)?;
        if let Some(existing) = sessions.insert(session_id, sender) {
            let _ = existing.send(TerminalControl::Close);
        }

        Ok(())
    }

    fn send(&self, session_id: &str, control: TerminalControl) -> Result<(), String> {
        let sessions = self.sessions.lock().map_err(error_message)?;
        let sender = sessions
            .get(session_id)
            .ok_or_else(|| "Terminal session is not running".to_string())?;

        sender
            .send(control)
            .map_err(|_| "Terminal session is not accepting input".to_string())
    }

    fn close(&self, session_id: &str) -> Result<(), String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .remove(session_id);

        if let Some(sender) = sender {
            let _ = sender.send(TerminalControl::Close);
        }

        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceVaultKeyFile {
    schema_version: u16,
    key: String,
}

#[tauri::command]
pub fn get_app_status(app: AppHandle) -> Result<AppStatus, String> {
    let state = read_app_state(&app)?;
    let workspace_dir = state.and_then(|state_file| state_file.workspace_dir);
    let initialized = workspace_dir
        .as_ref()
        .map(|dir| workspace_files_exist(Path::new(dir)))
        .unwrap_or(false);

    Ok(AppStatus {
        initialized,
        vault_path: workspace_dir.as_ref().map(|dir| vault_path(Path::new(dir))),
        workspace_dir,
    })
}

#[tauri::command]
pub fn init_workspace(
    app: AppHandle,
    workspace_dir: Option<String>,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = match workspace_dir {
        Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => default_workspace_dir(&app)?,
    };

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn load_workspace(app: AppHandle) -> Result<Option<WorkspaceSnapshot>, String> {
    let Some(state) = read_app_state(&app)? else {
        return Ok(None);
    };

    let Some(workspace_dir) = state.workspace_dir else {
        return Ok(None);
    };

    let workspace = PathBuf::from(workspace_dir);
    if !workspace_files_exist(&workspace) {
        return Ok(None);
    }

    load_workspace_from_dir(&workspace).map(Some)
}

#[tauri::command]
pub fn change_workspace_dir(
    app: AppHandle,
    workspace_dir: String,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = PathBuf::from(workspace_dir);

    if workspace_files_exist(&workspace) {
        write_app_state(&app, &workspace)?;
        return load_workspace_from_dir(&workspace);
    }

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn save_profile(
    app: AppHandle,
    profile: ConnectionProfile,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_profile = profile;
    next_profile.updated_at = now_stamp();

    if let Some(existing) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == next_profile.id)
    {
        *existing = next_profile;
    } else {
        profiles_file.profiles.push(next_profile);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, profile_id: String) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    if let Some(profile) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == profile_id)
    {
        profile.deleted_at = Some(deleted_at);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn save_group(app: AppHandle, group: ConnectionGroup) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_group = group;
    next_group.updated_at = now_stamp();

    if let Some(existing) = profiles_file
        .groups
        .iter_mut()
        .find(|item| item.id == next_group.id)
    {
        *existing = next_group;
    } else {
        profiles_file.groups.push(next_group);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn delete_group(app: AppHandle, group_id: String) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    if let Some(group) = profiles_file
        .groups
        .iter_mut()
        .find(|item| item.id == group_id)
    {
        group.deleted_at = Some(deleted_at.clone());
    }

    for profile in profiles_file
        .profiles
        .iter_mut()
        .filter(|profile| profile.group_id == group_id)
    {
        profile.group_id = default_group_id();
        profile.updated_at = deleted_at.clone();
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    settings: WorkspaceSettings,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    write_settings_file(&workspace, &settings)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn get_device_vault_password(app: AppHandle) -> Result<String, String> {
    let path = device_vault_key_path(&app)?;
    if path.exists() {
        let key_file: DeviceVaultKeyFile = read_json_file(&path)?;
        if !key_file.key.trim().is_empty() {
            return Ok(key_file.key);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_message)?;
    }

    let key = generate_device_vault_key()?;
    write_json_file(
        &path,
        &DeviceVaultKeyFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            key: key.clone(),
        },
    )?;

    Ok(key)
}

#[tauri::command]
pub fn archive_credential_store(app: AppHandle) -> Result<Option<String>, String> {
    let workspace = active_workspace_dir(&app)?;
    let vault = workspace.join(VAULT_FILE);
    if !vault.exists() {
        return Ok(None);
    }

    let backup = workspace.join(format!("vault-{}.backup.xinvault", now_stamp()));
    fs::rename(&vault, &backup).map_err(error_message)?;
    Ok(Some(path_to_string(&backup)))
}

#[tauri::command]
pub async fn test_connection(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, String> {
    validate_connection_test_request(&request)?;

    timeout(Duration::from_secs(12), run_connection_test(request))
        .await
        .map_err(|_| "Connection test timed out after 12 seconds".to_string())?
}

#[tauri::command]
pub async fn start_terminal_session(
    app: AppHandle,
    registry: State<'_, TerminalRegistry>,
    request: TerminalStartRequest,
) -> Result<(), String> {
    validate_terminal_start_request(&request)?;

    let session_id = request.session_id.clone();
    let (control_sender, control_receiver) = mpsc::unbounded_channel();
    registry.replace(session_id.clone(), control_sender)?;

    emit_terminal_event(
        &app,
        &request.session_id,
        "connecting",
        None,
        None,
    );

    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_terminal_shell(app.clone(), request, control_receiver).await {
            emit_terminal_event(&app, &session_id, "error", None, Some(error));
        }
    });

    Ok(())
}

#[tauri::command]
pub fn send_terminal_input(
    registry: State<'_, TerminalRegistry>,
    request: TerminalInputRequest,
) -> Result<(), String> {
    registry.send(
        &request.session_id,
        TerminalControl::Input(request.data.into_bytes()),
    )
}

#[tauri::command]
pub fn resize_terminal(
    registry: State<'_, TerminalRegistry>,
    request: TerminalResizeRequest,
) -> Result<(), String> {
    registry.send(
        &request.session_id,
        TerminalControl::Resize {
            columns: request.columns.max(1),
            rows: request.rows.max(1),
        },
    )
}

#[tauri::command]
pub fn close_terminal_session(
    registry: State<'_, TerminalRegistry>,
    session_id: String,
) -> Result<(), String> {
    registry.close(&session_id)
}

async fn run_connection_test(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, String> {
    let auth_method = request.auth_method.clone();
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(12)),
        ..Default::default()
    });
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    let started = Instant::now();

    let mut session = client::connect(config, (host.as_str(), request.port), handler)
        .await
        .map_err(connection_error)?;

    let auth_result = match request.auth_method {
        AuthMethod::Password => {
            let password = request
                .password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Password is required".to_string())?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = request
                .private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Private key path is required".to_string())?;
            let passphrase = request
                .passphrase
                .as_deref()
                .filter(|value| !value.is_empty());
            let key_pair =
                load_secret_key(&private_key_path, passphrase).map_err(connection_error)?;
            let rsa_hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(connection_error)?
                .flatten();

            session
                .authenticate_publickey(
                    username,
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), rsa_hash),
                )
                .await
                .map_err(connection_error)?
        }
    };

    if !auth_result.success() {
        return Err("Authentication failed".to_string());
    }

    let _ = session
        .disconnect(Disconnect::ByApplication, "connection test complete", "en")
        .await;

    let server_key = server_key
        .lock()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_default();

    Ok(ConnectionTestResult {
        authenticated: true,
        auth_method,
        latency_ms: started.elapsed().as_millis(),
        server_fingerprint: server_key.fingerprint,
        server_key_algorithm: server_key.algorithm,
    })
}

async fn run_terminal_shell(
    app: AppHandle,
    request: TerminalStartRequest,
    mut control_receiver: mpsc::UnboundedReceiver<TerminalControl>,
) -> Result<(), String> {
    let session_id = request.session_id.clone();
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        ..Default::default()
    });
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();

    let mut session = client::connect(config, (host.as_str(), request.port), handler)
        .await
        .map_err(connection_error)?;

    authenticate_session(
        &mut session,
        username,
        request.auth_method.clone(),
        request.password,
        request.private_key_path,
        request.passphrase,
    )
    .await?;

    let mut channel = session.channel_open_session().await.map_err(connection_error)?;
    channel
        .request_pty(
            false,
            "xterm-256color",
            request.columns.max(1),
            request.rows.max(1),
            0,
            0,
            &[],
        )
        .await
        .map_err(connection_error)?;
    channel
        .request_shell(true)
        .await
        .map_err(connection_error)?;

    emit_terminal_event(&app, &session_id, "connected", None, None);

    let initial_path = request.default_remote_path.trim();
    if !initial_path.is_empty() && initial_path != "/" {
        channel
            .data_bytes(format!("cd {}\r", shell_quote(initial_path)).into_bytes())
            .await
            .map_err(connection_error)?;
    }

    loop {
        tokio::select! {
            Some(control) = control_receiver.recv() => {
                match control {
                    TerminalControl::Input(data) => {
                        channel
                            .data_bytes(data)
                            .await
                            .map_err(connection_error)?;
                    }
                    TerminalControl::Resize { columns, rows } => {
                        channel
                            .window_change(columns.max(1), rows.max(1), 0, 0)
                            .await
                            .map_err(connection_error)?;
                    }
                    TerminalControl::Close => {
                        let _ = channel.close().await;
                        break;
                    }
                }
            }
            message = channel.wait() => {
                let Some(message) = message else {
                    break;
                };

                match message {
                    ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                        emit_terminal_event(
                            &app,
                            &session_id,
                            "output",
                            Some(String::from_utf8_lossy(&data).to_string()),
                            None,
                        );
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => {
                        break;
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        emit_terminal_event(
                            &app,
                            &session_id,
                            "output",
                            Some(format!("\r\n[exit status: {exit_status}]\r\n")),
                            None,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = session
        .disconnect(Disconnect::ByApplication, "terminal closed", "en")
        .await;
    emit_terminal_event(&app, &session_id, "closed", None, None);
    Ok(())
}

async fn authenticate_session(
    session: &mut client::Handle<ConnectionTestClient>,
    username: String,
    auth_method: AuthMethod,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
) -> Result<(), String> {
    let auth_result = match auth_method {
        AuthMethod::Password => {
            let password = password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Password is required".to_string())?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Private key path is required".to_string())?;
            let passphrase = passphrase.as_deref().filter(|value| !value.is_empty());
            let key_pair =
                load_secret_key(&private_key_path, passphrase).map_err(connection_error)?;
            let rsa_hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(connection_error)?
                .flatten();

            session
                .authenticate_publickey(
                    username,
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), rsa_hash),
                )
                .await
                .map_err(connection_error)?
        }
    };

    if auth_result.success() {
        Ok(())
    } else {
        Err("Authentication failed".to_string())
    }
}

fn validate_connection_test_request(request: &ConnectionTestRequest) -> Result<(), String> {
    if request.host.trim().is_empty() {
        return Err("Host is required".to_string());
    }

    if request.username.trim().is_empty() {
        return Err("Username is required".to_string());
    }

    if request.port == 0 {
        return Err("Port must be greater than 0".to_string());
    }

    Ok(())
}

fn validate_terminal_start_request(request: &TerminalStartRequest) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.host.trim().is_empty() {
        return Err("Host is required".to_string());
    }

    if request.username.trim().is_empty() {
        return Err("Username is required".to_string());
    }

    if request.port == 0 {
        return Err("Port must be greater than 0".to_string());
    }

    match &request.auth_method {
        AuthMethod::Password => {
            if request.password.as_deref().unwrap_or_default().is_empty() {
                return Err("Password is required".to_string());
            }
        }
        AuthMethod::PrivateKey => {
            if request
                .private_key_path
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err("Private key path is required".to_string());
            }
        }
    }

    Ok(())
}

fn emit_terminal_event(
    app: &AppHandle,
    session_id: &str,
    kind: &str,
    data: Option<String>,
    error: Option<String>,
) {
    let _ = app.emit(
        TERMINAL_EVENT,
        TerminalEventPayload {
            session_id: session_id.to_string(),
            kind: kind.to_string(),
            data,
            error,
        },
    );
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Clone, Default)]
struct ServerKeySnapshot {
    fingerprint: Option<String>,
    algorithm: Option<String>,
}

struct ConnectionTestClient {
    server_key: Arc<Mutex<ServerKeySnapshot>>,
}

impl client::Handler for ConnectionTestClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        if let Ok(mut snapshot) = self.server_key.lock() {
            snapshot.fingerprint = Some(
                server_public_key
                    .fingerprint(ssh_key::HashAlg::Sha256)
                    .to_string(),
            );
            snapshot.algorithm = Some(server_public_key.algorithm().to_string());
        }

        Ok(true)
    }
}

fn load_workspace_from_dir(workspace: &Path) -> Result<WorkspaceSnapshot, String> {
    ensure_workspace(workspace)?;
    let settings = read_settings_file(workspace)?;
    let profiles_file = read_profiles_file(workspace)?;

    Ok(WorkspaceSnapshot {
        workspace_dir: path_to_string(workspace),
        vault_path: vault_path(workspace),
        settings,
        groups: profiles_file.groups,
        profiles: profiles_file.profiles,
    })
}

fn ensure_workspace(workspace: &Path) -> Result<(), String> {
    fs::create_dir_all(workspace).map_err(error_message)?;

    let profiles_path = workspace.join(PROFILES_FILE);
    if !profiles_path.exists() {
        write_profiles_file(workspace, &default_profiles_file())?;
    }

    let settings_path = workspace.join(SETTINGS_FILE);
    if !settings_path.exists() {
        write_settings_file(workspace, &default_settings())?;
    }

    Ok(())
}

fn workspace_files_exist(workspace: &Path) -> bool {
    workspace.join(PROFILES_FILE).exists() && workspace.join(SETTINGS_FILE).exists()
}

fn read_app_state(app: &AppHandle) -> Result<Option<AppStateFile>, String> {
    let path = app_state_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    read_json_file(&path).map(Some)
}

fn write_app_state(app: &AppHandle, workspace: &Path) -> Result<(), String> {
    let path = app_state_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_message)?;
    }

    write_json_file(
        &path,
        &AppStateFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            workspace_dir: Some(path_to_string(workspace)),
        },
    )
}

fn active_workspace_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let state = read_app_state(app)?.ok_or_else(|| "Workspace is not initialized".to_string())?;
    let workspace_dir = state
        .workspace_dir
        .ok_or_else(|| "Workspace directory is not configured".to_string())?;

    Ok(PathBuf::from(workspace_dir))
}

fn app_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join(APP_STATE_FILE))
}

fn device_vault_key_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join(DEVICE_VAULT_KEY_FILE))
}

fn default_workspace_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join("workspace"))
}

fn read_profiles_file(workspace: &Path) -> Result<ProfilesFile, String> {
    read_json_file(&workspace.join(PROFILES_FILE))
}

fn write_profiles_file(workspace: &Path, profiles_file: &ProfilesFile) -> Result<(), String> {
    write_json_file(&workspace.join(PROFILES_FILE), profiles_file)
}

fn read_settings_file(workspace: &Path) -> Result<WorkspaceSettings, String> {
    read_json_file(&workspace.join(SETTINGS_FILE))
}

fn write_settings_file(workspace: &Path, settings: &WorkspaceSettings) -> Result<(), String> {
    write_json_file(&workspace.join(SETTINGS_FILE), settings)
}

fn read_json_file<T>(path: &Path) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(error_message)?;
    serde_json::from_str(&content).map_err(error_message)
}

fn write_json_file<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: serde::Serialize,
{
    let content = serde_json::to_string_pretty(value).map_err(error_message)?;
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(error_message)?;

    if path.exists() {
        fs::remove_file(path).map_err(error_message)?;
    }

    fs::rename(temp_path, path).map_err(error_message)
}

fn default_profiles_file() -> ProfilesFile {
    let now = now_stamp();
    ProfilesFile {
        schema_version: CURRENT_SCHEMA_VERSION,
        groups: vec![ConnectionGroup {
            id: default_group_id(),
            sync_id: default_group_id(),
            name: "Ungrouped".to_string(),
            sort_order: 0,
            collapsed: false,
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        }],
        profiles: Vec::new(),
    }
}

fn default_settings() -> WorkspaceSettings {
    WorkspaceSettings {
        schema_version: CURRENT_SCHEMA_VERSION,
        locale: "zh-CN".to_string(),
        terminal_font_family: "Cascadia Code".to_string(),
        remember_vault_unlock: false,
    }
}

fn default_group_id() -> String {
    "group-default".to_string()
}

fn vault_path(workspace: &Path) -> String {
    path_to_string(&workspace.join(VAULT_FILE))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn now_stamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    millis.to_string()
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn connection_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn generate_device_vault_key() -> Result<String, String> {
    let rng = SystemRandom::new();
    let mut key = [0_u8; DEVICE_VAULT_KEY_BYTES];
    rng.fill(&mut key)
        .map_err(|_| "Failed to generate local credential key".to_string())?;

    Ok(hex_encode(&key))
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
