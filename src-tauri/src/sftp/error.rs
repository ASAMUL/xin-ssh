use std::io;

use russh_sftp::{client::error::Error as SftpError, protocol::StatusCode};

use crate::error::{codes, connection_error, AppError};

// SFTP 状态码不依赖服务器语言，权限和路径错误不能触发前端的 SSH 断线处理。
pub(crate) fn sftp_error(error: SftpError, path: &str) -> AppError {
    let code = match &error {
        SftpError::Status(status) => match status.status_code {
            StatusCode::PermissionDenied => codes::REMOTE_PERMISSION_DENIED,
            StatusCode::NoSuchFile => codes::REMOTE_NOT_FOUND,
            StatusCode::NoConnection | StatusCode::ConnectionLost => codes::SSH_DISCONNECTED,
            StatusCode::OpUnsupported => codes::SFTP_UNSUPPORTED,
            _ => codes::SFTP_FAILED,
        },
        SftpError::Timeout => codes::SFTP_TIMED_OUT,
        SftpError::IO(_) | SftpError::UnexpectedBehavior(_) => {
            if connection_error(&error).is(codes::SSH_DISCONNECTED) {
                codes::SSH_DISCONNECTED
            } else {
                codes::SFTP_FAILED
            }
        }
        _ => codes::SFTP_FAILED,
    };
    let message = if path.is_empty() { error.to_string() } else { format!("{path}: {error}") };
    AppError::new(code, message)
        .with("path", path)
        .with("detail", error)
}

// 库的 AsyncRead/AsyncWrite 会把状态码转成字符串，不能凭服务器文案猜权限；保留远端路径和原因。
pub(crate) fn remote_io_error(error: io::Error, path: &str) -> AppError {
    let code = match error.kind() {
        io::ErrorKind::PermissionDenied => codes::REMOTE_PERMISSION_DENIED,
        io::ErrorKind::NotFound => codes::REMOTE_NOT_FOUND,
        io::ErrorKind::TimedOut => codes::SFTP_TIMED_OUT,
        io::ErrorKind::BrokenPipe
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset | io::ErrorKind::NotConnected => codes::SSH_DISCONNECTED,
        _ if connection_error(&error).is(codes::SSH_DISCONNECTED) => codes::SSH_DISCONNECTED,
        _ => codes::REMOTE_IO_FAILED,
    };
    AppError::new(code, format!("{path}: {error}"))
        .with("path", path)
        .with("detail", error)
}
