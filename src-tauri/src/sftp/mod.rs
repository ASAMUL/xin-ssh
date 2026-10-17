pub(crate) mod delete;
pub(crate) mod error;
pub(crate) mod ops;
pub(crate) mod path;
mod remote_digest;
pub(crate) mod session;
pub(crate) mod transfer;
mod transfer_verify;
mod transfer_metadata;

use std::sync::Arc;

use russh::client;
use russh_sftp::{
    client::{RawSftpSession, SftpSession},
    protocol::{File, FileAttributes, FileType},
};
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{connection_error, AppError};
use crate::models::RemoteFileEntry;
use crate::sftp::error::sftp_error;
use crate::sftp::path::normalize_remote_path;
use crate::ssh::client::ConnectionTestClient;

pub(crate) async fn open_sftp_session(
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
) -> Result<SftpSession, AppError> {
    let channel = {
        let session_handle = session.lock().await;
        session_handle
            .channel_open_session()
            .await
            .map_err(connection_error)?
    };

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(connection_error)?;

    SftpSession::new(channel.into_stream())
        .await
        .map_err(|error| sftp_error(error, ""))
}

pub(crate) async fn open_raw_sftp_session(
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
) -> Result<RawSftpSession, AppError> {
    let channel = {
        let session_handle = session.lock().await;
        session_handle
            .channel_open_session()
            .await
            .map_err(connection_error)?
    };

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(connection_error)?;

    let raw = RawSftpSession::new(channel.into_stream());
    raw.init().await.map_err(|error| sftp_error(error, ""))?;

    Ok(raw)
}

pub(crate) fn remote_file_entry(parent_path: &str, file: &File) -> RemoteFileEntry {
    let name = file.filename.clone();
    let path = normalize_remote_path(&format!("{parent_path}/{name}"));

    remote_file_entry_from_attrs(path, name, &file.attrs)
}

pub(crate) fn remote_file_entry_from_attrs(
    path: String,
    name: String,
    attrs: &FileAttributes,
) -> RemoteFileEntry {
    let file_type = attrs.file_type();
    let permissions_mode = attrs.permissions.map(|mode| mode & 0o777);

    RemoteFileEntry {
        id: path.clone(),
        name: name.clone(),
        path,
        kind: remote_file_kind(file_type).to_string(),
        size: attrs.size,
        modified_at: attrs.mtime.map(u64::from),
        permissions: permissions_mode.map(format_permissions),
        permissions_mode,
        owner: attrs
            .user
            .clone()
            .or_else(|| attrs.uid.map(|uid| uid.to_string())),
        group: attrs
            .group
            .clone()
            .or_else(|| attrs.gid.map(|gid| gid.to_string())),
        is_hidden: name.starts_with('.'),
        is_directory: file_type.is_dir(),
        is_symlink: file_type.is_symlink(),
    }
}

fn remote_file_kind(file_type: FileType) -> &'static str {
    if file_type.is_dir() {
        "directory"
    } else if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_file() {
        "file"
    } else {
        "other"
    }
}

fn format_permissions(mode: u32) -> String {
    [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ]
    .iter()
    .map(|(flag, label)| if mode & flag != 0 { *label } else { '-' })
    .collect()
}
