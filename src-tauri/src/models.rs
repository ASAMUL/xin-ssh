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

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionRestoreEntry {
    pub profile_id: String,
    pub remote_path: Option<String>,
    pub label_index: Option<u32>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionRestoreState {
    pub sessions: Vec<SessionRestoreEntry>,
    pub active_index: Option<usize>,
    pub active_profile_id: Option<String>,
    pub active_label_index: Option<u32>,
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
    // 终端调色板家族（xin / catppuccin / …）；老存档里已下线的主题 id 由前端折算
    #[serde(default = "default_terminal_theme")]
    pub terminal_theme: String,
    // 终端明暗：auto（跟随应用）| light | dark
    #[serde(default = "default_terminal_appearance")]
    pub terminal_appearance: String,
    // 终端材质：solid（实底）| clear（通透，透出应用壁纸）
    #[serde(default = "default_terminal_material")]
    pub terminal_material: String,
    // 命令联想：按本机记下的命令历史给灰字提示，历史只存在本机、不进工作区
    #[serde(default = "default_true")]
    pub command_suggestions: bool,
    // 文件面板跟随终端目录：只读终端已经显示出来的提示符 / OSC 7，不往服务器发命令
    #[serde(default = "default_true")]
    pub follow_terminal_cwd: bool,
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
    // 全局终端背景图；单个连接可用 ConnectionProfile::terminal_background 覆盖
    #[serde(default)]
    pub terminal_background: Option<TerminalBackground>,
    // SSH keepalive 间隔（秒），0 = 禁用；终端与隧道连接共用
    #[serde(default = "default_ssh_keepalive_seconds")]
    pub ssh_keepalive_seconds: u64,
    // 应用明暗模式：system | light | dark（终端默认跟着它走，见 terminal_appearance）
    #[serde(default = "default_appearance_mode")]
    pub appearance_mode: String,
    // 液态玻璃强度 0~100：0 通透（透出桌面壁纸），100 着色（更实、对比更高），设置里是一条滑块
    #[serde(default = "default_glass_tint")]
    pub glass_tint: u8,
    // 旧版只有通透 / 着色两档开关。只读不写：读到后由 migrate_legacy_fields 折算成 glass_tint，
    // 下次写盘时这个字段就消失了；也因为不序列化，save_settings 的字段白名单里没有它，前端改不动
    #[serde(default, skip_serializing)]
    pub glass_style: Option<String>,
    // 应用壁纸：铺在整个窗口最底层，给液态玻璃「底下的东西」。
    // None 和两档都没换过一样，都是内置壁纸——玻璃底下不能是空的，所以没有「无壁纸」这个状态。
    #[serde(default)]
    pub app_wallpaper: Option<AppWallpaper>,
    // 只恢复连接引用和界面位置；密码、OTP、sessionId 与终端输出永不进入配置。
    #[serde(default)]
    pub session_restore: SessionRestoreState,
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
            terminal_appearance: default_terminal_appearance(),
            terminal_material: default_terminal_material(),
            command_suggestions: true,
            follow_terminal_cwd: true,
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
            terminal_background: None,
            appearance_mode: default_appearance_mode(),
            glass_tint: default_glass_tint(),
            glass_style: None,
            app_wallpaper: None,
            session_restore: SessionRestoreState::default(),
        }
    }
}

impl WorkspaceSettings {
    /// 把旧版字段折算成现在的形状；读盘后调用一次，写盘时旧字段不会再写出去
    pub fn migrate_legacy_fields(&mut self) {
        // 两档开关落到滑块两端：通透就是 0，着色就是 100
        if let Some(style) = self.glass_style.take() {
            self.glass_tint = if style == "tinted" { 100 } else { 0 };
        }
        if let Some(wallpaper) = self.app_wallpaper.as_mut() {
            wallpaper.migrate_legacy_file();
        }
    }
}

/// 应用壁纸。浅色 / 深色各一张，跟着应用的实际明暗切换。
/// `None` 表示这一档用内置壁纸：内置图是前端打包进安装包的资源，不复制进工作区——
/// 工作区可以同步到别的机器，没必要每份都带两张一样的图。
/// 用户换上的图和终端背景图同一套存储：复制进 `<workspace>/backgrounds/`，这里只记文件名。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppWallpaper {
    pub light: Option<String>,
    pub dark: Option<String>,
    /// 图片压在画布上的可见度 0.1~1，两档共用
    pub opacity: f64,
    /// 高斯模糊半径 px，0 = 不模糊，两档共用
    pub blur: u16,
    // 旧版只有一张壁纸、明暗共用（形状同 TerminalBackground，fit 读到也直接丢掉）。
    // 只读不写：读到后由 migrate_legacy_file 落到两档上，下次写盘时这个字段就消失了
    #[serde(skip_serializing)]
    file: Option<String>,
}

impl Default for AppWallpaper {
    fn default() -> Self {
        Self {
            light: None,
            dark: None,
            opacity: 1.0,
            blur: 0,
            file: None,
        }
    }
}

impl AppWallpaper {
    // 升级前那张图两种明暗下都在用，升级后也得两档都是它，不能悄悄换成内置图
    fn migrate_legacy_file(&mut self) {
        if let Some(file) = self.file.take().filter(|file| !file.is_empty()) {
            self.light = Some(file.clone());
            self.dark = Some(file);
        }
    }
}

/// 终端背景图。图片本体统一存放在 `<workspace>/backgrounds/`，这里只记文件名——
/// 绝对路径会在换机器 / 同步工作区后失效，而工作区目录本来就是可同步的整体。
///
/// `WorkspaceSettings` 持有全局一张，`ConnectionProfile` 可以覆盖成自己的一张
/// （生产机配红底、测试机配蓝底，切 tab 时背景直接变，误操作概率会明显下降）。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerminalBackground {
    /// 文件名（不含目录），实际文件在 `<workspace>/backgrounds/` 下
    pub file: String,
    /// 图片可见度 0.04~1
    #[serde(default = "default_background_opacity")]
    pub opacity: f64,
    /// 高斯模糊半径 px，0 = 不模糊
    #[serde(default)]
    pub blur: u16,
    /// cover | contain | tile
    #[serde(default = "default_background_fit")]
    pub fit: String,
}

impl Default for TerminalBackground {
    fn default() -> Self {
        Self {
            file: String::new(),
            opacity: default_background_opacity(),
            blur: 0,
            fit: default_background_fit(),
        }
    }
}

pub fn default_background_opacity() -> f64 {
    0.35
}

pub fn default_background_fit() -> String {
    "cover".to_string()
}

pub fn default_locale() -> String {
    "zh-CN".to_string()
}

/// 默认跟随系统明暗，前端 utils/appearanceMode.js 负责解析成实际的 light/dark
pub fn default_appearance_mode() -> String {
    "system".to_string()
}

/// 默认最通透；前端 utils/glassTint.js 把它换算成 0~1 写到 <html> 的 --xin-glass-tint
pub fn default_glass_tint() -> u8 {
    0
}

pub fn default_terminal_font_family() -> String {
    "Cascadia Code".to_string()
}

pub fn default_terminal_theme() -> String {
    "xin".to_string()
}

pub fn default_terminal_appearance() -> String {
    "auto".to_string()
}

pub fn default_terminal_material() -> String {
    "solid".to_string()
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
    pub agent: Option<AgentConfig>,
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
    // 该连接专属的终端背景图；None = 跟随全局设置
    #[serde(default)]
    pub terminal_background: Option<TerminalBackground>,
    // 用户手选的系统图标；None / "auto" = 用连接后自动识别的 detected_os
    #[serde(default)]
    pub os_icon: Option<String>,
    // 连接成功后从远端 /etc/os-release 识别出的图标 id，由 set_profile_detected_os 单独写入
    #[serde(default)]
    pub detected_os: Option<String>,
    // 生产机：标签 / 终端带红色标记，危险命令和多行粘贴执行前确认（全在客户端拦，不碰服务器）
    #[serde(default)]
    pub production: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigEntry {
    pub alias: String,
    pub host: String,
    pub username: String,
    pub port: u16,
    pub private_key_path: Option<String>,
    // 原样保留 ProxyJump / ProxyCommand：现在用来在导入时跳过，以后支持跳板机时可以直接接上
    pub proxy: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionExportEntry {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub private_key_path: Option<String>,
    pub default_remote_path: String,
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
    Agent,
    KeyboardInteractive,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentConfig {
    pub provider: AgentProvider,
    pub socket_path: Option<String>,
    pub key_fingerprint: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentProvider {
    #[default]
    System,
    Pageant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIdentityInfo {
    pub fingerprint: String,
    pub comment: String,
    pub algorithm: String,
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
    pub attempt_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub agent: Option<AgentConfig>,
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
    pub attempt_id: String,
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
    pub agent: Option<AgentConfig>,
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
    pub agent: Option<AgentConfig>,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
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
    pub expected_version: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileContent {
    pub path: String,
    pub size: u64,
    pub content: String,
    pub version: String,
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
pub struct RemoteCopyRequest {
    pub session_id: String,
    pub from_path: String,
    pub to_path: String,
    pub recursive: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteChmodRequest {
    pub session_id: String,
    pub path: String,
    pub mode: u32,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeleteRequest {
    pub task_id: String,
    pub session_id: String,
    pub entries: Vec<RemoteTransferEntry>,
    pub recursive: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationEventPayload {
    pub task_id: String,
    pub session_id: String,
    pub kind: String,
    pub current_path: Option<String>,
    pub completed_entries: u64,
    pub error: Option<AppError>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSystemSnapshot {
    pub session_id: String,
    pub captured_at: u64,
    pub hostname: Option<String>,
    pub kernel: Option<String>,
    // /etc/os-release 的 ID（ubuntu / debian / rhel …），前端据此挑连接图标
    pub os_id: Option<String>,
    // /etc/os-release 的 PRETTY_NAME（如 "Ubuntu 24.04 LTS"），概览卡片的标题
    pub os_name: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub load_average: Vec<f64>,
    pub cpu: RemoteCpuInfo,
    pub memory: RemoteMemoryInfo,
    pub swap: RemoteMemoryInfo,
    pub networks: Vec<RemoteNetworkInfo>,
    pub processes: Vec<RemoteProcessInfo>,
    pub disks: Vec<RemoteDiskInfo>,
    // 运行中的 systemd 服务；None = 这台机器没有 systemd（Alpine、容器等），和「一个都没跑」区分开
    pub services: Option<Vec<RemoteServiceInfo>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteServiceInfo {
    pub name: String,
    pub sub_state: String,
    pub description: String,
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
    // 概览的内存分段条要「已用 / 缓存 / 空闲」三段；交换分区没有缓存一说，恒为 None
    pub cache_bytes: Option<u64>,
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
    // 之前几次尝试留下、还没传完的文件；这次取消时一并删掉，失败时继续报给前端
    #[serde(default)]
    pub partial_files: Vec<PartialFile>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDownloadRequest {
    pub task_id: String,
    pub session_id: String,
    pub remote_entries: Vec<RemoteTransferEntry>,
    pub local_dir: String,
    pub conflict_policy: Option<String>,
    #[serde(default)]
    pub partial_files: Vec<PartialFile>,
}

// 没传完的目标文件。删之前要确认它还是当时那个半成品，不能误删别人后来放上去的完整文件：
// 记下了大小和修改时间就要求两者都没变；断线时读不到属性，只能要求它比完整文件小。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialFile {
    pub path: String,
    pub full_size: u64,
    pub size: Option<u64>,
    pub modified: Option<u64>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscardPartialFilesRequest {
    pub direction: String,
    // 上传的半成品在服务器上，要借一个已连接的会话去删；下载的在本机，不需要
    pub session_id: Option<String>,
    pub files: Vec<PartialFile>,
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
    // 只在结束事件里有值：这次结束后还留在磁盘上的半成品；None 表示后端不清楚，前端沿用旧值
    pub partial_files: Option<Vec<PartialFile>>,
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
