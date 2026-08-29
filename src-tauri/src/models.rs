use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const CURRENT_SCHEMA_VERSION: u16 = 1;

pub fn default_schema_version() -> u16 {
    CURRENT_SCHEMA_VERSION
}

// 所有落盘结构都开 `#[serde(default)]`：缺字段 / 旧版本 / 新版本写回的文件
// 都能反序列化成功，不会因为一个字段就让整个 workspace 加载失败。
// 记录级结构缺 id 时视为损坏，由 workspace.rs 在加载后过滤掉。

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppStateFile {
    #[serde(default = "default_schema_version")]
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
#[serde(rename_all = "camelCase", default)]
pub struct WorkspaceSettings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub locale: String,
    pub terminal_font_family: String,
    #[serde(default = "default_monitor_refresh_seconds")]
    pub monitor_refresh_seconds: u64,
    #[serde(default = "default_terminal_theme")]
    pub terminal_theme: String,
    #[serde(default = "default_terminal_font_size")]
    pub terminal_font_size: u16,
    // 工作区面板布局：显隐开关 + splitpanes 百分比，serde default 兼容旧 settings.json
    #[serde(default = "default_true")]
    pub show_monitor_sidebar: bool,
    #[serde(default = "default_true")]
    pub show_file_explorer: bool,
    #[serde(default = "default_true")]
    pub show_file_tree: bool,
    #[serde(default = "default_true")]
    pub show_hidden_files: bool,
    #[serde(default = "default_monitor_pane_size")]
    pub monitor_pane_size: f64,
    #[serde(default = "default_file_pane_size")]
    pub file_pane_size: f64,
    #[serde(default = "default_file_tree_size")]
    pub file_tree_size: f64,
    // 首页「所有连接」视图模式：grid | list
    #[serde(default = "default_home_view_mode")]
    pub home_view_mode: String,
    // SSH keepalive 间隔（秒），0 = 禁用；终端与隧道连接共用
    #[serde(default = "default_ssh_keepalive_seconds")]
    pub ssh_keepalive_seconds: u64,
}

// 新装、settings.json 缺字段、以及文件损坏回退，都落到这一份默认值上
impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            locale: default_locale(),
            terminal_font_family: default_terminal_font_family(),
            monitor_refresh_seconds: default_monitor_refresh_seconds(),
            terminal_theme: default_terminal_theme(),
            terminal_font_size: default_terminal_font_size(),
            show_monitor_sidebar: true,
            show_file_explorer: true,
            show_file_tree: true,
            show_hidden_files: true,
            monitor_pane_size: default_monitor_pane_size(),
            file_pane_size: default_file_pane_size(),
            file_tree_size: default_file_tree_size(),
            home_view_mode: default_home_view_mode(),
            ssh_keepalive_seconds: default_ssh_keepalive_seconds(),
        }
    }
}

pub fn default_locale() -> String {
    "zh-CN".to_string()
}

pub fn default_terminal_font_family() -> String {
    "Cascadia Code".to_string()
}

pub fn default_terminal_theme() -> String {
    "aurora".to_string()
}

pub fn default_terminal_font_size() -> u16 {
    14
}

pub fn default_true() -> bool {
    true
}

// 监控栏占工作区宽度的百分比
pub fn default_monitor_pane_size() -> f64 {
    22.0
}

// 文件面板占工作区高度的百分比
pub fn default_file_pane_size() -> f64 {
    42.0
}

// 目录树占文件面板宽度的百分比
pub fn default_file_tree_size() -> f64 {
    22.0
}

pub fn default_home_view_mode() -> String {
    "grid".to_string()
}

pub fn default_ssh_keepalive_seconds() -> u64 {
    30
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfilesFile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub groups: Vec<ConnectionGroup>,
    pub profiles: Vec<ConnectionProfile>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConnectionProfile {
    pub id: String,
    pub sync_id: String,
    pub group_id: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub private_key_path: Option<String>,
    pub credential_ref: Option<String>,
    #[serde(default = "default_remote_path")]
    pub default_remote_path: String,
    pub sort_order: i32,
    // 首页 Dashboard 字段：收藏走 save_profile 正常同步；最后连接时间由
    // mark_profile_connected 单独写入且不 bump updated_at（避免同步冲突）
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub last_connected_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

pub fn default_ssh_port() -> u16 {
    22
}

pub fn default_remote_path() -> String {
    "/".to_string()
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethod {
    #[default]
    Password,
    PrivateKey,
}

// —— 命令片段库（snippets.json，软删除同 profiles）——

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnippetsFile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub snippets: Vec<Snippet>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Snippet {
    pub id: String,
    pub sync_id: String,
    pub name: String,
    pub command: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

// —— SSH 隧道（本地端口转发）——

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelsFile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub tunnels: Vec<TunnelConfig>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelConfig {
    pub id: String,
    pub sync_id: String,
    pub profile_id: String,
    pub name: String,
    pub local_port: u16,
    // 从服务器视角解析的转发目标（默认 127.0.0.1 即服务器自身）
    #[serde(default = "default_tunnel_remote_host")]
    pub remote_host: String,
    pub remote_port: u16,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

pub fn default_tunnel_remote_host() -> String {
    "127.0.0.1".to_string()
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStartRequest {
    pub tunnel_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    #[serde(default)]
    pub keepalive_seconds: Option<u64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelEventPayload {
    pub tunnel_id: String,
    pub kind: String,
    pub error: Option<AppError>,
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
    pub attempt_id: Option<String>,
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
    // 未传时用默认 30s（settings 可调，0 = 禁用）
    #[serde(default)]
    pub keepalive_seconds: Option<u64>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInputRequest {
    pub session_id: String,
    pub data: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustHostKeyRequest {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub algorithm: Option<String>,
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
pub struct RemoteDirectoryStreamRequest {
    pub session_id: String,
    pub request_id: String,
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
pub struct RemoteCreateFileRequest {
    pub session_id: String,
    pub path: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteListDirectoryRequest {
    pub session_id: String,
    pub path: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteReadFileRequest {
    pub session_id: String,
    pub path: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWriteFileRequest {
    pub session_id: String,
    pub path: String,
    pub content: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileContent {
    pub path: String,
    pub size: u64,
    pub content: String,
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
pub struct DirectoryEventPayload {
    pub session_id: String,
    pub request_id: String,
    pub kind: String,
    pub path: String,
    pub entries: Vec<RemoteFileEntry>,
    pub total: Option<u64>,
    pub error: Option<AppError>,
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
    pub error: Option<AppError>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEventPayload {
    pub session_id: String,
    pub attempt_id: Option<String>,
    pub kind: String,
    pub data: Option<String>,
    pub error: Option<AppError>,
    pub reason: Option<String>,
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
