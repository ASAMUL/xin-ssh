use std::{fmt::Display, fs::Metadata, path::Path, time::UNIX_EPOCH};

use filetime::FileTime;
use russh_sftp::protocol::FileAttributes;
use tokio::{fs, task::spawn_blocking};

use crate::error::{codes, AppError};
use crate::util::path_to_string;

// 只同步约定的权限与时间，不把源文件的 owner、特殊权限位或大小写到目标。
pub(super) fn upload_attributes(metadata: &Metadata, path: &Path) -> Result<FileAttributes, AppError> {
    let modified = metadata.modified().map_err(|error| metadata_error(path_to_string(path), error))?;
    let seconds = modified.duration_since(UNIX_EPOCH)
        .ok().and_then(|duration| u32::try_from(duration.as_secs()).ok())
        .ok_or_else(|| metadata_error(path_to_string(path), "Modification time is outside the SFTP v3 range"))?;
    let mut attributes = FileAttributes::empty();
    attributes.permissions = Some(if metadata.is_dir() || is_executable(metadata) { 0o755 } else { 0o644 });
    attributes.mtime = Some(seconds);
    // SFTP v3 必须成对发送 atime/mtime，缺少 atime 时用 mtime，不能让库补成 1970。
    attributes.atime = Some(metadata.accessed().ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u32::try_from(duration.as_secs()).ok())
        .unwrap_or(seconds));
    Ok(attributes)
}

#[cfg(unix)]
fn is_executable(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_: &Metadata) -> bool {
    false
}

// 通过已写完的文件句柄恢复时间，避免重开路径以及 Windows 文件共享方式的差异。
pub(super) async fn set_local_file_modified(
    file: fs::File,
    path: &Path,
    modified: Option<u32>,
) -> Result<(), AppError> {
    let Some(seconds) = modified else { return Ok(()); };
    let file = file.into_std().await;
    spawn_blocking(move || filetime::set_file_handle_times(
        &file, None, Some(FileTime::from_unix_time(i64::from(seconds), 0)),
    )).await.map_err(|error| metadata_error(path_to_string(path), error))?
        .map_err(|error| metadata_error(path_to_string(path), error))
}

// 目录要等子项创建完再恢复时间；filetime 处理各平台目录句柄，阻塞工作移出异步 worker。
pub(super) async fn set_local_path_modified(path: &Path, modified: Option<u32>) -> Result<(), AppError> {
    let Some(seconds) = modified else { return Ok(()); };
    let target = path.to_path_buf();
    spawn_blocking(move || filetime::set_file_mtime(
        target, FileTime::from_unix_time(i64::from(seconds), 0),
    )).await.map_err(|error| metadata_error(path_to_string(path), error))?
        .map_err(|error| metadata_error(path_to_string(path), error))
}

pub(super) fn metadata_error(path: impl Display, error: impl Display) -> AppError {
    AppError::new(codes::TRANSFER_METADATA_FAILED, format!("Could not preserve file metadata: {error}"))
        .with("path", path).with("detail", error)
}
