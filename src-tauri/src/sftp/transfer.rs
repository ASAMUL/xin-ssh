use std::{
    collections::{HashMap, HashSet},
    io::SeekFrom,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use futures::stream::{self, StreamExt};
use russh::client;
use russh_sftp::{
    client::{error::Error as SftpError, SftpSession},
    protocol::{OpenFlags, StatusCode},
};
use tauri::{AppHandle, Emitter, State};
use tokio::time::{Duration, Instant};
use tokio::{
    fs as async_fs,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::Mutex as AsyncMutex,
};

use crate::error::{
    codes, io_error, lock_poisoned, required, AppError,
};
use crate::models::{
    RemoteDownloadRequest, RemoteTransferEntry, RemoteUploadRequest, TransferEventPayload,
};
use crate::sftp::error::{remote_io_error, sftp_error};
use crate::sftp::open_sftp_session;
use crate::sftp::path::{
    join_remote_path, local_file_name, normalize_remote_path, remote_basename, remote_parent_path,
    safe_local_name,
};
use crate::ssh::client::ConnectionTestClient;
use crate::ssh::registry::TerminalRegistry;
use crate::util::{path_to_string, summarize_entry_errors};

const TRANSFER_EVENT: &str = "transfer-event";
const TRANSFER_BUFFER_SIZE: usize = 256 * 1024;
const TRANSFER_CONCURRENCY: usize = 4;

fn transfer_canceled_error() -> AppError {
    AppError::new(codes::TRANSFER_CANCELED, "Transfer canceled")
}

#[tauri::command]
pub fn start_upload_transfer(
    registry: State<'_, TerminalRegistry>,
    request: RemoteUploadRequest,
) -> Result<(), AppError> {
    validate_upload_request(&request)?;
    registry.start_upload(request)
}

#[tauri::command]
pub fn start_download_transfer(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDownloadRequest,
) -> Result<(), AppError> {
    validate_download_request(&request)?;
    registry.start_download(request)
}

#[tauri::command]
pub fn cancel_transfer(
    registry: State<'_, TerminalRegistry>,
    task_id: String,
) -> Result<(), AppError> {
    if task_id.trim().is_empty() {
        return Err(required("taskId"));
    }

    registry.cancel_transfer(&task_id)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransferConflictPolicy {
    Fail,
    Overwrite,
    Skip,
    Rename,
    // 断点续传：目标已存在时从已有字节续，size 已够则整个跳过
    Resume,
}

pub(crate) async fn run_upload_transfer(
    app: AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteUploadRequest,
    cancellation: Arc<AtomicBool>,
) {
    if let Err(error) = upload_transfer(&app, session, request.clone(), cancellation).await {
        emit_transfer_event(
            &app,
            transfer_event(
                &request.task_id,
                &request.session_id,
                "upload",
                transfer_error_kind(&error),
            )
            .with_error(error),
        );
    }
}
pub(crate) async fn run_download_transfer(
    app: AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteDownloadRequest,
    cancellation: Arc<AtomicBool>,
) {
    if let Err(error) = download_transfer(&app, session, request.clone(), cancellation).await {
        emit_transfer_event(
            &app,
            transfer_event(
                &request.task_id,
                &request.session_id,
                "download",
                transfer_error_kind(&error),
            )
            .with_error(error),
        );
    }
}

async fn upload_transfer(
    app: &AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteUploadRequest,
    cancellation: Arc<AtomicBool>,
) -> Result<(), AppError> {
    let sftp = open_sftp_session(session).await?;
    let result = upload_transfer_with_sftp(app, &sftp, &request, &cancellation).await;
    let _ = sftp.close().await;
    result
}

async fn upload_transfer_with_sftp(
    app: &AppHandle,
    sftp: &SftpSession,
    request: &RemoteUploadRequest,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), AppError> {
    ensure_transfer_active(cancellation)?;
    let mut plan = plan_local_upload(&request.local_paths, &request.remote_dir).await?;
    apply_remote_conflict_policy(
        sftp,
        &mut plan,
        parse_transfer_conflict_policy(&request.conflict_policy)?,
    )
    .await?;

    let progress = Mutex::new(TransferProgress::new(
        &request.task_id,
        &request.session_id,
        "upload",
        plan.bytes_total,
        plan.files.len() as u64 + plan.resumed_files_done,
        transfer_label_from_local_paths(&request.local_paths),
    ));
    // 续传起步：进度直接落在已就位的字节/文件数上，不从零跳变
    with_progress(&progress, |p| {
        p.bytes_transferred = plan.resumed_bytes;
        p.files_done = plan.resumed_files_done;
        p.emit(app, "started", None, true)
    })?;

    for directory in &plan.directories {
        if is_transfer_canceled(cancellation) {
            with_progress(&progress, |p| p.cancel(app))?;
            return Ok(());
        }

        ensure_remote_directory(sftp, directory).await?;
    }

    let progress = &progress;
    let files = std::mem::take(&mut plan.files);
    let file_results: Vec<Result<(), (String, AppError)>> = stream::iter(files)
        .map(|file| async move {
            let result = async {
                ensure_transfer_active(cancellation)?;
                with_progress(progress, |p| p.label = file.label.clone())?;
                upload_file(sftp, &file, app, progress, cancellation).await
            }
            .await;

            // 失败/取消保留远端半成品，之后可用 resume 策略续传
            if let Err(error) = result {
                return Err((file.label.clone(), error));
            }

            Ok(())
        })
        .buffer_unordered(TRANSFER_CONCURRENCY)
        .collect()
        .await;

    finish_transfer(app, progress, cancellation, file_results)
}

async fn download_transfer(
    app: &AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteDownloadRequest,
    cancellation: Arc<AtomicBool>,
) -> Result<(), AppError> {
    let sftp = open_sftp_session(session).await?;
    let result = download_transfer_with_sftp(app, &sftp, &request, &cancellation).await;
    let _ = sftp.close().await;
    result
}

async fn download_transfer_with_sftp(
    app: &AppHandle,
    sftp: &SftpSession,
    request: &RemoteDownloadRequest,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), AppError> {
    ensure_transfer_active(cancellation)?;
    let mut plan = plan_remote_download(sftp, &request.remote_entries, &request.local_dir).await?;
    apply_local_conflict_policy(
        &mut plan,
        parse_transfer_conflict_policy(&request.conflict_policy)?,
    )?;

    let progress = Mutex::new(TransferProgress::new(
        &request.task_id,
        &request.session_id,
        "download",
        plan.bytes_total,
        plan.files.len() as u64 + plan.resumed_files_done,
        transfer_label_from_remote_entries(&request.remote_entries),
    ));
    with_progress(&progress, |p| {
        p.bytes_transferred = plan.resumed_bytes;
        p.files_done = plan.resumed_files_done;
        p.emit(app, "started", None, true)
    })?;

    for directory in &plan.directories {
        if is_transfer_canceled(cancellation) {
            with_progress(&progress, |p| p.cancel(app))?;
            return Ok(());
        }

        async_fs::create_dir_all(directory)
            .await
            .map_err(io_error)?;
    }

    let progress = &progress;
    let files = std::mem::take(&mut plan.files);
    let file_results: Vec<Result<(), (String, AppError)>> = stream::iter(files)
        .map(|file| async move {
            let result = async {
                ensure_transfer_active(cancellation)?;
                with_progress(progress, |p| p.label = file.label.clone())?;
                download_file(sftp, &file, app, progress, cancellation).await
            }
            .await;

            // 失败/取消保留本地半成品，之后可用 resume 策略续传
            if let Err(error) = result {
                return Err((file.label.clone(), error));
            }

            Ok(())
        })
        .buffer_unordered(TRANSFER_CONCURRENCY)
        .collect()
        .await;

    finish_transfer(app, progress, cancellation, file_results)
}

async fn upload_file(
    sftp: &SftpSession,
    file: &UploadFilePlan,
    app: &AppHandle,
    progress: &Mutex<TransferProgress>,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), AppError> {
    let mut local_file = async_fs::File::open(&file.local_path)
        .await
        .map_err(io_error)?;
    // 续传（offset > 0）：不 TRUNCATE，双端 seek 到断点；整传保持覆盖语义
    let open_flags = if file.offset > 0 {
        OpenFlags::CREATE | OpenFlags::WRITE
    } else {
        OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE
    };
    let mut remote_file = sftp
        .open_with_flags(file.remote_path.clone(), open_flags)
        .await
        .map_err(|error| sftp_error(error, &file.remote_path))?;
    if file.offset > 0 {
        local_file
            .seek(SeekFrom::Start(file.offset))
            .await
            .map_err(io_error)?;
        remote_file
            .seek(SeekFrom::Start(file.offset))
            .await
            .map_err(|error| remote_io_error(error, &file.remote_path))?;
    }
    let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];

    loop {
        ensure_transfer_active(cancellation)?;
        let read = local_file.read(&mut buffer).await.map_err(io_error)?;
        if read == 0 {
            break;
        }

        remote_file
            .write_all(&buffer[..read])
            .await
            .map_err(|error| remote_io_error(error, &file.remote_path))?;
        with_progress(progress, |p| {
            p.bytes_transferred += read as u64;
            p.emit(app, "progress", Some(file.remote_path.clone()), false);
        })?;
    }

    remote_file.flush().await.map_err(|error| remote_io_error(error, &file.remote_path))?;
    remote_file.shutdown()
        .await
        .map_err(|error| remote_io_error(error, &file.remote_path))?;
    with_progress(progress, |p| {
        p.files_done += 1;
        // 不强制发送：海量小文件时靠节流合并，最终计数由 completed 事件兜底
        p.emit(app, "progress", Some(file.remote_path.clone()), false);
    })?;
    Ok(())
}

async fn download_file(
    sftp: &SftpSession,
    file: &DownloadFilePlan,
    app: &AppHandle,
    progress: &Mutex<TransferProgress>,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), AppError> {
    if let Some(parent) = file.local_path.parent() {
        async_fs::create_dir_all(parent).await.map_err(io_error)?;
    }

    let mut remote_file = sftp
        .open(file.remote_path.clone())
        .await
        .map_err(|error| sftp_error(error, &file.remote_path))?;
    // 续传：本地不截断，双端 seek 到断点；整传保持 create 覆盖语义
    let mut local_file = if file.offset > 0 {
        async_fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&file.local_path)
            .await
            .map_err(io_error)?
    } else {
        async_fs::File::create(&file.local_path)
            .await
            .map_err(io_error)?
    };
    if file.offset > 0 {
        remote_file
            .seek(SeekFrom::Start(file.offset))
            .await
            .map_err(|error| remote_io_error(error, &file.remote_path))?;
        local_file
            .seek(SeekFrom::Start(file.offset))
            .await
            .map_err(io_error)?;
    }
    let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];

    loop {
        ensure_transfer_active(cancellation)?;
        let read = remote_file.read(&mut buffer)
            .await
            .map_err(|error| remote_io_error(error, &file.remote_path))?;
        if read == 0 {
            break;
        }

        local_file
            .write_all(&buffer[..read])
            .await
            .map_err(io_error)?;
        with_progress(progress, |p| {
            p.bytes_transferred += read as u64;
            p.emit(app, "progress", Some(file.remote_path.clone()), false);
        })?;
    }

    local_file.flush().await.map_err(io_error)?;
    remote_file.shutdown()
        .await
        .map_err(|error| remote_io_error(error, &file.remote_path))?;
    with_progress(progress, |p| {
        p.files_done += 1;
        p.emit(app, "progress", Some(file.remote_path.clone()), false);
    })?;
    Ok(())
}

fn finish_transfer(
    app: &AppHandle,
    progress: &Mutex<TransferProgress>,
    cancellation: &Arc<AtomicBool>,
    file_results: Vec<Result<(), (String, AppError)>>,
) -> Result<(), AppError> {
    let mut canceled = false;
    let mut errors = Vec::new();

    for file_result in file_results {
        if let Err((label, error)) = file_result {
            if error.is(codes::TRANSFER_CANCELED) {
                canceled = true;
            } else {
                errors.push((label, error));
            }
        }
    }

    if canceled || is_transfer_canceled(cancellation) {
        with_progress(progress, |p| p.cancel(app))?;
        return Ok(());
    }

    if !errors.is_empty() {
        let summary = summarize_entry_errors(errors);
        with_progress(progress, |p| p.fail(app, summary))?;
        return Ok(());
    }

    with_progress(progress, |p| p.emit(app, "completed", None, true))?;
    Ok(())
}

fn with_progress<T>(
    progress: &Mutex<TransferProgress>,
    action: impl FnOnce(&mut TransferProgress) -> T,
) -> Result<T, AppError> {
    let mut guard = progress.lock().map_err(|_| lock_poisoned())?;
    Ok(action(&mut guard))
}

struct UploadPlan {
    root_targets: Vec<String>,
    directories: Vec<String>,
    files: Vec<UploadFilePlan>,
    bytes_total: u64,
    // resume 策略下已就位的部分：进度条从这里起步，不回零
    resumed_bytes: u64,
    resumed_files_done: u64,
}

struct UploadFilePlan {
    local_path: PathBuf,
    remote_path: String,
    label: String,
    size: u64,
    // 断点续传起点（远端已有字节数），0 = 从头传
    offset: u64,
}

struct DownloadPlan {
    root_targets: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    files: Vec<DownloadFilePlan>,
    bytes_total: u64,
    resumed_bytes: u64,
    resumed_files_done: u64,
}

struct DownloadFilePlan {
    remote_path: String,
    local_path: PathBuf,
    label: String,
    size: u64,
    // 断点续传起点（本地已有字节数），0 = 从头下
    offset: u64,
}

async fn apply_remote_conflict_policy(
    sftp: &SftpSession,
    plan: &mut UploadPlan,
    policy: TransferConflictPolicy,
) -> Result<(), AppError> {
    // resume 不看根目标是否存在，而是逐文件比对远端已有字节
    if policy == TransferConflictPolicy::Resume {
        return apply_remote_resume(sftp, plan).await;
    }

    let conflicts = existing_remote_targets(sftp, &plan.root_targets).await?;
    if conflicts.is_empty() {
        return Ok(());
    }

    match policy {
        TransferConflictPolicy::Fail => {
            let first = conflicts
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            Err(crate::error::remote_target_exists(&first))
        }
        TransferConflictPolicy::Overwrite => Ok(()),
        TransferConflictPolicy::Skip => {
            filter_upload_plan_roots(plan, &conflicts);
            Ok(())
        }
        TransferConflictPolicy::Rename => rename_upload_plan_roots(sftp, plan, &conflicts).await,
        // 已在函数开头 early return，此分支不可达
        TransferConflictPolicy::Resume => Ok(()),
    }
}

fn apply_local_conflict_policy(
    plan: &mut DownloadPlan,
    policy: TransferConflictPolicy,
) -> Result<(), AppError> {
    if policy == TransferConflictPolicy::Resume {
        return apply_local_resume(plan);
    }

    let conflicts = existing_local_targets(&plan.root_targets);
    if conflicts.is_empty() {
        return Ok(());
    }

    match policy {
        TransferConflictPolicy::Fail => {
            let first = conflicts
                .iter()
                .next()
                .map(|path| path_to_string(path))
                .unwrap_or_else(|| "unknown".to_string());
            Err(
                AppError::new(
                    codes::LOCAL_TARGET_EXISTS,
                    format!("Local target already exists: {first}"),
                )
                .with("path", &first),
            )
        }
        TransferConflictPolicy::Overwrite => Ok(()),
        TransferConflictPolicy::Skip => {
            filter_download_plan_roots(plan, &conflicts);
            Ok(())
        }
        TransferConflictPolicy::Rename => rename_download_plan_roots(plan, &conflicts),
        // 已在函数开头 early return，此分支不可达
        TransferConflictPolicy::Resume => Ok(()),
    }
}

// 上传向续传：远端已有同名文件时，size 已够 → 整个跳过（计入已完成），
// 不够 → 从远端已有字节续；不存在或非普通文件 → 照常整传
async fn apply_remote_resume(sftp: &SftpSession, plan: &mut UploadPlan) -> Result<(), AppError> {
    let mut kept_files = Vec::with_capacity(plan.files.len());

    for mut file in std::mem::take(&mut plan.files) {
        match sftp.metadata(file.remote_path.clone()).await {
            Ok(metadata) if metadata.file_type().is_file() => {
                let remote_size = metadata.size.unwrap_or(0);
                if remote_size >= file.size {
                    plan.resumed_files_done += 1;
                    plan.resumed_bytes += file.size;
                    continue;
                }

                file.offset = remote_size;
                plan.resumed_bytes += remote_size;
                kept_files.push(file);
            }
            Ok(_) => kept_files.push(file),
            Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => {
                kept_files.push(file);
            }
            // 读不到属性不等于目标不存在，不能在权限或网络出错后退化为截断重传。
            Err(error) => return Err(sftp_error(error, &file.remote_path)),
        }
    }

    plan.files = kept_files;
    Ok(())
}

// 下载向续传：本地已有同名文件时按其大小续；size 已够 → 跳过
fn apply_local_resume(plan: &mut DownloadPlan) -> Result<(), AppError> {
    let mut kept_files = Vec::with_capacity(plan.files.len());

    for mut file in std::mem::take(&mut plan.files) {
        match std::fs::metadata(&file.local_path) {
            Ok(metadata) if metadata.is_file() => {
                let local_size = metadata.len();
                if local_size >= file.size {
                    plan.resumed_files_done += 1;
                    plan.resumed_bytes += file.size;
                    continue;
                }

                file.offset = local_size;
                plan.resumed_bytes += local_size;
                kept_files.push(file);
            }
            Ok(_) => kept_files.push(file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => kept_files.push(file),
            Err(error) => return Err(io_error(error)),
        }
    }

    plan.files = kept_files;
    Ok(())
}

async fn existing_remote_targets(
    sftp: &SftpSession,
    root_targets: &[String],
) -> Result<HashSet<String>, AppError> {
    let mut conflicts = HashSet::new();

    for target in root_targets {
        if sftp
            .try_exists(target.clone())
            .await
            .map_err(|error| sftp_error(error, target))?
        {
            conflicts.insert(target.clone());
        }
    }

    Ok(conflicts)
}

fn existing_local_targets(root_targets: &[PathBuf]) -> HashSet<PathBuf> {
    root_targets
        .iter()
        .filter(|target| target.exists())
        .cloned()
        .collect()
}

fn filter_upload_plan_roots(plan: &mut UploadPlan, skipped_roots: &HashSet<String>) {
    plan.root_targets
        .retain(|target| !skipped_roots.contains(target));
    plan.directories
        .retain(|path| !skipped_roots.iter().any(|root| remote_path_contains(root, path)));
    plan.files
        .retain(|file| !skipped_roots.iter().any(|root| remote_path_contains(root, &file.remote_path)));
    plan.bytes_total = plan.files.iter().map(|file| file.size).sum();
}

fn filter_download_plan_roots(plan: &mut DownloadPlan, skipped_roots: &HashSet<PathBuf>) {
    plan.root_targets
        .retain(|target| !skipped_roots.contains(target));
    plan.directories
        .retain(|path| !skipped_roots.iter().any(|root| local_path_contains(root, path)));
    plan.files
        .retain(|file| !skipped_roots.iter().any(|root| local_path_contains(root, &file.local_path)));
    plan.bytes_total = plan.files.iter().map(|file| file.size).sum();
}

async fn rename_upload_plan_roots(
    sftp: &SftpSession,
    plan: &mut UploadPlan,
    conflicting_roots: &HashSet<String>,
) -> Result<(), AppError> {
    let mut replacements = Vec::new();

    for root in &plan.root_targets {
        if conflicting_roots.contains(root) {
            replacements.push((root.clone(), unique_remote_target(sftp, root).await?));
        }
    }

    apply_remote_replacements(&mut plan.root_targets[..], &replacements);
    apply_remote_replacements(&mut plan.directories[..], &replacements);
    for file in &mut plan.files {
        file.remote_path = replace_remote_root(&file.remote_path, &replacements);
    }

    Ok(())
}

fn rename_download_plan_roots(
    plan: &mut DownloadPlan,
    conflicting_roots: &HashSet<PathBuf>,
) -> Result<(), AppError> {
    let mut replacements = Vec::new();

    for root in &plan.root_targets {
        if conflicting_roots.contains(root) {
            replacements.push((root.clone(), unique_local_target(root)?));
        }
    }

    apply_local_replacements(&mut plan.root_targets[..], &replacements);
    apply_local_replacements(&mut plan.directories[..], &replacements);
    for file in &mut plan.files {
        file.local_path = replace_local_root(&file.local_path, &replacements);
    }

    Ok(())
}

async fn unique_remote_target(sftp: &SftpSession, root: &str) -> Result<String, AppError> {
    for index in 1..1000 {
        let candidate = remote_conflict_variant(root, index);
        if !sftp
            .try_exists(candidate.clone())
            .await
            .map_err(|error| sftp_error(error, &candidate))?
        {
            return Ok(candidate);
        }
    }

    Err(AppError::new(
        codes::REMOTE_UNIQUE_NAME_FAILED,
        format!("Could not create a unique remote name for {root}"),
    )
    .with("path", root))
}

fn unique_local_target(root: &Path) -> Result<PathBuf, AppError> {
    for index in 1..1000 {
        let candidate = local_conflict_variant(root, index);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::new(
        codes::LOCAL_UNIQUE_NAME_FAILED,
        format!(
            "Could not create a unique local name for {}",
            path_to_string(root)
        ),
    )
    .with("path", path_to_string(root)))
}

fn apply_remote_replacements(paths: &mut [String], replacements: &[(String, String)]) {
    for path in paths {
        *path = replace_remote_root(path, replacements);
    }
}

fn apply_local_replacements(paths: &mut [PathBuf], replacements: &[(PathBuf, PathBuf)]) {
    for path in paths {
        *path = replace_local_root(path, replacements);
    }
}

fn replace_remote_root(path: &str, replacements: &[(String, String)]) -> String {
    replacements
        .iter()
        .filter(|(from, _)| remote_path_contains(from, path))
        .max_by_key(|(from, _)| from.len())
        .map(|(from, to)| {
            if path == from {
                to.clone()
            } else {
                format!("{to}/{}", &path[from.len() + 1..])
            }
        })
        .unwrap_or_else(|| path.to_string())
}

fn replace_local_root(path: &Path, replacements: &[(PathBuf, PathBuf)]) -> PathBuf {
    replacements
        .iter()
        .filter(|(from, _)| local_path_contains(from, path))
        .max_by_key(|(from, _)| from.to_string_lossy().len())
        .map(|(from, to)| {
            if path == from {
                to.clone()
            } else {
                path.strip_prefix(from)
                    .map(|suffix| to.join(suffix))
                    .unwrap_or_else(|_| path.to_path_buf())
            }
        })
        .unwrap_or_else(|| path.to_path_buf())
}

fn remote_path_contains(root: &str, path: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}

fn local_path_contains(root: &Path, path: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn remote_conflict_variant(root: &str, index: usize) -> String {
    let parent = remote_parent_path(root);
    let name = remote_basename(root);
    let variant = conflict_name_variant(&name, index);

    join_remote_path(&parent, &variant)
}

fn local_conflict_variant(root: &Path, index: usize) -> PathBuf {
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "remote-item".to_string());
    let variant = conflict_name_variant(&name, index);

    root.parent()
        .map(|parent| parent.join(&variant))
        .unwrap_or_else(|| PathBuf::from(variant))
}

fn conflict_name_variant(name: &str, index: usize) -> String {
    let suffix = if index == 1 {
        " copy".to_string()
    } else {
        format!(" copy {index}")
    };

    let Some(dot_index) = name.rfind('.') else {
        return format!("{name}{suffix}");
    };

    if dot_index == 0 {
        format!("{name}{suffix}")
    } else {
        format!("{}{}{}", &name[..dot_index], suffix, &name[dot_index..])
    }
}

async fn plan_local_upload(
    local_paths: &[String],
    remote_dir: &str,
) -> Result<UploadPlan, AppError> {
    let remote_dir = normalize_remote_path(remote_dir);
    let mut root_targets = Vec::new();
    let mut seen_root_targets = HashSet::new();
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut bytes_total = 0;

    for local_path in local_paths {
        let root = PathBuf::from(local_path);
        let metadata = async_fs::metadata(&root).await.map_err(io_error)?;
        let name = local_file_name(&root)?;
        let remote_path = join_remote_path(&remote_dir, &name);

        if !seen_root_targets.insert(remote_path.clone()) {
            return Err(AppError::new(
                codes::TRANSFER_DUPLICATE_UPLOAD_TARGET,
                format!("Duplicate upload target: {remote_path}"),
            )
            .with("path", &remote_path));
        }
        root_targets.push(remote_path.clone());

        if metadata.is_dir() {
            directories.push(remote_path.clone());
            let mut stack = vec![(root, remote_path)];

            while let Some((local_dir, remote_parent)) = stack.pop() {
                let mut read_dir = async_fs::read_dir(&local_dir).await.map_err(io_error)?;

                while let Some(child) = read_dir.next_entry().await.map_err(io_error)? {
                    let child_path = child.path();
                    let child_file_type = child.file_type().await.map_err(io_error)?;
                    let child_name = local_file_name(&child_path)?;
                    let child_remote_path = join_remote_path(&remote_parent, &child_name);

                    if child_file_type.is_dir() {
                        directories.push(child_remote_path.clone());
                        stack.push((child_path, child_remote_path));
                    } else if child_file_type.is_file() {
                        let child_metadata = child.metadata().await.map_err(io_error)?;
                        let size = child_metadata.len();
                        bytes_total += size;
                        files.push(UploadFilePlan {
                            label: child_name,
                            local_path: child_path,
                            remote_path: child_remote_path,
                            size,
                            offset: 0,
                        });
                    }
                }
            }
        } else if metadata.is_file() {
            let size = metadata.len();
            bytes_total += size;
            files.push(UploadFilePlan {
                label: name,
                local_path: root,
                remote_path,
                size,
                offset: 0,
            });
        }
    }

    Ok(UploadPlan {
        root_targets,
        directories,
        files,
        bytes_total,
        resumed_bytes: 0,
        resumed_files_done: 0,
    })
}

async fn plan_remote_download(
    sftp: &SftpSession,
    remote_entries: &[RemoteTransferEntry],
    local_dir: &str,
) -> Result<DownloadPlan, AppError> {
    let local_dir = PathBuf::from(local_dir);
    let mut root_targets = Vec::new();
    let mut seen_root_targets = HashSet::new();
    let mut seen_local_paths = HashSet::new();
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut bytes_total = 0;

    for entry in remote_entries {
        let remote_path = normalize_remote_path(&entry.path);
        let raw_local_name = if entry.name.trim().is_empty() {
            remote_basename(&remote_path)
        } else {
            entry.name.clone()
        };
        let local_name = safe_local_name(&raw_local_name);
        let local_path = local_dir.join(local_name);

        if !seen_root_targets.insert(local_path.clone()) {
            return Err(duplicate_download_target(&local_path));
        }
        ensure_unique_local_download_path(&mut seen_local_paths, &local_path)?;
        root_targets.push(local_path.clone());

        let metadata = sftp
            .symlink_metadata(remote_path.clone())
            .await
            .map_err(|error| sftp_error(error, &remote_path))?;
        if metadata.file_type().is_dir() || entry.is_directory {
            directories.push(local_path.clone());
            let mut stack = vec![(remote_path, local_path)];

            while let Some((remote_dir, local_parent)) = stack.pop() {
                let read_dir = sftp
                    .read_dir(remote_dir.clone())
                    .await
                    .map_err(|error| sftp_error(error, &remote_dir))?;

                for child in read_dir {
                    let child_metadata = child.metadata();
                    let child_remote_path = normalize_remote_path(&child.path());
                    let child_local_path = local_parent.join(safe_local_name(&child.file_name()));
                    ensure_unique_local_download_path(&mut seen_local_paths, &child_local_path)?;

                    if child_metadata.file_type().is_dir() {
                        directories.push(child_local_path.clone());
                        stack.push((child_remote_path, child_local_path));
                    } else {
                        let size = child_metadata.size.unwrap_or(0);
                        bytes_total += size;
                        files.push(DownloadFilePlan {
                            label: child.file_name(),
                            remote_path: child_remote_path,
                            local_path: child_local_path,
                            size,
                            offset: 0,
                        });
                    }
                }
            }
        } else {
            // 历史记录中的大小可能已过期，续传必须优先按远端当前大小判断是否传完。
            let size = metadata.size.or(entry.size).unwrap_or(0);
            bytes_total += size;
            files.push(DownloadFilePlan {
                label: entry.name.clone(),
                remote_path,
                local_path,
                size,
                offset: 0,
            });
        }
    }

    Ok(DownloadPlan {
        root_targets,
        directories,
        files,
        bytes_total,
        resumed_bytes: 0,
        resumed_files_done: 0,
    })
}

fn duplicate_download_target(path: &Path) -> AppError {
    AppError::new(
        codes::LOCAL_DUPLICATE_TARGET,
        format!("Duplicate download target: {}", path_to_string(path)),
    )
    .with("path", path_to_string(path))
}

fn ensure_unique_local_download_path(
    seen_paths: &mut HashSet<PathBuf>,
    path: &Path,
) -> Result<(), AppError> {
    if seen_paths.insert(path.to_path_buf()) {
        return Ok(());
    }

    Err(duplicate_download_target(path))
}

async fn ensure_remote_directory(sftp: &SftpSession, path: &str) -> Result<(), AppError> {
    if path == "/" {
        return Ok(());
    }

    // 先尝试直接创建（新目录只需 1 次往返），失败再区分“已存在”与真实错误
    let Err(create_error) = sftp.create_dir(path.to_string()).await else {
        return Ok(());
    };

    match sftp.metadata(path.to_string()).await {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(AppError::new(
            codes::REMOTE_NOT_DIRECTORY,
            format!("Remote path is not a directory: {path}"),
        )
        .with("path", path)),
        Err(_) => Err(sftp_error(create_error, path)),
    }
}

struct TransferProgress {
    task_id: String,
    session_id: String,
    direction: String,
    label: String,
    bytes_transferred: u64,
    bytes_total: u64,
    files_done: u64,
    files_total: u64,
    last_emit_at: Instant,
}

impl TransferProgress {
    fn new(
        task_id: &str,
        session_id: &str,
        direction: &str,
        bytes_total: u64,
        files_total: u64,
        label: String,
    ) -> Self {
        Self {
            task_id: task_id.to_string(),
            session_id: session_id.to_string(),
            direction: direction.to_string(),
            label,
            bytes_transferred: 0,
            bytes_total,
            files_done: 0,
            files_total,
            last_emit_at: Instant::now(),
        }
    }

    fn emit(&mut self, app: &AppHandle, kind: &str, current_path: Option<String>, force: bool) {
        if !force && self.last_emit_at.elapsed() < Duration::from_millis(120) {
            return;
        }

        self.last_emit_at = Instant::now();
        emit_transfer_event(
            app,
            TransferEventPayload {
                task_id: self.task_id.clone(),
                session_id: self.session_id.clone(),
                direction: self.direction.clone(),
                kind: kind.to_string(),
                label: Some(self.label.clone()),
                current_path,
                bytes_transferred: self.bytes_transferred,
                bytes_total: self.bytes_total,
                files_done: self.files_done,
                files_total: self.files_total,
                error: None,
            },
        );
    }

    fn fail(&mut self, app: &AppHandle, error: AppError) {
        emit_transfer_event(
            app,
            TransferEventPayload {
                task_id: self.task_id.clone(),
                session_id: self.session_id.clone(),
                direction: self.direction.clone(),
                kind: "failed".to_string(),
                label: Some(self.label.clone()),
                current_path: None,
                bytes_transferred: self.bytes_transferred,
                bytes_total: self.bytes_total,
                files_done: self.files_done,
                files_total: self.files_total,
                error: Some(error),
            },
        );
    }

    fn cancel(&mut self, app: &AppHandle) {
        emit_transfer_event(
            app,
            TransferEventPayload {
                task_id: self.task_id.clone(),
                session_id: self.session_id.clone(),
                direction: self.direction.clone(),
                kind: "canceled".to_string(),
                label: Some(self.label.clone()),
                current_path: None,
                bytes_transferred: self.bytes_transferred,
                bytes_total: self.bytes_total,
                files_done: self.files_done,
                files_total: self.files_total,
                error: Some(transfer_canceled_error()),
            },
        );
    }
}

fn validate_upload_request(request: &RemoteUploadRequest) -> Result<(), AppError> {
    if request.task_id.trim().is_empty() {
        return Err(required("taskId"));
    }

    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.local_paths.is_empty() {
        return Err(AppError::new(
            codes::VALIDATION_SELECT_LOCAL_ITEM,
            "Choose at least one local file or folder",
        ));
    }

    if request.local_paths.iter().any(|path| path.trim().is_empty()) {
        return Err(required("localPath"));
    }

    parse_transfer_conflict_policy(&request.conflict_policy)?;

    Ok(())
}

fn validate_download_request(request: &RemoteDownloadRequest) -> Result<(), AppError> {
    if request.task_id.trim().is_empty() {
        return Err(required("taskId"));
    }

    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.remote_entries.is_empty() {
        return Err(AppError::new(
            codes::VALIDATION_SELECT_REMOTE_ITEM,
            "Choose at least one remote file or folder",
        ));
    }

    if request.local_dir.trim().is_empty() {
        return Err(AppError::new(
            codes::VALIDATION_SELECT_LOCAL_DIR,
            "Choose a local destination folder",
        ));
    }

    parse_transfer_conflict_policy(&request.conflict_policy)?;

    Ok(())
}

fn parse_transfer_conflict_policy(
    policy: &Option<String>,
) -> Result<TransferConflictPolicy, AppError> {
    match policy
        .as_deref()
        .unwrap_or("fail")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | "fail" | "ask" => Ok(TransferConflictPolicy::Fail),
        "overwrite" => Ok(TransferConflictPolicy::Overwrite),
        "skip" => Ok(TransferConflictPolicy::Skip),
        "rename" => Ok(TransferConflictPolicy::Rename),
        "resume" => Ok(TransferConflictPolicy::Resume),
        value => Err(AppError::new(
            codes::VALIDATION_CONFLICT_POLICY,
            format!("Unsupported conflict policy: {value}"),
        )
        .with("policy", value)),
    }
}

fn transfer_label_from_local_paths(local_paths: &[String]) -> String {
    if local_paths.len() == 1 {
        return local_file_name(&PathBuf::from(&local_paths[0]))
            .unwrap_or_else(|_| "Upload".to_string());
    }

    format!("{} local items", local_paths.len())
}

fn transfer_label_from_remote_entries(entries: &[RemoteTransferEntry]) -> String {
    if entries.len() == 1 {
        return entries[0].name.clone();
    }

    format!("{} remote items", entries.len())
}

fn is_transfer_canceled(cancellation: &Arc<AtomicBool>) -> bool {
    cancellation.load(Ordering::SeqCst)
}

fn ensure_transfer_active(cancellation: &Arc<AtomicBool>) -> Result<(), AppError> {
    if is_transfer_canceled(cancellation) {
        Err(transfer_canceled_error())
    } else {
        Ok(())
    }
}

fn transfer_error_kind(error: &AppError) -> &'static str {
    if error.is(codes::TRANSFER_CANCELED) {
        "canceled"
    } else {
        "failed"
    }
}

pub(crate) fn remove_transfer_cancellation(
    cancellations: &Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    task_id: &str,
) {
    if let Ok(mut cancellations) = cancellations.lock() {
        cancellations.remove(task_id);
    }
}

fn transfer_event(
    task_id: &str,
    session_id: &str,
    direction: &str,
    kind: &str,
) -> TransferEventPayload {
    TransferEventPayload {
        task_id: task_id.to_string(),
        session_id: session_id.to_string(),
        direction: direction.to_string(),
        kind: kind.to_string(),
        label: None,
        current_path: None,
        bytes_transferred: 0,
        bytes_total: 0,
        files_done: 0,
        files_total: 0,
        error: None,
    }
}

impl TransferEventPayload {
    fn with_error(mut self, error: AppError) -> Self {
        self.error = Some(error);
        self
    }
}

fn emit_transfer_event(app: &AppHandle, event: TransferEventPayload) {
    let _ = app.emit(TRANSFER_EVENT, event);
}
