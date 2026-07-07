use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use russh::client;
use russh_sftp::client::{error::Error as SftpError, RawSftpSession, SftpSession};
use russh_sftp::protocol::StatusCode;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{timeout, Duration};

use crate::models::{
    DirectoryEventPayload, RemoteCreateDirectoryRequest, RemoteDeleteRequest,
    RemoteDirectoryStreamRequest, RemoteFileEntry, RemoteRenameRequest, RemoteTransferEntry,
};
use crate::sftp::path::normalize_remote_path;
use crate::sftp::{open_raw_sftp_session, remote_file_entry};
use crate::ssh::client::ConnectionTestClient;
use crate::ssh::registry::TerminalRegistry;
use crate::util::{connection_error, summarize_transfer_errors};

const DIRECTORY_EVENT: &str = "directory-event";

#[tauri::command]
pub fn stream_remote_directory(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDirectoryStreamRequest,
) -> Result<(), String> {
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
) -> Result<(), String> {
    validate_create_directory_request(&request)?;

    timeout(
        Duration::from_secs(12),
        registry.create_directory(&request.session_id, normalize_remote_path(&request.path)),
    )
    .await
    .map_err(|_| "Create directory request timed out after 12 seconds".to_string())?
}

#[tauri::command]
pub async fn rename_remote_path(
    registry: State<'_, TerminalRegistry>,
    request: RemoteRenameRequest,
) -> Result<(), String> {
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
    .map_err(|_| "Rename request timed out after 12 seconds".to_string())?
}

#[tauri::command]
pub async fn delete_remote_entries(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDeleteRequest,
) -> Result<(), String> {
    validate_delete_request(&request)?;

    timeout(
        Duration::from_secs(30),
        registry.delete_paths(&request.session_id, request.entries, request.recursive),
    )
    .await
    .map_err(|_| "Delete request timed out after 30 seconds".to_string())?
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
) -> Result<(), String> {
    ensure_stream_active(cancellation)?;

    let raw = timeout(Duration::from_secs(12), open_raw_sftp_session(session))
        .await
        .map_err(|_| "Opening SFTP channel timed out after 12 seconds".to_string())??;

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
) -> Result<(), String> {
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
) -> Result<(), String> {
    let path = normalize_remote_path(path);

    if path == "/" {
        return Err("Cannot create the root directory".to_string());
    }

    if sftp
        .try_exists(path.clone())
        .await
        .map_err(connection_error)?
    {
        return Err(format!("Remote target already exists: {path}"));
    }

    sftp.create_dir(path).await.map_err(connection_error)
}

pub(crate) async fn rename_path_on_session(
    sftp: &SftpSession,
    from_path: &str,
    to_path: &str,
) -> Result<(), String> {
    let from_path = normalize_remote_path(from_path);
    let to_path = normalize_remote_path(to_path);

    if from_path == "/" {
        return Err("Cannot rename the root directory".to_string());
    }

    if from_path == to_path {
        return Ok(());
    }

    if sftp
        .try_exists(to_path.clone())
        .await
        .map_err(connection_error)?
    {
        return Err(format!("Remote target already exists: {to_path}"));
    }

    sftp.rename(from_path, to_path)
        .await
        .map_err(connection_error)
}

pub(crate) async fn delete_paths_on_session(
    sftp: &SftpSession,
    entries: &[RemoteTransferEntry],
    recursive: bool,
) -> Result<(), String> {
    let mut errors = Vec::new();

    for entry in entries {
        let path = normalize_remote_path(&entry.path);
        if let Err(error) = delete_remote_path(sftp, &path, recursive).await {
            errors.push(format!("{}: {error}", entry.name));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(summarize_transfer_errors(errors))
    }
}

async fn delete_remote_path(
    sftp: &SftpSession,
    path: &str,
    recursive: bool,
) -> Result<(), String> {
    if path == "/" {
        return Err("Cannot delete the root directory".to_string());
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
) -> Result<(), String> {
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

fn ensure_stream_active(cancellation: &Arc<AtomicBool>) -> Result<(), String> {
    if is_stream_canceled(cancellation) {
        Err("Directory listing canceled".to_string())
    } else {
        Ok(())
    }
}

fn validate_directory_stream_request(
    request: &RemoteDirectoryStreamRequest,
) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.request_id.trim().is_empty() {
        return Err("Request id is required".to_string());
    }

    Ok(())
}

fn validate_create_directory_request(
    request: &RemoteCreateDirectoryRequest,
) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.path.trim().is_empty() {
        return Err("Remote folder path is required".to_string());
    }

    if normalize_remote_path(&request.path) == "/" {
        return Err("Cannot create the root directory".to_string());
    }

    Ok(())
}

fn validate_rename_request(request: &RemoteRenameRequest) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.from_path.trim().is_empty() || request.to_path.trim().is_empty() {
        return Err("Remote path is required".to_string());
    }

    if normalize_remote_path(&request.from_path) == "/" {
        return Err("Cannot rename the root directory".to_string());
    }

    Ok(())
}

fn validate_delete_request(request: &RemoteDeleteRequest) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.entries.is_empty() {
        return Err("Choose at least one remote item".to_string());
    }

    if request
        .entries
        .iter()
        .any(|entry| normalize_remote_path(&entry.path) == "/")
    {
        return Err("Cannot delete the root directory".to_string());
    }

    Ok(())
}
