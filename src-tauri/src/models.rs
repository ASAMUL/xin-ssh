use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateFile {
    pub schema_version: u16,
    pub workspace_dir: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub initialized: bool,
    pub workspace_dir: Option<String>,
    pub vault_path: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
    pub schema_version: u16,
    pub locale: String,
    pub terminal_font_family: String,
    pub remember_vault_unlock: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesFile {
    pub schema_version: u16,
    pub groups: Vec<ConnectionGroup>,
    pub profiles: Vec<ConnectionProfile>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionGroup {
    pub id: String,
    pub sync_id: String,
    pub name: String,
    pub sort_order: i32,
    pub collapsed: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub sync_id: String,
    pub group_id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub private_key_path: Option<String>,
    pub credential_ref: Option<String>,
    pub default_remote_path: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    Password,
    PrivateKey,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub authenticated: bool,
    pub auth_method: AuthMethod,
    pub latency_ms: u128,
    pub server_fingerprint: Option<String>,
    pub server_key_algorithm: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalStartRequest {
    pub session_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
    pub default_remote_path: String,
    pub columns: u32,
    pub rows: u32,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInputRequest {
    pub session_id: String,
    pub data: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResizeRequest {
    pub session_id: String,
    pub columns: u32,
    pub rows: u32,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirectoryListRequest {
    pub session_id: String,
    pub path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirectoryListing {
    pub session_id: String,
    pub path: String,
    pub entries: Vec<RemoteFileEntry>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: Option<u64>,
    pub modified_at: Option<u64>,
    pub permissions: Option<String>,
    pub permissions_mode: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub is_hidden: bool,
    pub is_directory: bool,
    pub is_symlink: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEventPayload {
    pub session_id: String,
    pub kind: String,
    pub data: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub workspace_dir: String,
    pub vault_path: String,
    pub settings: WorkspaceSettings,
    pub groups: Vec<ConnectionGroup>,
    pub profiles: Vec<ConnectionProfile>,
}
