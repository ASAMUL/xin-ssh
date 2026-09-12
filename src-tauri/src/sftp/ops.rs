use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::io::SeekFrom;

use russh::client;
use russh_sftp::client::{error::Error as SftpError, RawSftpSession, SftpSession};
use russh_sftp::protocol::{OpenFlags, StatusCode};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{timeout, Duration};

use crate::error::{
    codes, remote_target_exists, required, root_forbidden,
    timeout as timeout_error, AppError,
};
use crate::models::{
    DirectoryEventPayload, RemoteChmodRequest, RemoteCopyRequest, RemoteCreateDirectoryRequest, RemoteCreateFileRequest,
    RemoteDirectoryStreamRequest, RemoteFileContent, RemoteFileEntry, RemoteListDirectoryRequest,
    RemoteReadFileRequest, RemoteRenameRequest, RemoteWriteFileRequest,
};
use crate::sftp::error::{remote_io_error, sftp_error};
use crate::sftp::path::{normalize_remote_path, join_remote_path, remote_parent_path};
use crate::sftp::{open_raw_sftp_session, remote_file_entry, remote_file_entry_from_attrs};
use crate::ssh::client::ConnectionTestClient;
use crate::ssh::registry::TerminalRegistry;

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
    .map_err(|_| modification_timeout("createDirectory", 12))?
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
    .map_err(|_| modification_timeout("createFile", 12))?
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
) -> Result<String, AppError> {
    validate_write_file_request(&request)?;
    // 在命令层确定随机备份路径，接收端超时或取消时仍能向用户报告恢复位置。
    let path = normalize_remote_path(&request.path);
    let random = ring::rand::generate::<[u8; 16]>(&ring::rand::SystemRandom::new())
        .map_err(|_| AppError::new(codes::REMOTE_IO_FAILED, "Could not generate backup name")
            .with("path", &path).with("detail", "Could not generate backup name"))?
        .expose();
    let backup_path = join_remote_path(&remote_parent_path(&path), &format!(".xin-backup-{:032x}", u128::from_le_bytes(random)));
    let result = timeout(
        Duration::from_secs(60),
        registry.write_file_text(
            &request.session_id,
            path,
            request.content,
            request.expected_version,
            backup_path.clone(),
        ),
    )
    .await
    .map_err(|_| modification_timeout("writeFile", 60).with("recoveryPath", &backup_path))?;
    result.map_err(|error| {
        if error.is(codes::SESSION_REQUEST_CANCELED) {
            error.with("recoveryPath", &backup_path)
        } else {
            error
        }
    })
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
    .map_err(|_| modification_timeout("rename", 12))?
}

#[tauri::command]
pub async fn copy_remote_path(
    registry: State<'_, TerminalRegistry>,
    request: RemoteCopyRequest,
) -> Result<(), AppError> {
    validate_copy_request(&request)?;

    timeout(
        Duration::from_secs(120),
        registry.copy_path(RemoteCopyRequest {
            session_id: request.session_id,
            from_path: normalize_remote_path(&request.from_path),
            to_path: normalize_remote_path(&request.to_path),
            recursive: request.recursive,
        }),
    )
    .await
    .map_err(|_| modification_timeout("copy", 120))?
}

#[tauri::command]
pub async fn chmod_remote_path(
    registry: State<'_, TerminalRegistry>,
    request: RemoteChmodRequest,
) -> Result<(), AppError> {
    validate_chmod_request(&request)?;

    timeout(
        Duration::from_secs(12),
        registry.chmod_path(RemoteChmodRequest {
            session_id: request.session_id,
            path: normalize_remote_path(&request.path),
            mode: request.mode,
        }),
    )
    .await
    .map_err(|_| modification_timeout("chmod", 12))?
}

fn modification_timeout(operation: &str, seconds: u64) -> AppError {
    AppError::new(
        codes::REMOTE_OPERATION_TIMED_OUT,
        "Remote operation timed out; refresh before retrying",
    )
    .with("operation", operation)
    .with("seconds", seconds)
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
        .map_err(|error| sftp_error(error, &listing_path))?
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
            Err(error) => break Err(sftp_error(error, &listing_path)),
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
        .map_err(|error| sftp_error(error, &path))?
    {
        return Err(remote_target_exists(&path));
    }

    sftp.create_dir(path.clone())
        .await
        .map_err(|error| sftp_error(error, &path))
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
        .map_err(|error| sftp_error(error, &path))?
    {
        return Err(remote_target_exists(&path));
    }

    let mut file = sftp
        .open_with_flags(
            path.clone(),
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(|error| sftp_error(error, &path))?;

    file.shutdown()
        .await
        .map_err(|error| remote_io_error(error, &path))
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
        .map_err(|error| sftp_error(error, &path))?;

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

    let mut file = sftp.open(path.clone()).await.map_err(|error| sftp_error(error, &path))?;
    let mut bytes = Vec::with_capacity(size as usize);
    let read_result = (&mut file).take(MAX_EDITABLE_FILE_BYTES + 1).read_to_end(&mut bytes).await;
    let _ = file.shutdown().await;
    read_result.map_err(|error| remote_io_error(error, &path))?;

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
        version: file_content_version(content.as_bytes()),
        content,
    })
}

pub(crate) async fn write_file_text_on_session(
    sftp: Arc<SftpSession>,
    path: &str,
    content: &str,
    expected_version: &str,
    backup_path: &str,
) -> Result<String, AppError> {
    let path = normalize_remote_path(path);

    if path == "/" {
        return Err(root_forbidden("writeFile"));
    }

    // 先通过同一文件句柄校验旧内容并保存独立备份，断网或取消时原内容仍可恢复；
    // 最后写回原句柄，保留属主、权限以及软链接/硬链接语义。
    let mut file = sftp
        .open_with_flags(
            path.clone(),
            OpenFlags::READ | OpenFlags::WRITE,
        )
        .await
        .map_err(|error| sftp_error(error, &path))?;

    let metadata = file.metadata().await.map_err(|error| sftp_error(error, &path))?;
    if !metadata.file_type().is_file() {
        return Err(AppError::new(codes::REMOTE_NOT_REGULAR_FILE, "Only regular files can be saved").with("path", &path));
    }
    let original = read_editor_bytes(&mut file, &path).await?;
    if file_content_version(&original) != expected_version {
        return Err(file_changed_error(&path));
    }
    let mut attributes = russh_sftp::protocol::FileAttributes::empty();
    attributes.permissions = Some(0o600);
    let mut backup = sftp.open_with_flags_and_attributes(
        backup_path.to_string(), OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE, attributes,
    ).await.map_err(|error| sftp_error(error, backup_path))?;
    // 某些服务器忽略创建权限；确认备份不会暴露原文件里的凭据后才写入内容。
    if !backup.metadata().await.map_err(|error| sftp_error(error, backup_path))?
        .permissions.is_some_and(|mode| mode & 0o077 == 0)
    {
        return Err(AppError::new(codes::REMOTE_BACKUP_UNSAFE, "Server did not restrict backup access"));
    }
    write_editor_bytes(&mut backup, backup_path, &original).await?;
    backup.shutdown().await.map_err(|error| remote_io_error(error, backup_path))?;

    let result = async {
        // 备份往返期间文件可能被其他程序修改，再核对一次才能开始破坏性写入。
        if file_content_version(&read_editor_bytes(&mut file, &path).await?) != expected_version {
            return Err(file_changed_error(&path));
        }
        let mut truncate = russh_sftp::protocol::FileAttributes::empty();
        truncate.size = Some(0);
        file.set_metadata(truncate).await.map_err(|error| sftp_error(error, &path))?;
        file.seek(SeekFrom::Start(0)).await.map_err(|error| remote_io_error(error, &path))?;
        write_editor_bytes(&mut file, &path, content.as_bytes()).await?;
        if let Some(permissions) = metadata.permissions {
            let mut attributes = russh_sftp::protocol::FileAttributes::empty();
            attributes.permissions = Some(permissions);
            file.set_metadata(attributes).await.map_err(|error| sftp_error(error, &path))?;
        }
        let version = file_content_version(content.as_bytes());
        if file_content_version(&read_editor_bytes(&mut file, &path).await?) != version {
            return Err(file_changed_error(&path));
        }
        // 外部编辑器常用 rename 替换原文件；旧句柄写入成功不代表当前路径已保存。
        let mut current = sftp.open(path.clone()).await.map_err(|error| sftp_error(error, &path))?;
        if !current.metadata().await.map_err(|error| sftp_error(error, &path))?.file_type().is_file()
            || file_content_version(&read_editor_bytes(&mut current, &path).await?) != version
        {
            return Err(file_changed_error(&path));
        }
        current.shutdown().await.map_err(|error| remote_io_error(error, &path))?;
        file.shutdown().await.map_err(|error| remote_io_error(error, &path))?;
        Ok(version)
    }.await.map_err(|error: AppError| error.with("recoveryPath", backup_path));
    if result.is_ok() {
        // 清理不能占用保存的剩余时限，否则成功写入也会因删除备份超时而被误报失败。
        let cleanup_sftp = Arc::clone(&sftp);
        let cleanup_path = backup_path.to_string();
        tauri::async_runtime::spawn(async move {
            let _ = timeout(Duration::from_secs(5), cleanup_sftp.remove_file(cleanup_path)).await;
        });
    }
    result
}

fn file_content_version(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes).as_ref()
        .iter().map(|byte| format!("{byte:02x}")).collect()
}

fn file_changed_error(path: &str) -> AppError {
    AppError::new(codes::REMOTE_FILE_CHANGED, "Remote file changed since it was read").with("path", path)
}

async fn read_editor_bytes(file: &mut russh_sftp::client::fs::File, path: &str) -> Result<Vec<u8>, AppError> {
    file.seek(SeekFrom::Start(0)).await.map_err(|error| remote_io_error(error, path))?;
    let mut bytes = Vec::new();
    (&mut *file).take(MAX_EDITABLE_FILE_BYTES + 1).read_to_end(&mut bytes)
        .await.map_err(|error| remote_io_error(error, path))?;
    if bytes.len() as u64 > MAX_EDITABLE_FILE_BYTES {
        return Err(editable_file_too_large_error());
    }
    Ok(bytes)
}

async fn write_editor_bytes(file: &mut russh_sftp::client::fs::File, path: &str, bytes: &[u8]) -> Result<(), AppError> {
    for chunk in bytes.chunks(EDITOR_WRITE_CHUNK_BYTES) {
        file.write_all(chunk).await.map_err(|error| remote_io_error(error, path))?;
    }
    file.flush().await.map_err(|error| remote_io_error(error, path))?;
    file.sync_all()
        .await
        .map_err(|error| sftp_error(error, path))
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
        .map_err(|error| sftp_error(error, &listing_path))?;

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
        .map_err(|error| sftp_error(error, &to_path))?
    {
        return Err(remote_target_exists(&to_path));
    }

    sftp.rename(from_path.clone(), to_path.clone())
        .await
        .map_err(|error| sftp_error(error, &from_path).with("target", &to_path))
}

// 复制走固定块流式传输，避免大文件一次性读入内存；目录递归前先检查目标路径，防止自复制和覆盖现有数据。
pub(crate) async fn copy_path_on_session(
    sftp: &SftpSession,
    from_path: &str,
    to_path: &str,
    recursive: bool,
) -> Result<(), AppError> {
    let from_path = normalize_remote_path(from_path);
    let to_path = normalize_remote_path(to_path);

    if from_path == "/" || to_path == "/" {
        return Err(root_forbidden("copy"));
    }
    if from_path == to_path || to_path.starts_with(&format!("{from_path}/")) {
        return Err(AppError::new(
            codes::REMOTE_IO_FAILED,
            "Copy destination cannot be inside the source",
        )
        .with("path", &to_path));
    }
    if sftp
        .try_exists(to_path.clone())
        .await
        .map_err(|error| sftp_error(error, &to_path))?
    {
        return Err(remote_target_exists(&to_path));
    }

    let metadata = sftp
        .metadata(from_path.clone())
        .await
        .map_err(|error| sftp_error(error, &from_path))?;
    if metadata.file_type().is_dir() && !recursive {
        return Err(AppError::new(
            codes::REMOTE_IS_DIRECTORY,
            format!("Remote path is a directory: {from_path}"),
        )
        .with("path", &from_path));
    }
    if !metadata.file_type().is_dir() && !metadata.file_type().is_file() {
        return Err(AppError::new(
            codes::REMOTE_NOT_REGULAR_FILE,
            format!("Remote path is not a regular file: {from_path}"),
        )
        .with("path", &from_path));
    }

    let mut pending = vec![(from_path, to_path, metadata, false)];
    while let Some((source_path, target_path, entry_metadata, metadata_only)) = pending.pop() {
      if entry_metadata.file_type().is_dir() && !metadata_only {
        sftp.create_dir(target_path.clone())
            .await
            .map_err(|error| sftp_error(error, &target_path))?;
        let entries = sftp
            .read_dir(source_path.clone())
            .await
            .map_err(|error| sftp_error(error, &source_path))?;
        // 栈是后进先出：先登记目录元数据，再压入子项，确保权限在所有子项复制完成后恢复。
        pending.push((source_path.clone(), target_path.clone(), entry_metadata, true));
        for entry in entries {
            let name = entry.file_name();
            let child_from = normalize_remote_path(&format!("{source_path}/{name}"));
            let child_to = normalize_remote_path(&format!("{target_path}/{name}"));
            pending.push((child_from, child_to, entry.metadata(), false));
        }
        // 元数据已交给回访任务，本轮不能继续使用或提前应用目录权限。
        continue;
      } else {
        if !metadata_only {
            let mut source = sftp
                .open(source_path.clone())
                .await
                .map_err(|error| sftp_error(error, &source_path))?;
            let mut target = sftp
                .open_with_flags(
                    target_path.clone(),
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                )
                .await
                .map_err(|error| sftp_error(error, &target_path))?;
            let mut buffer = vec![0u8; EDITOR_WRITE_CHUNK_BYTES];
            loop {
                let read = source
                    .read(&mut buffer)
                    .await
                    .map_err(|error| remote_io_error(error, &source_path))?;
                if read == 0 {
                    break;
                }
                target
                    .write_all(&buffer[..read])
                    .await
                    .map_err(|error| remote_io_error(error, &target_path))?;
            }
            target
                .shutdown()
                .await
                .map_err(|error| remote_io_error(error, &target_path))?;
        }
      }

    if let Some(permissions) = entry_metadata.permissions {
        sftp.set_metadata(
            target_path.to_string(),
            russh_sftp::client::fs::Metadata {
                size: None,
                uid: None,
                user: None,
                gid: None,
                group: None,
                permissions: Some(permissions),
                atime: None,
                mtime: None,
            },
        )
        .await
        .map_err(|error| sftp_error(error, &target_path))?;
    }
    }
    Ok(())
}

pub(crate) async fn chmod_path_on_session(
    sftp: &SftpSession,
    path: &str,
    mode: u32,
) -> Result<(), AppError> {
    let path = normalize_remote_path(path);
    if path == "/" {
        return Err(root_forbidden("chmod"));
    }
    let metadata = sftp
        .metadata(path.clone())
        .await
        .map_err(|error| sftp_error(error, &path))?;
    sftp.set_metadata(
        path.clone(),
        russh_sftp::client::fs::Metadata {
            size: None,
            uid: None,
            user: None,
            gid: None,
            group: None,
            permissions: Some((metadata.permissions.unwrap_or_default() & !0o777) | mode),
            atime: None,
            mtime: None,
        },
    )
    .await
    .map_err(|error| sftp_error(error, &path))
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
    if request.expected_version.is_empty() {
        return Err(file_changed_error(&request.path));
    }
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

fn validate_copy_request(request: &RemoteCopyRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    if request.from_path.trim().is_empty() || request.to_path.trim().is_empty() {
        return Err(required("remotePath"));
    }
    if normalize_remote_path(&request.from_path) == "/" || normalize_remote_path(&request.to_path) == "/" {
        return Err(root_forbidden("copy"));
    }
    Ok(())
}

fn validate_chmod_request(request: &RemoteChmodRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    if request.path.trim().is_empty() {
        return Err(required("remotePath"));
    }
    if request.mode > 0o777 {
        return Err(AppError::new(codes::REMOTE_INVALID_PERMISSIONS, "Mode must be between 000 and 777")
            .with("mode", request.mode));
    }
    if normalize_remote_path(&request.path) == "/" {
        return Err(root_forbidden("chmod"));
    }
    Ok(())
}
