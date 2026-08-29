use std::collections::BTreeMap;
use std::fmt::{self, Display};

use serde::Serialize;

/// 结构化错误：前端按 `code` 查 i18n 文案，`params` 填占位符，
/// `message` 只作为英文兜底（未知 code）与日志用，不参与界面判断。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            params: BTreeMap::new(),
        }
    }

    pub fn with(mut self, key: &str, value: impl Display) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }

    pub fn is(&self, code: &str) -> bool {
        self.code == code
    }
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// 稳定错误码常量：Rust 侧构造与比较、前端 i18n 查表都以这里为准。
pub mod codes {
    pub const VALIDATION_REQUIRED: &str = "validation.required";
    pub const VALIDATION_PORT_INVALID: &str = "validation.portInvalid";
    pub const VALIDATION_SELECT_LOCAL_ITEM: &str = "validation.selectLocalItem";
    pub const VALIDATION_SELECT_REMOTE_ITEM: &str = "validation.selectRemoteItem";
    pub const VALIDATION_SELECT_LOCAL_DIR: &str = "validation.selectLocalDir";
    pub const VALIDATION_CONFLICT_POLICY: &str = "validation.unsupportedConflictPolicy";
    pub const VALIDATION_CREDENTIAL_KIND: &str = "validation.unsupportedCredentialKind";

    pub const AUTH_FAILED: &str = "auth.failed";
    pub const HOST_KEY_CHANGED: &str = "hostKey.changed";

    pub const SESSION_NOT_RUNNING: &str = "session.notRunning";
    pub const SESSION_NOT_ACCEPTING: &str = "session.notAccepting";
    pub const SESSION_REQUEST_CANCELED: &str = "session.requestCanceled";

    pub const TIMEOUT_OPERATION: &str = "timeout.operation";

    pub const REMOTE_TARGET_EXISTS: &str = "remote.targetExists";
    pub const REMOTE_IS_DIRECTORY: &str = "remote.isDirectory";
    pub const REMOTE_NOT_DIRECTORY: &str = "remote.notDirectory";
    pub const REMOTE_NOT_REGULAR_FILE: &str = "remote.notRegularFile";
    pub const REMOTE_ROOT_FORBIDDEN: &str = "remote.rootForbidden";
    pub const REMOTE_UNIQUE_NAME_FAILED: &str = "remote.uniqueNameFailed";
    pub const REMOTE_COMMAND_FAILED: &str = "remote.commandFailed";

    pub const LOCAL_TARGET_EXISTS: &str = "local.targetExists";
    pub const LOCAL_DUPLICATE_TARGET: &str = "local.duplicateTarget";
    pub const LOCAL_UNIQUE_NAME_FAILED: &str = "local.uniqueNameFailed";
    pub const LOCAL_FILE_NAME_UNKNOWN: &str = "local.fileNameUnknown";

    pub const TRANSFER_DUPLICATE_UPLOAD_TARGET: &str = "transfer.duplicateUploadTarget";
    pub const TRANSFER_CANCELED: &str = "transfer.canceled";
    pub const TRANSFER_PARTIAL_FAILURE: &str = "transfer.partialFailure";

    pub const FILE_TOO_LARGE: &str = "file.tooLarge";
    pub const FILE_BINARY: &str = "file.binary";
    pub const FILE_NOT_UTF8: &str = "file.notUtf8";

    pub const DIRECTORY_LISTING_CANCELED: &str = "directory.listingCanceled";

    pub const WORKSPACE_NOT_INITIALIZED: &str = "workspace.notInitialized";
    pub const WORKSPACE_DIR_MISSING: &str = "workspace.dirMissing";
    pub const WORKSPACE_FILE_CORRUPT: &str = "workspace.fileCorrupt";

    pub const CREDENTIAL_ENCRYPT_FAILED: &str = "credential.encryptFailed";
    pub const CREDENTIAL_DECRYPT_FAILED: &str = "credential.decryptFailed";
    pub const CREDENTIAL_KEY_FAILED: &str = "credential.keyFailed";
    pub const CREDENTIAL_CORRUPT: &str = "credential.corrupt";

    pub const TUNNEL_ALREADY_RUNNING: &str = "tunnel.alreadyRunning";
    pub const TUNNEL_PORT_UNAVAILABLE: &str = "tunnel.portUnavailable";

    pub const SSH_CONNECTION: &str = "ssh.connection";
    pub const SSH_DISCONNECTED: &str = "ssh.disconnected";

    pub const IO_FAILED: &str = "io.failed";
    pub const INTERNAL_LOCK_POISONED: &str = "internal.lockPoisoned";
}

// —— 通用构造器 ——

pub fn required(field: &str) -> AppError {
    AppError::new(codes::VALIDATION_REQUIRED, format!("{field} is required")).with("field", field)
}

pub fn port_invalid() -> AppError {
    AppError::new(
        codes::VALIDATION_PORT_INVALID,
        "Port must be between 1 and 65535",
    )
}

pub fn timeout(operation: &str, seconds: u64) -> AppError {
    AppError::new(
        codes::TIMEOUT_OPERATION,
        format!("{operation} timed out after {seconds} seconds"),
    )
    .with("operation", operation)
    .with("seconds", seconds)
}

pub fn request_canceled(operation: &str) -> AppError {
    AppError::new(
        codes::SESSION_REQUEST_CANCELED,
        format!("{operation} was canceled"),
    )
    .with("operation", operation)
}

pub fn session_not_running() -> AppError {
    AppError::new(codes::SESSION_NOT_RUNNING, "Terminal session is not running")
}

pub fn not_accepting(kind: &str) -> AppError {
    AppError::new(
        codes::SESSION_NOT_ACCEPTING,
        format!("Terminal session is not accepting {kind}"),
    )
    .with("kind", kind)
}

pub fn root_forbidden(operation: &str) -> AppError {
    AppError::new(
        codes::REMOTE_ROOT_FORBIDDEN,
        format!("Cannot {operation} the root path"),
    )
    .with("operation", operation)
}

pub fn remote_target_exists(path: &str) -> AppError {
    AppError::new(
        codes::REMOTE_TARGET_EXISTS,
        format!("Remote target already exists: {path}"),
    )
    .with("path", path)
}

pub fn lock_poisoned() -> AppError {
    AppError::new(codes::INTERNAL_LOCK_POISONED, "Internal lock was poisoned")
}

/// std::io / serde / 其它宿主侧失败的统一包装。
pub fn io_error(error: impl Display) -> AppError {
    AppError::new(codes::IO_FAILED, error.to_string()).with("detail", error)
}

/// russh / russh-sftp / 网络层错误的统一包装。
///
/// 这里是**唯一**一处按文本判断错误性质的地方：底层库只给出 Display 字符串，
/// 无法结构化。判断结果固化成 `ssh.disconnected` / `ssh.connection` 两个码，
/// 前端只认码，不再自己匹配英文文本。
pub fn connection_error(error: impl Display) -> AppError {
    let message = error.to_string();
    let code = if is_disconnected_signature(&message) {
        codes::SSH_DISCONNECTED
    } else {
        codes::SSH_CONNECTION
    };

    AppError::new(code, message.clone()).with("detail", message)
}

fn is_disconnected_signature(message: &str) -> bool {
    let message = message.to_ascii_lowercase();

    [
        "connection closed",
        "connection reset",
        "connection aborted",
        "broken pipe",
        "lost connection",
        "disconnected",
        "disconnect",
        "not connected",
        "channel is closed",
        "channel closed",
        "session is closed",
        "eof",
    ]
    .iter()
    .any(|signature| message.contains(signature))
}
