use std::sync::Arc;

use russh::client;
use russh_sftp::client::SftpSession;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{timeout, Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::error::{
    codes, required, root_forbidden, timeout as timeout_error, AppError,
};
use crate::models::{FileOperationEventPayload, RemoteDeleteRequest};
use crate::sftp::error::sftp_error;
use crate::sftp::{open_sftp_session, path::normalize_remote_path};
use crate::ssh::{client::ConnectionTestClient, registry::TerminalRegistry};
use crate::util::summarize_entry_errors;

const FILE_OPERATION_EVENT: &str = "file-operation-event";

#[tauri::command]
pub async fn start_remote_delete(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDeleteRequest,
) -> Result<(), AppError> {
    if request.task_id.trim().is_empty() {
        return Err(required("taskId"));
    }
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    if request.entries.is_empty() {
        return Err(AppError::new(
            codes::VALIDATION_SELECT_REMOTE_ITEM,
            "Choose at least one remote item",
        ));
    }
    if request
        .entries
        .iter()
        .any(|entry| normalize_remote_path(&entry.path) == "/")
    {
        return Err(root_forbidden("delete"));
    }
    registry.start_delete(request).await
}

#[tauri::command]
pub fn cancel_remote_delete(
    registry: State<'_, TerminalRegistry>,
    task_id: String,
) -> Result<(), AppError> {
    registry.cancel_delete(&task_id)
}

// 删除没有总时限，单请求仍受 SFTP 超时保护；独立子通道保证大目录不阻塞编辑器和目录树。
pub(crate) async fn run_delete_task(
    app: AppHandle,
    session: Arc<Mutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteDeleteRequest,
    cancellation: CancellationToken,
    permits: Arc<Semaphore>,
) {
    let mut progress = DeleteProgress {
        event: FileOperationEventPayload {
            task_id: request.task_id.clone(),
            session_id: request.session_id.clone(),
            kind: "started".to_string(),
            current_path: None,
            completed_entries: 0,
            error: None,
        },
        last_emit: Instant::now(),
    };
    let mut sftp = None;
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(delete_canceled()),
        result = async {
            let _permit = permits.acquire().await.map_err(|_| delete_canceled())?;
            progress.emit(&app, "started", true);
            let channel = sftp.insert(timeout(Duration::from_secs(12), open_sftp_session(session))
                .await.map_err(|_| timeout_error("openSftpChannel", 12))??);
            let mut errors = Vec::new();
            for entry in &request.entries {
                let path = normalize_remote_path(&entry.path);
                if let Err(error) = delete_path(channel, &path, request.recursive, &app, &mut progress).await {
                    errors.push((entry.name.clone(), error));
                }
            }
            if errors.is_empty() { Ok(()) } else { Err(summarize_entry_errors(errors)) }
        } => result,
    };
    // 取消只停止后续请求，已发送的远端请求可能完成；进度只统计收到成功响应的删除。
    if let Some(sftp) = sftp {
        let _ = sftp.close().await;
    }
    let kind = match result {
        Ok(()) => "completed",
        Err(error) if error.is(codes::FILE_OPERATION_CANCELED) => "canceled",
        Err(error) => {
            progress.event.error = Some(error);
            "failed"
        }
    };
    progress.emit(&app, kind, true);
}

// 边遍历边删除，避免预扫描整棵树后才有进度；使用显式栈避免深目录递归溢出。
async fn delete_path(
    sftp: &SftpSession,
    root: &str,
    recursive: bool,
    app: &AppHandle,
    progress: &mut DeleteProgress,
) -> Result<(), AppError> {
    let mut pending = vec![(root.to_string(), false)];
    while let Some((path, remove_directory)) = pending.pop() {
        if path == "/" {
            return Err(root_forbidden("delete"));
        }
        progress.event.current_path = Some(path.clone());
        progress.emit(app, "progress", false);
        if remove_directory {
            sftp.remove_dir(path.clone())
                .await
                .map_err(|error| sftp_error(error, &path))?;
        } else {
            // lstat 不跟随软链接，递归删除不得沿链接进入用户未选中的目录。
            let metadata = sftp
                .symlink_metadata(path.clone())
                .await
                .map_err(|error| sftp_error(error, &path))?;
            if metadata.file_type().is_dir() {
                if recursive {
                    let children = sftp
                        .read_dir(path.clone())
                        .await
                        .map_err(|error| sftp_error(error, &path))?;
                    pending.push((path, true));
                    for child in children {
                        // 远端文件名允许反斜杠和尾部空格，规范化会把它改成另一个删除目标。
                        pending.push((child.path(), false));
                    }
                    continue;
                }
                sftp.remove_dir(path.clone())
                    .await
                    .map_err(|error| sftp_error(error, &path))?;
            } else {
                sftp.remove_file(path.clone())
                    .await
                    .map_err(|error| sftp_error(error, &path))?;
            }
        }
        progress.event.completed_entries += 1;
        progress.emit(app, "progress", false);
    }
    Ok(())
}

struct DeleteProgress {
    event: FileOperationEventPayload,
    last_emit: Instant,
}

impl DeleteProgress {
    fn emit(&mut self, app: &AppHandle, kind: &str, force: bool) {
        if !force && self.last_emit.elapsed() < Duration::from_millis(120) {
            return;
        }
        self.event.kind = kind.to_string();
        let _ = app.emit(FILE_OPERATION_EVENT, self.event.clone());
        self.last_emit = Instant::now();
    }
}

fn delete_canceled() -> AppError {
    AppError::new(codes::FILE_OPERATION_CANCELED, "File operation was canceled")
}
