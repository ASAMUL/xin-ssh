use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, MutexGuard},
    time::UNIX_EPOCH,
};

use russh_sftp::{
    client::{error::Error as SftpError, SftpSession},
    protocol::StatusCode,
};
use tauri::State;
use tokio::fs as async_fs;
use tokio::time::{timeout, Duration};

use crate::error::{required, timeout as timeout_error, AppError};
use crate::models::{DiscardPartialFilesRequest, PartialFile};
use crate::ssh::registry::TerminalRegistry;

const DISCARD_TIMEOUT_SECS: u64 = 30;
// 断线后每个 SFTP 请求都要等到超时，收尾不能把失败提示拖上几十秒
const SETTLE_STEP_TIMEOUT_SECS: u64 = 3;

enum Tracked {
    // 这次传输打开写过、还没核验完的目标
    Touched { full_size: u64 },
    // 之前的尝试留下、这次还没碰过的半成品
    Known(PartialFile),
}

impl Tracked {
    // 连不上服务器时留下的记录：没有属性可比，删除时只能要求它比完整文件小
    fn unverified(&self, path: &str) -> PartialFile {
        match self {
            Self::Touched { full_size } => PartialFile {
                path: path.to_string(),
                full_size: *full_size,
                size: None,
                modified: None,
            },
            Self::Known(file) => file.clone(),
        }
    }
}

// 一次传输涉及的半成品目标。用户取消时把它们删掉；出错时保留并报给前端，
// 方便续传，也让用户删记录时能顺手清理。
pub(super) struct PartialFiles {
    entries: Mutex<HashMap<String, Tracked>>,
}

impl PartialFiles {
    pub(super) fn new(known: &[PartialFile]) -> Self {
        let entries = known
            .iter()
            .map(|file| (file.path.clone(), Tracked::Known(file.clone())))
            .collect();
        Self { entries: Mutex::new(entries) }
    }

    // 开始往目标里写时调用（续传要等前缀核对通过），直到整个文件核验完成都算半成品
    pub(super) fn touch(&self, path: &str, full_size: u64) {
        self.lock().insert(path.to_string(), Tracked::Touched { full_size });
    }

    pub(super) fn finish(&self, path: &str) {
        self.lock().remove(path);
    }

    fn take(&self) -> Vec<(String, Tracked)> {
        self.lock().drain().collect()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Tracked>> {
        self.entries.lock().unwrap_or_else(|error| error.into_inner())
    }

    // 返回结束后仍留在服务器上的半成品：出错时全部保留，取消时只剩删不掉的。
    pub(super) async fn settle_remote(&self, sftp: &SftpSession, discard: bool) -> Vec<PartialFile> {
        let mut left = Vec::new();
        let mut reachable = true;
        for (path, tracked) in self.take() {
            let fallback = tracked.unverified(&path);
            let remaining = if reachable {
                let step = settle_remote_one(sftp, path, tracked, discard);
                match timeout(Duration::from_secs(SETTLE_STEP_TIMEOUT_SECS), step).await {
                    Ok(remaining) => remaining,
                    Err(_) => {
                        // 一次超时就当连接已断，剩下的不再碰服务器
                        reachable = false;
                        Some(fallback)
                    }
                }
            } else {
                Some(fallback)
            };
            left.extend(remaining);
        }
        left
    }

    pub(super) async fn settle_local(&self, discard: bool) -> Vec<PartialFile> {
        let mut left = Vec::new();
        for (path, tracked) in self.take() {
            let remaining = match (tracked, discard) {
                (Tracked::Touched { full_size }, true) => match async_fs::remove_file(&path).await {
                    Ok(()) => None,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(_) => local_snapshot(path, full_size).await,
                },
                (Tracked::Touched { full_size }, false) => local_snapshot(path, full_size).await,
                (Tracked::Known(file), true) => discard_local_file(file).await,
                (Tracked::Known(file), false) => Some(file),
            };
            left.extend(remaining);
        }
        left
    }
}

async fn settle_remote_one(
    sftp: &SftpSession,
    path: String,
    tracked: Tracked,
    discard: bool,
) -> Option<PartialFile> {
    match (tracked, discard) {
        // 这次自己写的，不用比对，直接删
        (Tracked::Touched { full_size }, true) => match sftp.remove_file(path.clone()).await {
            Ok(()) => None,
            Err(error) if is_missing(&error) => None,
            Err(_) => remote_snapshot(sftp, path, full_size).await,
        },
        (Tracked::Touched { full_size }, false) => remote_snapshot(sftp, path, full_size).await,
        (Tracked::Known(file), true) => discard_remote_file(sftp, file).await,
        (Tracked::Known(file), false) => Some(file),
    }
}

// 用户删掉传输记录时清理它留下的半成品，返回没删掉的（之后还能再试）。
#[tauri::command]
pub async fn discard_partial_files(
    registry: State<'_, TerminalRegistry>,
    request: DiscardPartialFilesRequest,
) -> Result<Vec<PartialFile>, AppError> {
    match request.direction.as_str() {
        "download" => {
            let mut left = Vec::new();
            for file in request.files {
                left.extend(discard_local_file(file).await);
            }
            Ok(left)
        }
        "upload" => {
            let session_id = request
                .session_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| required("sessionId"))?;
            timeout(
                Duration::from_secs(DISCARD_TIMEOUT_SECS),
                registry.discard_partial_files(session_id, request.files),
            )
            .await
            .map_err(|_| timeout_error("discardPartialFiles", DISCARD_TIMEOUT_SECS))?
        }
        _ => Err(required("direction")),
    }
}

pub(crate) async fn discard_remote_files(sftp: &SftpSession, files: Vec<PartialFile>) -> Vec<PartialFile> {
    let mut left = Vec::new();
    for file in files {
        left.extend(discard_remote_file(sftp, file).await);
    }
    left
}

// 返回 Some 表示还在、但这次没删成；已经不在或已不是当时的半成品都算处理完。
async fn discard_remote_file(sftp: &SftpSession, file: PartialFile) -> Option<PartialFile> {
    if !file.path.starts_with('/') {
        return None;
    }
    let attributes = match sftp.symlink_metadata(file.path.clone()).await {
        Ok(attributes) => attributes,
        Err(error) if is_missing(&error) => return None,
        Err(_) => return Some(file),
    };
    if !attributes.file_type().is_file()
        || !still_partial(&file, attributes.size, attributes.mtime.map(u64::from))
    {
        return None;
    }
    match sftp.remove_file(file.path.clone()).await {
        Ok(()) => None,
        Err(error) if is_missing(&error) => None,
        Err(_) => Some(file),
    }
}

async fn discard_local_file(file: PartialFile) -> Option<PartialFile> {
    if !Path::new(&file.path).is_absolute() {
        return None;
    }
    let metadata = match async_fs::symlink_metadata(&file.path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => return Some(file),
    };
    if !metadata.is_file() || !still_partial(&file, Some(metadata.len()), modified_seconds(&metadata)) {
        return None;
    }
    match async_fs::remove_file(&file.path).await {
        Ok(()) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(file),
    }
}

// 断线时读不到属性，留一份只有完整大小的记录：比完整文件小才可能是我们的半成品。
async fn remote_snapshot(sftp: &SftpSession, path: String, full_size: u64) -> Option<PartialFile> {
    match sftp.symlink_metadata(path.clone()).await {
        Ok(attributes) if attributes.file_type().is_file() => Some(PartialFile {
            path,
            full_size,
            size: attributes.size,
            modified: attributes.mtime.map(u64::from),
        }),
        Ok(_) => None,
        Err(error) if is_missing(&error) => None,
        Err(_) => Some(PartialFile { path, full_size, size: None, modified: None }),
    }
}

async fn local_snapshot(path: String, full_size: u64) -> Option<PartialFile> {
    match async_fs::symlink_metadata(&path).await {
        Ok(metadata) if metadata.is_file() => Some(PartialFile {
            full_size,
            size: Some(metadata.len()),
            modified: modified_seconds(&metadata),
            path,
        }),
        Ok(_) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(PartialFile { path, full_size, size: None, modified: None }),
    }
}

// 记下了大小和修改时间就要求都没变；没记下（断线时）只能要求它比完整文件小，
// 完整大小的文件可能是别人后来重新传好的，不能删。
fn still_partial(file: &PartialFile, size: Option<u64>, modified: Option<u64>) -> bool {
    let Some(size) = size else { return false };
    match (file.size, file.modified) {
        (Some(expected_size), Some(expected_modified)) => {
            size == expected_size && modified == Some(expected_modified)
        }
        _ => size < file.full_size,
    }
}

fn modified_seconds(metadata: &std::fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn is_missing(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::NoSuchFile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partial(size: Option<u64>, modified: Option<u64>) -> PartialFile {
        PartialFile { path: "/srv/a.bin".to_string(), full_size: 100, size, modified }
    }

    #[test]
    fn a_recorded_snapshot_must_match_exactly() {
        let file = partial(Some(40), Some(1_700_000_000));
        assert!(still_partial(&file, Some(40), Some(1_700_000_000)));
        assert!(!still_partial(&file, Some(41), Some(1_700_000_000)));
        assert!(!still_partial(&file, Some(40), Some(1_700_000_001)));
    }

    #[test]
    fn without_a_snapshot_only_files_smaller_than_the_source_count() {
        let file = partial(None, None);
        assert!(still_partial(&file, Some(99), None));
        assert!(!still_partial(&file, Some(100), Some(1_700_000_000)));
        assert!(!still_partial(&file, None, None));
    }
}
