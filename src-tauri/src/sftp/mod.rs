pub(crate) mod ops;
pub(crate) mod path;
pub(crate) mod transfer;

use std::sync::Arc;

use russh::client;
use russh_sftp::{
    client::{fs::DirEntry, SftpSession},
    protocol::FileType,
};
use tokio::sync::Mutex as AsyncMutex;

use crate::models::RemoteFileEntry;
use crate::sftp::path::normalize_remote_path;
use crate::ssh::client::ConnectionTestClient;
use crate::util::connection_error;

pub(crate) async fn open_sftp_session(
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
) -> Result<SftpSession, String> {
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
        .map_err(connection_error)
}

pub(crate) fn remote_file_entry(entry: DirEntry) -> RemoteFileEntry {
    let metadata = entry.metadata();
    let file_type = entry.file_type();
    let name = entry.file_name();
    let path = normalize_remote_path(&entry.path());
    let permissions_mode = metadata.permissions.map(|mode| mode & 0o777);

    RemoteFileEntry {
        id: path.clone(),
        name: name.clone(),
        path,
        kind: remote_file_kind(file_type).to_string(),
        size: metadata.size,
        modified_at: metadata.mtime.map(u64::from),
        permissions: permissions_mode.map(format_permissions),
        permissions_mode,
        owner: metadata.user.or_else(|| metadata.uid.map(|uid| uid.to_string())),
        group: metadata.group.or_else(|| metadata.gid.map(|gid| gid.to_string())),
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
