use std::{collections::HashMap, fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{codes, io_error, lock_poisoned, required, AppError};
use crate::models::TrustHostKeyRequest;
use crate::util::now_millis;
use crate::workspace::{read_json_file_or_recover, write_json_file, CURRENT_SCHEMA_VERSION};

const KNOWN_HOSTS_FILE: &str = "known-hosts.json";

// known-hosts.json 是本机信任状态（TOFU），存应用数据目录，不进可同步的 workspace 目录。
// 多个会话可能同时首连并记录指纹，用进程内锁串行化读改写。
static KNOWN_HOSTS_IO_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct KnownHostsFile {
    schema_version: u16,
    hosts: HashMap<String, KnownHostEntry>,
}

impl Default for KnownHostsFile {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            hosts: HashMap::new(),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct KnownHostEntry {
    fingerprint: String,
    algorithm: Option<String>,
    first_seen_at: u64,
    updated_at: u64,
}

#[tauri::command]
pub async fn trust_host_key(app: AppHandle, request: TrustHostKeyRequest) -> Result<(), AppError> {
    if request.host.trim().is_empty() {
        return Err(required("host"));
    }

    if request.port == 0 {
        return Err(crate::error::port_invalid());
    }

    if request.fingerprint.trim().is_empty() {
        return Err(required("fingerprint"));
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
) -> Result<Option<String>, AppError> {
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
) -> Result<(), AppError> {
    let _guard = KNOWN_HOSTS_IO_LOCK.lock().map_err(|_| lock_poisoned())?;
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
        fs::create_dir_all(parent).map_err(io_error)?;
    }

    write_json_file(&path, &known_hosts)
}

/// 指纹与记录不一致：前端据此弹「主机密钥已变更」面板，
/// 两个指纹通过 params 传递（不再靠 `HOST_KEY_CHANGED|a|b` 拼字符串）。
pub(crate) fn host_key_changed_error(expected: &str, actual: &str) -> AppError {
    AppError::new(
        codes::HOST_KEY_CHANGED,
        format!("Host key changed (expected {expected}, got {actual})"),
    )
    .with("expected", expected)
    .with("actual", actual)
}

fn read_known_hosts(app: &AppHandle) -> Result<KnownHostsFile, AppError> {
    read_known_hosts_at(&known_hosts_path(app)?)
}

fn read_known_hosts_at(path: &PathBuf) -> Result<KnownHostsFile, AppError> {
    if !path.exists() {
        return Ok(KnownHostsFile::default());
    }

    read_json_file_or_recover(path, KnownHostsFile::default)
}

fn known_hosts_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app.path().app_data_dir().map_err(io_error)?;
    Ok(dir.join(KNOWN_HOSTS_FILE))
}

fn host_entry_id(host: &str, port: u16) -> String {
    format!("{}:{port}", host.trim().to_ascii_lowercase())
}
