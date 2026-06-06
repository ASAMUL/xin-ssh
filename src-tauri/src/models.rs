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
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
    pub schema_version: u16,
    pub locale: String,
    pub terminal_font_family: String,
    #[serde(default = "default_monitor_refresh_seconds")]
    pub monitor_refresh_seconds: u64,
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
pub struct CredentialSecret {
    pub password: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCredentialRequest {
    pub credential_ref: String,
    pub secret: CredentialSecret,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadCredentialRequest {
    pub credential_ref: String,
    pub kind: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveCredentialRequest {
    pub credential_ref: String,
    pub kinds: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirectoryListRequest {
    pub session_id: String,
    pub path: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSystemInfoRequest {
    pub session_id: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCreateDirectoryRequest {
    pub session_id: String,
    pub path: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRenameRequest {
    pub session_id: String,
    pub from_path: String,
    pub to_path: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeleteRequest {
    pub session_id: String,
    pub entries: Vec<RemoteTransferEntry>,
    pub recursive: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSystemSnapshot {
    pub session_id: String,
    pub captured_at: u64,
    pub hostname: Option<String>,
    pub kernel: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub load_average: Vec<f64>,
    pub cpu: RemoteCpuInfo,
    pub memory: RemoteMemoryInfo,
    pub swap: RemoteMemoryInfo,
    pub networks: Vec<RemoteNetworkInfo>,
    pub processes: Vec<RemoteProcessInfo>,
    pub disks: Vec<RemoteDiskInfo>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCpuInfo {
    pub usage_percent: Option<f64>,
    pub core_count: Option<u32>,
    pub model_name: Option<String>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMemoryInfo {
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub usage_percent: Option<f64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteNetworkInfo {
    pub name: String,
    pub ipv4_address: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProcessInfo {
    pub pid: u32,
    pub user: String,
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub memory_bytes: Option<u64>,
    pub command: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDiskInfo {
    pub filesystem: String,
    pub path: String,
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub usage_percent: Option<f64>,
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteUploadRequest {
    pub task_id: String,
    pub session_id: String,
    pub local_paths: Vec<String>,
    pub remote_dir: String,
    pub conflict_policy: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDownloadRequest {
    pub task_id: String,
    pub session_id: String,
    pub remote_entries: Vec<RemoteTransferEntry>,
    pub local_dir: String,
    pub conflict_policy: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTransferEntry {
    pub path: String,
    pub name: String,
    pub is_directory: bool,
    pub size: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferEventPayload {
    pub task_id: String,
    pub session_id: String,
    pub direction: String,
    pub kind: String,
    pub label: Option<String>,
    pub current_path: Option<String>,
    pub bytes_transferred: u64,
    pub bytes_total: u64,
    pub files_done: u64,
    pub files_total: u64,
    pub error: Option<String>,
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
    pub settings: WorkspaceSettings,
    pub groups: Vec<ConnectionGroup>,
    pub profiles: Vec<ConnectionProfile>,
}

fn default_monitor_refresh_seconds() -> u64 {
    5
}
