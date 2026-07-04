use std::sync::Arc;

use russh::client;
use russh_sftp::client::SftpSession;
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{timeout, Duration};

use crate::models::{
    RemoteCreateDirectoryRequest, RemoteDeleteRequest, RemoteDirectoryListRequest,
    RemoteDirectoryListing, RemoteFileEntry, RemoteRenameRequest, RemoteTransferEntry,
};
use crate::sftp::path::normalize_remote_path;
use crate::sftp::{open_sftp_session, remote_file_entry};
use crate::ssh::client::ConnectionTestClient;
use crate::ssh::registry::TerminalRegistry;
use crate::util::{connection_error, summarize_transfer_errors};

#[tauri::command]
pub async fn list_remote_directory(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDirectoryListRequest,
) -> Result<RemoteDirectoryListing, String> {
    validate_remote_directory_list_request(&request)?;

    timeout(
        Duration::from_secs(15),
        registry.list_directory(
            &request.session_id,
            normalize_remote_path(&request.path),
        ),
    )
    .await
    .map_err(|_| "SFTP directory listing timed out after 15 seconds".to_string())?
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

pub(crate) async fn list_directory_on_session(
    session_id: &str,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    path: &str,
) -> Result<RemoteDirectoryListing, String> {
    let sftp = open_sftp_session(session).await?;
    let requested_path = normalize_remote_path(path);
    let listing_path = sftp
        .canonicalize(requested_path.clone())
        .await
        .unwrap_or(requested_path);
    let entries_result: Result<Vec<RemoteFileEntry>, String> = sftp
        .read_dir(listing_path.clone())
        .await
        .map_err(connection_error)
        .map(|read_dir| read_dir.map(remote_file_entry).collect());

    let _ = sftp.close().await;
    let mut entries = entries_result?;

    entries.sort_by(|left, right| {
        right
            .is_directory
            .cmp(&left.is_directory)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });

    Ok(RemoteDirectoryListing {
        session_id: session_id.to_string(),
        path: listing_path,
        entries,
    })
}

pub(crate) async fn create_directory_on_session(
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    path: &str,
) -> Result<(), String> {
    let sftp = open_sftp_session(session).await?;
    let path = normalize_remote_path(path);

    if path == "/" {
        let _ = sftp.close().await;
        return Err("Cannot create the root directory".to_string());
    }

    let result = async {
        if sftp
            .try_exists(path.clone())
            .await
            .map_err(connection_error)?
        {
            return Err(format!("Remote target already exists: {path}"));
        }

        sftp.create_dir(path).await.map_err(connection_error)
    }
    .await;

    let _ = sftp.close().await;
    result
}

pub(crate) async fn rename_path_on_session(
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    from_path: &str,
    to_path: &str,
) -> Result<(), String> {
    let sftp = open_sftp_session(session).await?;
    let from_path = normalize_remote_path(from_path);
    let to_path = normalize_remote_path(to_path);

    let result = async {
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
    .await;

    let _ = sftp.close().await;
    result
}

pub(crate) async fn delete_paths_on_session(
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    entries: &[RemoteTransferEntry],
    recursive: bool,
) -> Result<(), String> {
    let sftp = open_sftp_session(session).await?;
    let mut errors = Vec::new();

    for entry in entries {
        let path = normalize_remote_path(&entry.path);
        if let Err(error) = delete_remote_path(&sftp, &path, recursive).await {
            errors.push(format!("{}: {error}", entry.name));
        }
    }

    let _ = sftp.close().await;

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

fn validate_remote_directory_list_request(
    request: &RemoteDirectoryListRequest,
) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
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
