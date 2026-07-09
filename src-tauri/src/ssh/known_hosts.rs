use std::{collections::HashMap, fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::models::TrustHostKeyRequest;
use crate::util::{error_message, now_millis};
use crate::workspace::{read_json_file, write_json_file, CURRENT_SCHEMA_VERSION};

const KNOWN_HOSTS_FILE: &str = "known-hosts.json";
pub(crate) const HOST_KEY_CHANGED_PREFIX: &str = "HOST_KEY_CHANGED|";

// known-hosts.json 是本机信任状态（TOFU），存应用数据目录，不进可同步的 workspace 目录。
// 多个会话可能同时首连并记录指纹，用进程内锁串行化读改写。
static KNOWN_HOSTS_IO_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnownHostsFile {
    schema_version: u16,
    hosts: HashMap<String, KnownHostEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnownHostEntry {
    fingerprint: String,
    algorithm: Option<String>,
    first_seen_at: u64,
    updated_at: u64,
}

#[tauri::command]
pub async fn trust_host_key(app: AppHandle, request: TrustHostKeyRequest) -> Result<(), String> {
    if request.host.trim().is_empty() {
        return Err("Host is required".to_string());
    }

    if request.port == 0 {
        return Err("Port must be greater than 0".to_string());
    }

    if request.fingerprint.trim().is_empty() {
        return Err("Fingerprint is required".to_string());
    }

    record_host_key(
        &app,
        &request.host,
        request.port,
        request.fingerprint.trim(),
        request.algorithm.as_deref(),
    )
}

pub(crate) fn lookup_fingerprint(
    app: &AppHandle,
    host: &str,
    port: u16,
) -> Result<Option<String>, String> {
    let known_hosts = read_known_hosts(app)?;

    Ok(known_hosts
        .hosts
        .get(&host_entry_id(host, port))
        .map(|entry| entry.fingerprint.clone()))
}

pub(crate) fn record_host_key(
    app: &AppHandle,
    host: &str,
    port: u16,
    fingerprint: &str,
    algorithm: Option<&str>,
) -> Result<(), String> {
    let _guard = KNOWN_HOSTS_IO_LOCK.lock().map_err(error_message)?;
    let path = known_hosts_path(app)?;
    let mut known_hosts = read_known_hosts_at(&path)?;
    let now = now_millis();

    known_hosts
        .hosts
        .entry(host_entry_id(host, port))
        .and_modify(|entry| {
            entry.fingerprint = fingerprint.to_string();
            entry.algorithm = algorithm.map(str::to_string);
            entry.updated_at = now;
        })
        .or_insert_with(|| KnownHostEntry {
            fingerprint: fingerprint.to_string(),
            algorithm: algorithm.map(str::to_string),
            first_seen_at: now,
            updated_at: now,
        });
    known_hosts.schema_version = CURRENT_SCHEMA_VERSION;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_message)?;
    }

    write_json_file(&path, &known_hosts)
}

pub(crate) fn host_key_changed_error(expected: &str, actual: &str) -> String {
    format!("{HOST_KEY_CHANGED_PREFIX}{expected}|{actual}")
}

fn read_known_hosts(app: &AppHandle) -> Result<KnownHostsFile, String> {
    read_known_hosts_at(&known_hosts_path(app)?)
}

fn read_known_hosts_at(path: &PathBuf) -> Result<KnownHostsFile, String> {
    if !path.exists() {
        return Ok(KnownHostsFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            hosts: HashMap::new(),
        });
    }

    read_json_file(path)
}

fn known_hosts_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join(KNOWN_HOSTS_FILE))
}

fn host_entry_id(host: &str, port: u16) -> String {
    format!("{}:{port}", host.trim().to_ascii_lowercase())
}
