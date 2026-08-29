use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use russh::client;
use russh_sftp::client::{error::Error as SftpError, RawSftpSession, SftpSession};
use russh_sftp::protocol::{OpenFlags, StatusCode};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{timeout, Duration};

use crate::error::{
    codes, connection_error, io_error, remote_target_exists, required, root_forbidden,
    timeout as timeout_error, AppError,
};
use crate::models::{
    DirectoryEventPayload, RemoteCreateDirectoryRequest, RemoteCreateFileRequest,
    RemoteDeleteRequest, RemoteDirectoryStreamRequest, RemoteFileContent, RemoteFileEntry,
    RemoteListDirectoryRequest, RemoteReadFileRequest, RemoteRenameRequest, RemoteTransferEntry,
    RemoteWriteFileRequest,
};
use crate::sftp::path::normalize_remote_path;
use crate::sftp::{open_raw_sftp_session, remote_file_entry, remote_file_entry_from_attrs};
use crate::ssh::client::ConnectionTestClient;
use crate::ssh::registry::TerminalRegistry;
use crate::util::summarize_entry_errors;

const DIRECTORY_EVENT: &str = "directory-event";
// 内置编辑器单文件上限：内容走内存与 IPC 字符串往返，超限引导用户下载后编辑
const MAX_EDITABLE_FILE_BYTES: u64 = 5 * 1024 * 1024;
// 与 transfer.rs 的传输缓冲一致，分块写避免单次 SFTP 包过大
const EDITOR_WRITE_CHUNK_BYTES: usize = 256 * 1024;

fn editable_file_too_large_error() -> AppError {
    let limit_mb = MAX_EDITABLE_FILE_BYTES / (1024 * 1024);
    AppError::new(
        codes::FILE_TOO_LARGE,
        format!("File is too large to edit (limit {limit_mb} MB)"),
    )
    .with("limitMb", limit_mb)
}

#[tauri::command]
pub fn stream_remote_directory(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDirectoryStreamRequest,
) -> Result<(), AppError> {
    validate_directory_stream_request(&request)?;

    registry.stream_directory(
        &request.session_id,
        request.request_id,
        normalize_remote_path(&request.path),
    )
}

#[tauri::command]
pub async fn create_remote_directory(
    registry: State<'_, TerminalRegistry>,
    request: RemoteCreateDirectoryRequest,
) -> Result<(), AppError> {
    validate_create_directory_request(&request)?;

    timeout(
        Duration::from_secs(12),
        registry.create_directory(&request.session_id, normalize_remote_path(&request.path)),
    )
    .await
    .map_err(|_| timeout_error("createDirectory", 12))?
}

#[tauri::command]
pub async fn create_remote_file(
    registry: State<'_, TerminalRegistry>,
    request: RemoteCreateFileRequest,
) -> Result<(), AppError> {
    validate_create_file_request(&request)?;

    timeout(
        Duration::from_secs(12),
        registry.create_file(&request.session_id, normalize_remote_path(&request.path)),
    )
    .await
    .map_err(|_| timeout_error("createFile", 12))?
}

#[tauri::command]
pub async fn list_remote_directory(
    registry: State<'_, TerminalRegistry>,
    request: RemoteListDirectoryRequest,
) -> Result<Vec<RemoteFileEntry>, AppError> {
    validate_list_directory_request(&request)?;

    timeout(
        Duration::from_secs(20),
        registry.list_directory(&request.session_id, normalize_remote_path(&request.path)),
    )
    .await
    .map_err(|_| timeout_error("listDirectory", 20))?
}

#[tauri::command]
pub async fn read_remote_file_text(
    registry: State<'_, TerminalRegistry>,
    request: RemoteReadFileRequest,
) -> Result<RemoteFileContent, AppError> {
    validate_read_file_request(&request)?;

    timeout(
        Duration::from_secs(60),
        registry.read_file_text(&request.session_id, normalize_remote_path(&request.path)),
    )
    .await
    .map_err(|_| timeout_error("readFile", 60))?
}

#[tauri::command]
pub async fn write_remote_file_text(
    registry: State<'_, TerminalRegistry>,
    request: RemoteWriteFileRequest,
) -> Result<(), AppError> {
    validate_write_file_request(&request)?;

    timeout(
        Duration::from_secs(60),
        registry.write_file_text(
            &request.session_id,
            normalize_remote_path(&request.path),
            request.content,
        ),
    )
    .await
    .map_err(|_| timeout_error("writeFile", 60))?
}

#[tauri::command]
pub async fn rename_remote_path(
    registry: State<'_, TerminalRegistry>,
    request: RemoteRenameRequest,
) -> Result<(), AppError> {
    validate_rename_request(&request)?;

    timeout(
        Duration::from_secs(12),
        registry.rename_path(
            &request.session_id,
            normalize_remote_path(&request.from_path),
            normalize_remote_path(&request.to_path),
        ),
    )
    .await
    .map_err(|_| timeout_error("rename", 12))?
}

#[tauri::command]
pub async fn delete_remote_entries(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDeleteRequest,
) -> Result<(), AppError> {
    validate_delete_request(&request)?;

    timeout(
        Duration::from_secs(30),
        registry.delete_paths(&request.session_id, request.entries, request.recursive),
    )
    .await
    .map_err(|_| timeout_error("delete", 30))?
}

pub(crate) async fn stream_directory_on_session(
    app: &AppHandle,
    session_id: &str,
    request_id: &str,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    path: &str,
    cancellation: &Arc<AtomicBool>,
) {
    let result = run_directory_stream(app, session_id, request_id, session, path, cancellation).await;

    if is_stream_canceled(cancellation) {
        emit_directory_event(
            app,
            directory_event(session_id, request_id, "canceled", path.to_string()),
        );
        return;
    }

    if let Err(error) = result {
        let mut event = directory_event(session_id, request_id, "failed", path.to_string());
        event.error = Some(error);
        emit_directory_event(app, event);
    }
}

async fn run_directory_stream(
    app: &AppHandle,
    session_id: &str,
    request_id: &str,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    path: &str,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), AppError> {
    ensure_stream_active(cancellation)?;

    let raw = timeout(Duration::from_secs(12), open_raw_sftp_session(session))
        .await
        .map_err(|_| timeout_error("openSftpChannel", 12))??;

    let result =
        stream_directory_entries(app, session_id, request_id, &raw, path, cancellation).await;
    let _ = raw.close_session();
    result
}

async fn stream_directory_entries(
    app: &AppHandle,
    session_id: &str,
    request_id: &str,
    raw: &RawSftpSession,
    requested_path: &str,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), AppError> {
    let listing_path = match raw.realpath(requested_path.to_string()).await {
        Ok(name) => name
            .files
            .first()
            .map(|file| file.filename.clone())
            .unwrap_or_else(|| requested_path.to_string()),
        Err(_) => requested_path.to_string(),
    };
    let listing_path = normalize_remote_path(&listing_path);

    ensure_stream_active(cancellation)?;
    let handle = raw
        .opendir(listing_path.clone())
        .await
        .map_err(connection_error)?
        .handle;

    emit_directory_event(
        app,
        directory_event(session_id, request_id, "started", listing_path.clone()),
    );

    let mut total: u64 = 0;
    let read_result = loop {
        if is_stream_canceled(cancellation) {
            break Ok(());
        }

        match raw.readdir(handle.as_str()).await {
            Ok(name) => {
                let entries: Vec<RemoteFileEntry> = name
                    .files
                    .iter()
                    .filter(|file| file.filename != "." && file.filename != "..")
                    .map(|file| remote_file_entry(&listing_path, file))
                    .collect();

                if entries.is_empty() {
                    continue;
                }

                total += entries.len() as u64;
                let mut event =
                    directory_event(session_id, request_id, "chunk", listing_path.clone());
                event.entries = entries;
                emit_directory_event(app, event);
            }
            Err(SftpError::Status(status)) if status.status_code == StatusCode::Eof => {
                break Ok(());
            }
            Err(error) => break Err(connection_error(error)),
        }
    };

    let _ = raw.close(handle).await;
    read_result?;
    ensure_stream_active(cancellation)?;

    let mut event = directory_event(session_id, request_id, "completed", listing_path);
    event.total = Some(total);
    emit_directory_event(app, event);

    Ok(())
}

pub(crate) async fn create_directory_on_session(
    sftp: &SftpSession,
    path: &str,
) -> Result<(), AppError> {
    let path = normalize_remote_path(path);

    if path == "/" {
        return Err(root_forbidden("createDirectory"));
    }

    if sftp
        .try_exists(path.clone())
        .await
        .map_err(connection_error)?
    {
        return Err(remote_target_exists(&path));
    }

    sftp.create_dir(path).await.map_err(connection_error)
}

pub(crate) async fn create_file_on_session(
    sftp: &SftpSession,
    path: &str,
) -> Result<(), AppError> {
    let path = normalize_remote_path(path);

    if path == "/" {
        return Err(root_forbidden("createFile"));
    }

    if sftp
        .try_exists(path.clone())
        .await
        .map_err(connection_error)?
    {
        return Err(remote_target_exists(&path));
    }

    let mut file = sftp
        .open_with_flags(
            path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(connection_error)?;

    file.shutdown().await.map_err(io_error)
}

pub(crate) async fn read_file_text_on_session(
    sftp: &SftpSession,
    path: &str,
) -> Result<RemoteFileContent, AppError> {
    let path = normalize_remote_path(path);
    // metadata 跟随软链接：双击指向目录的软链接时报 isDirectory，前端据此回退为目录导航
    let metadata = sftp
        .metadata(path.clone())
        .await
        .map_err(connection_error)?;

    if metadata.file_type().is_dir() {
        return Err(AppError::new(
            codes::REMOTE_IS_DIRECTORY,
            format!("Remote path is a directory: {path}"),
        )
        .with("path", &path));
    }

    // FIFO/设备文件读取可能永久阻塞并占死共享 SFTP 通道，仅允许常规文件
    if !metadata.file_type().is_file() {
        return Err(AppError::new(
            codes::REMOTE_NOT_REGULAR_FILE,
            format!("Remote path is not a regular file: {path}"),
        )
        .with("path", &path));
    }

    let size = metadata.size.unwrap_or(0);
    if size > MAX_EDITABLE_FILE_BYTES {
        return Err(editable_file_too_large_error());
    }

    let mut file = sftp.open(path.clone()).await.map_err(connection_error)?;
    let mut bytes = Vec::with_capacity(size as usize);
    let read_result = file.read_to_end(&mut bytes).await;
    let _ = file.shutdown().await;
    read_result.map_err(io_error)?;

    // /proc 等虚拟文件 metadata 报 0，按实际读到的字节数兜底校验
    if bytes.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(editable_file_too_large_error());
    }

    if bytes.contains(&0) {
        return Err(AppError::new(
            codes::FILE_BINARY,
            "File appears to be binary and cannot be edited as text",
        ));
    }

    let content = String::from_utf8(bytes)
        .map_err(|_| AppError::new(codes::FILE_NOT_UTF8, "File is not valid UTF-8 text"))?;

    Ok(RemoteFileContent {
        path,
        size: content.len() as u64,
        content,
    })
}

pub(crate) async fn write_file_text_on_session(
    sftp: &SftpSession,
    path: &str,
    content: &str,
) -> Result<(), AppError> {
    let path = normalize_remote_path(path);

    if path == "/" {
        return Err(root_forbidden("writeFile"));
    }

    // 就地截断重写以保留权限与属主；不走“临时文件+改名”，SFTP rename 无法覆盖且会重置属主
    let mut file = sftp
        .open_with_flags(
            path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(connection_error)?;

    for chunk in content.as_bytes().chunks(EDITOR_WRITE_CHUNK_BYTES) {
        file.write_all(chunk).await.map_err(io_error)?;
    }

    file.flush().await.map_err(io_error)?;
    file.shutdown().await.map_err(io_error)
}

// 目录树用的一次性列目录：走会话内缓存 SFTP 子通道，与流式面板列表互不影响
pub(crate) async fn list_directory_on_session(
    sftp: &SftpSession,
    path: &str,
) -> Result<Vec<RemoteFileEntry>, AppError> {
    let listing_path = normalize_remote_path(path);
    let read_dir = sftp
        .read_dir(listing_path.clone())
        .await
        .map_err(connection_error)?;

    Ok(read_dir
        .into_iter()
        .map(|child| {
            let child_path = normalize_remote_path(&child.path());
            let name = child_path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            remote_file_entry_from_attrs(child_path, name, &child.metadata())
        })
        .collect())
}

pub(crate) async fn rename_path_on_session(
    sftp: &SftpSession,
    from_path: &str,
    to_path: &str,
) -> Result<(), AppError> {
    let from_path = normalize_remote_path(from_path);
    let to_path = normalize_remote_path(to_path);

    if from_path == "/" {
        return Err(root_forbidden("rename"));
    }

    if from_path == to_path {
        return Ok(());
    }

    if sftp
        .try_exists(to_path.clone())
        .await
        .map_err(connection_error)?
    {
        return Err(remote_target_exists(&to_path));
    }

    sftp.rename(from_path, to_path)
        .await
        .map_err(connection_error)
}

pub(crate) async fn delete_paths_on_session(
    sftp: &SftpSession,
    entries: &[RemoteTransferEntry],
    recursive: bool,
) -> Result<(), AppError> {
    let mut errors = Vec::new();

    for entry in entries {
        let path = normalize_remote_path(&entry.path);
        if let Err(error) = delete_remote_path(sftp, &path, recursive).await {
            errors.push((entry.name.clone(), error));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(summarize_entry_errors(errors))
    }
}

async fn delete_remote_path(
    sftp: &SftpSession,
    path: &str,
    recursive: bool,
) -> Result<(), AppError> {
    if path == "/" {
        return Err(root_forbidden("delete"));
    }

    let metadata = sftp
        .symlink_metadata(path.to_string())
        .await
        .map_err(connection_error)?;

    if metadata.file_type().is_dir() {
        if recursive {
            delete_remote_directory_recursive(sftp, path).await
        } else {
            sftp.remove_dir(path.to_string())
                .await
                .map_err(connection_error)
        }
    } else {
        sftp.remove_file(path.to_string())
            .await
            .map_err(connection_error)
    }
}

async fn delete_remote_directory_recursive(
    sftp: &SftpSession,
    root_path: &str,
) -> Result<(), AppError> {
    let mut pending_dirs = vec![normalize_remote_path(root_path)];
    let mut dirs_to_remove = Vec::new();

    while let Some(current_dir) = pending_dirs.pop() {
        dirs_to_remove.push(current_dir.clone());
        let read_dir = sftp
            .read_dir(current_dir.clone())
            .await
            .map_err(connection_error)?;

        for child in read_dir {
            let child_path = normalize_remote_path(&child.path());
            if child.metadata().file_type().is_dir() {
                pending_dirs.push(child_path);
            } else {
                sftp.remove_file(child_path)
                    .await
                    .map_err(connection_error)?;
            }
        }
    }

    for dir in dirs_to_remove.into_iter().rev() {
        sftp.remove_dir(dir).await.map_err(connection_error)?;
    }

    Ok(())
}

fn directory_event(
    session_id: &str,
    request_id: &str,
    kind: &str,
    path: String,
) -> DirectoryEventPayload {
    DirectoryEventPayload {
        session_id: session_id.to_string(),
        request_id: request_id.to_string(),
        kind: kind.to_string(),
        path,
        entries: Vec::new(),
        total: None,
        error: None,
    }
}

fn emit_directory_event(app: &AppHandle, event: DirectoryEventPayload) {
    let _ = app.emit(DIRECTORY_EVENT, event);
}

fn is_stream_canceled(cancellation: &Arc<AtomicBool>) -> bool {
    cancellation.load(Ordering::SeqCst)
}

fn ensure_stream_active(cancellation: &Arc<AtomicBool>) -> Result<(), AppError> {
    if is_stream_canceled(cancellation) {
        Err(AppError::new(
            codes::DIRECTORY_LISTING_CANCELED,
            "Directory listing canceled",
        ))
    } else {
        Ok(())
    }
}

fn validate_directory_stream_request(
    request: &RemoteDirectoryStreamRequest,
) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.request_id.trim().is_empty() {
        return Err(required("requestId"));
    }

    Ok(())
}

fn validate_create_directory_request(
    request: &RemoteCreateDirectoryRequest,
) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.path.trim().is_empty() {
        return Err(required("remotePath"));
    }

    if normalize_remote_path(&request.path) == "/" {
        return Err(root_forbidden("createDirectory"));
    }

    Ok(())
}

fn validate_create_file_request(request: &RemoteCreateFileRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.path.trim().is_empty() {
        return Err(required("remotePath"));
    }

    if normalize_remote_path(&request.path) == "/" {
        return Err(root_forbidden("createFile"));
    }

    Ok(())
}

fn validate_list_directory_request(request: &RemoteListDirectoryRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.path.trim().is_empty() {
        return Err(required("remotePath"));
    }

    Ok(())
}

fn validate_read_file_request(request: &RemoteReadFileRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.path.trim().is_empty() {
        return Err(required("remotePath"));
    }

    Ok(())
}

fn validate_write_file_request(request: &RemoteWriteFileRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.path.trim().is_empty() {
        return Err(required("remotePath"));
    }

    if normalize_remote_path(&request.path) == "/" {
        return Err(root_forbidden("writeFile"));
    }

    if request.content.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(editable_file_too_large_error());
    }

    Ok(())
}

fn validate_rename_request(request: &RemoteRenameRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.from_path.trim().is_empty() || request.to_path.trim().is_empty() {
        return Err(required("remotePath"));
    }

    if normalize_remote_path(&request.from_path) == "/" {
        return Err(root_forbidden("rename"));
    }

    Ok(())
}

fn validate_delete_request(request: &RemoteDeleteRequest) -> Result<(), AppError> {
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

    Ok(())
}
