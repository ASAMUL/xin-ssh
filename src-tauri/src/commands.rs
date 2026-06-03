use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use ring::rand::{SecureRandom, SystemRandom};
use russh::{
    ChannelMsg,
    client,
    keys::{load_secret_key, ssh_key, PrivateKeyWithHashAlg, PublicKey},
    Disconnect,
};
use russh_sftp::{
    client::{fs::DirEntry, SftpSession},
    protocol::{FileType, OpenFlags},
};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    fs as async_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot, Mutex as AsyncMutex},
};
use tokio::time::{timeout, Duration, Instant};

use crate::models::{
    AppStateFile, AppStatus, AuthMethod, ConnectionGroup, ConnectionProfile, ConnectionTestRequest,
    ConnectionTestResult, ProfilesFile, RemoteDirectoryListRequest, RemoteDirectoryListing,
    RemoteDownloadRequest, RemoteFileEntry, RemoteTransferEntry, RemoteUploadRequest,
    TerminalEventPayload, TerminalInputRequest, TerminalResizeRequest, TerminalStartRequest,
    TransferEventPayload, WorkspaceSettings, WorkspaceSnapshot,
};

const APP_STATE_FILE: &str = "app-state.json";
const DEVICE_VAULT_KEY_FILE: &str = "device-vault-key.json";
const PROFILES_FILE: &str = "profiles.json";
const SETTINGS_FILE: &str = "settings.json";
const VAULT_FILE: &str = "vault.xinvault";
const CURRENT_SCHEMA_VERSION: u16 = 1;
const DEVICE_VAULT_KEY_BYTES: usize = 32;
const TERMINAL_EVENT: &str = "terminal-event";
const TRANSFER_EVENT: &str = "transfer-event";
const TRANSFER_BUFFER_SIZE: usize = 256 * 1024;

#[derive(Clone, Default)]
pub struct TerminalRegistry {
    sessions: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<TerminalControl>>>>,
}

enum TerminalControl {
    Input(Vec<u8>),
    Resize { columns: u32, rows: u32 },
    ListDirectory {
        path: String,
        responder: oneshot::Sender<Result<RemoteDirectoryListing, String>>,
    },
    StartUpload(RemoteUploadRequest),
    StartDownload(RemoteDownloadRequest),
    Close,
}

impl TerminalRegistry {
    fn replace(
        &self,
        session_id: String,
        sender: mpsc::UnboundedSender<TerminalControl>,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(error_message)?;
        if let Some(existing) = sessions.insert(session_id, sender) {
            let _ = existing.send(TerminalControl::Close);
        }

        Ok(())
    }

    fn send(&self, session_id: &str, control: TerminalControl) -> Result<(), String> {
        let sessions = self.sessions.lock().map_err(error_message)?;
        let sender = sessions
            .get(session_id)
            .ok_or_else(|| "Terminal session is not running".to_string())?;

        sender
            .send(control)
            .map_err(|_| "Terminal session is not accepting input".to_string())
    }

    fn close(&self, session_id: &str) -> Result<(), String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .remove(session_id);

        if let Some(sender) = sender {
            let _ = sender.send(TerminalControl::Close);
        }

        Ok(())
    }

    async fn list_directory(
        &self,
        session_id: &str,
        path: String,
    ) -> Result<RemoteDirectoryListing, String> {
        let sender = self
            .sessions
            .lock()
            .map_err(error_message)?
            .get(session_id)
            .cloned()
            .ok_or_else(|| "Terminal session is not running".to_string())?;
        let (responder, receiver) = oneshot::channel();

        sender
            .send(TerminalControl::ListDirectory { path, responder })
            .map_err(|_| "Terminal session is not accepting SFTP requests".to_string())?;

        receiver
            .await
            .map_err(|_| "SFTP request was canceled".to_string())?
    }

    fn start_upload(&self, request: RemoteUploadRequest) -> Result<(), String> {
        let sessions = self.sessions.lock().map_err(error_message)?;
        let sender = sessions
            .get(&request.session_id)
            .ok_or_else(|| "Terminal session is not running".to_string())?;

        sender
            .send(TerminalControl::StartUpload(request))
            .map_err(|_| "Terminal session is not accepting upload requests".to_string())
    }

    fn start_download(&self, request: RemoteDownloadRequest) -> Result<(), String> {
        let sessions = self.sessions.lock().map_err(error_message)?;
        let sender = sessions
            .get(&request.session_id)
            .ok_or_else(|| "Terminal session is not running".to_string())?;

        sender
            .send(TerminalControl::StartDownload(request))
            .map_err(|_| "Terminal session is not accepting download requests".to_string())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceVaultKeyFile {
    schema_version: u16,
    key: String,
}

#[tauri::command]
pub fn get_app_status(app: AppHandle) -> Result<AppStatus, String> {
    let state = read_app_state(&app)?;
    let workspace_dir = state.and_then(|state_file| state_file.workspace_dir);
    let initialized = workspace_dir
        .as_ref()
        .map(|dir| workspace_files_exist(Path::new(dir)))
        .unwrap_or(false);

    Ok(AppStatus {
        initialized,
        vault_path: workspace_dir.as_ref().map(|dir| vault_path(Path::new(dir))),
        workspace_dir,
    })
}

#[tauri::command]
pub fn init_workspace(
    app: AppHandle,
    workspace_dir: Option<String>,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = match workspace_dir {
        Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => default_workspace_dir(&app)?,
    };

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn load_workspace(app: AppHandle) -> Result<Option<WorkspaceSnapshot>, String> {
    let Some(state) = read_app_state(&app)? else {
        return Ok(None);
    };

    let Some(workspace_dir) = state.workspace_dir else {
        return Ok(None);
    };

    let workspace = PathBuf::from(workspace_dir);
    if !workspace_files_exist(&workspace) {
        return Ok(None);
    }

    load_workspace_from_dir(&workspace).map(Some)
}

#[tauri::command]
pub fn change_workspace_dir(
    app: AppHandle,
    workspace_dir: String,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = PathBuf::from(workspace_dir);

    if workspace_files_exist(&workspace) {
        write_app_state(&app, &workspace)?;
        return load_workspace_from_dir(&workspace);
    }

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn save_profile(
    app: AppHandle,
    profile: ConnectionProfile,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_profile = profile;
    next_profile.updated_at = now_stamp();

    if let Some(existing) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == next_profile.id)
    {
        *existing = next_profile;
    } else {
        profiles_file.profiles.push(next_profile);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, profile_id: String) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    if let Some(profile) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == profile_id)
    {
        profile.deleted_at = Some(deleted_at);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn save_group(app: AppHandle, group: ConnectionGroup) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_group = group;
    next_group.updated_at = now_stamp();

    if let Some(existing) = profiles_file
        .groups
        .iter_mut()
        .find(|item| item.id == next_group.id)
    {
        *existing = next_group;
    } else {
        profiles_file.groups.push(next_group);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn delete_group(app: AppHandle, group_id: String) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    if let Some(group) = profiles_file
        .groups
        .iter_mut()
        .find(|item| item.id == group_id)
    {
        group.deleted_at = Some(deleted_at.clone());
    }

    for profile in profiles_file
        .profiles
        .iter_mut()
        .filter(|profile| profile.group_id == group_id)
    {
        profile.group_id = default_group_id();
        profile.updated_at = deleted_at.clone();
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    settings: WorkspaceSettings,
) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    write_settings_file(&workspace, &settings)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub fn get_device_vault_password(app: AppHandle) -> Result<String, String> {
    let path = device_vault_key_path(&app)?;
    if path.exists() {
        let key_file: DeviceVaultKeyFile = read_json_file(&path)?;
        if !key_file.key.trim().is_empty() {
            return Ok(key_file.key);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_message)?;
    }

    let key = generate_device_vault_key()?;
    write_json_file(
        &path,
        &DeviceVaultKeyFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            key: key.clone(),
        },
    )?;

    Ok(key)
}

#[tauri::command]
pub fn archive_credential_store(app: AppHandle) -> Result<Option<String>, String> {
    let workspace = active_workspace_dir(&app)?;
    let vault = workspace.join(VAULT_FILE);
    if !vault.exists() {
        return Ok(None);
    }

    let backup = workspace.join(format!("vault-{}.backup.xinvault", now_stamp()));
    fs::rename(&vault, &backup).map_err(error_message)?;
    Ok(Some(path_to_string(&backup)))
}

#[tauri::command]
pub async fn test_connection(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, String> {
    validate_connection_test_request(&request)?;

    timeout(Duration::from_secs(12), run_connection_test(request))
        .await
        .map_err(|_| "Connection test timed out after 12 seconds".to_string())?
}

#[tauri::command]
pub async fn start_terminal_session(
    app: AppHandle,
    registry: State<'_, TerminalRegistry>,
    request: TerminalStartRequest,
) -> Result<(), String> {
    validate_terminal_start_request(&request)?;

    let session_id = request.session_id.clone();
    let (control_sender, control_receiver) = mpsc::unbounded_channel();
    registry.replace(session_id.clone(), control_sender)?;

    emit_terminal_event(
        &app,
        &request.session_id,
        "connecting",
        None,
        None,
    );

    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_terminal_shell(app.clone(), request, control_receiver).await {
            emit_terminal_event(&app, &session_id, "error", None, Some(error));
        }
    });

    Ok(())
}

#[tauri::command]
pub fn send_terminal_input(
    registry: State<'_, TerminalRegistry>,
    request: TerminalInputRequest,
) -> Result<(), String> {
    registry.send(
        &request.session_id,
        TerminalControl::Input(request.data.into_bytes()),
    )
}

#[tauri::command]
pub fn resize_terminal(
    registry: State<'_, TerminalRegistry>,
    request: TerminalResizeRequest,
) -> Result<(), String> {
    registry.send(
        &request.session_id,
        TerminalControl::Resize {
            columns: request.columns.max(1),
            rows: request.rows.max(1),
        },
    )
}

#[tauri::command]
pub fn close_terminal_session(
    registry: State<'_, TerminalRegistry>,
    session_id: String,
) -> Result<(), String> {
    registry.close(&session_id)
}

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
pub fn start_upload_transfer(
    registry: State<'_, TerminalRegistry>,
    request: RemoteUploadRequest,
) -> Result<(), String> {
    validate_upload_request(&request)?;
    registry.start_upload(request)
}

#[tauri::command]
pub fn start_download_transfer(
    registry: State<'_, TerminalRegistry>,
    request: RemoteDownloadRequest,
) -> Result<(), String> {
    validate_download_request(&request)?;
    registry.start_download(request)
}

async fn run_connection_test(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, String> {
    let auth_method = request.auth_method.clone();
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(12)),
        ..Default::default()
    });
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    let started = Instant::now();

    let mut session = client::connect(config, (host.as_str(), request.port), handler)
        .await
        .map_err(connection_error)?;

    let auth_result = match request.auth_method {
        AuthMethod::Password => {
            let password = request
                .password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Password is required".to_string())?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = request
                .private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Private key path is required".to_string())?;
            let passphrase = request
                .passphrase
                .as_deref()
                .filter(|value| !value.is_empty());
            let key_pair =
                load_secret_key(&private_key_path, passphrase).map_err(connection_error)?;
            let rsa_hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(connection_error)?
                .flatten();

            session
                .authenticate_publickey(
                    username,
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), rsa_hash),
                )
                .await
                .map_err(connection_error)?
        }
    };

    if !auth_result.success() {
        return Err("Authentication failed".to_string());
    }

    let _ = session
        .disconnect(Disconnect::ByApplication, "connection test complete", "en")
        .await;

    let server_key = server_key
        .lock()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_default();

    Ok(ConnectionTestResult {
        authenticated: true,
        auth_method,
        latency_ms: started.elapsed().as_millis(),
        server_fingerprint: server_key.fingerprint,
        server_key_algorithm: server_key.algorithm,
    })
}

async fn run_terminal_shell(
    app: AppHandle,
    request: TerminalStartRequest,
    mut control_receiver: mpsc::UnboundedReceiver<TerminalControl>,
) -> Result<(), String> {
    let session_id = request.session_id.clone();
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        ..Default::default()
    });
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();

    let mut session = client::connect(config, (host.as_str(), request.port), handler)
        .await
        .map_err(connection_error)?;

    authenticate_session(
        &mut session,
        username,
        request.auth_method.clone(),
        request.password,
        request.private_key_path,
        request.passphrase,
    )
    .await?;

    let session = Arc::new(AsyncMutex::new(session));
    let mut channel = {
        let session_handle = session.lock().await;
        session_handle
            .channel_open_session()
            .await
            .map_err(connection_error)?
    };
    channel
        .request_pty(
            false,
            "xterm-256color",
            request.columns.max(1),
            request.rows.max(1),
            0,
            0,
            &[],
        )
        .await
        .map_err(connection_error)?;
    channel
        .request_shell(true)
        .await
        .map_err(connection_error)?;

    emit_terminal_event(&app, &session_id, "connected", None, None);

    let initial_path = request.default_remote_path.trim();
    if !initial_path.is_empty() && initial_path != "/" {
        channel
            .data_bytes(format!("cd {}\r", shell_quote(initial_path)).into_bytes())
            .await
            .map_err(connection_error)?;
    }

    loop {
        tokio::select! {
            Some(control) = control_receiver.recv() => {
                match control {
                    TerminalControl::Input(data) => {
                        channel
                            .data_bytes(data)
                            .await
                            .map_err(connection_error)?;
                    }
                    TerminalControl::Resize { columns, rows } => {
                        channel
                            .window_change(columns.max(1), rows.max(1), 0, 0)
                            .await
                            .map_err(connection_error)?;
                    }
                    TerminalControl::ListDirectory { path, responder } => {
                        let result = list_directory_on_session(
                            &session_id,
                            Arc::clone(&session),
                            &path,
                        )
                        .await;
                        let _ = responder.send(result);
                    }
                    TerminalControl::StartUpload(request) => {
                        let app = app.clone();
                        let session = Arc::clone(&session);
                        tauri::async_runtime::spawn(async move {
                            run_upload_transfer(app, session, request).await;
                        });
                    }
                    TerminalControl::StartDownload(request) => {
                        let app = app.clone();
                        let session = Arc::clone(&session);
                        tauri::async_runtime::spawn(async move {
                            run_download_transfer(app, session, request).await;
                        });
                    }
                    TerminalControl::Close => {
                        let _ = channel.close().await;
                        break;
                    }
                }
            }
            message = channel.wait() => {
                let Some(message) = message else {
                    break;
                };

                match message {
                    ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                        emit_terminal_event(
                            &app,
                            &session_id,
                            "output",
                            Some(String::from_utf8_lossy(&data).to_string()),
                            None,
                        );
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => {
                        break;
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        emit_terminal_event(
                            &app,
                            &session_id,
                            "output",
                            Some(format!("\r\n[exit status: {exit_status}]\r\n")),
                            None,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = session
        .lock()
        .await
        .disconnect(Disconnect::ByApplication, "terminal closed", "en")
        .await;
    emit_terminal_event(&app, &session_id, "closed", None, None);
    Ok(())
}

async fn list_directory_on_session(
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

async fn open_sftp_session(
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

fn remote_file_entry(entry: DirEntry) -> RemoteFileEntry {
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

async fn run_upload_transfer(
    app: AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteUploadRequest,
) {
    if let Err(error) = upload_transfer(&app, session, request.clone()).await {
        emit_transfer_event(
            &app,
            transfer_event(&request.task_id, &request.session_id, "upload", "failed")
                .with_error(error),
        );
    }
}

async fn run_download_transfer(
    app: AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteDownloadRequest,
) {
    if let Err(error) = download_transfer(&app, session, request.clone()).await {
        emit_transfer_event(
            &app,
            transfer_event(&request.task_id, &request.session_id, "download", "failed")
                .with_error(error),
        );
    }
}

async fn upload_transfer(
    app: &AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteUploadRequest,
) -> Result<(), String> {
    let sftp = open_sftp_session(session).await?;
    let result = upload_transfer_with_sftp(app, &sftp, &request).await;
    let _ = sftp.close().await;
    result
}

async fn upload_transfer_with_sftp(
    app: &AppHandle,
    sftp: &SftpSession,
    request: &RemoteUploadRequest,
) -> Result<(), String> {
    let plan = plan_local_upload(&request.local_paths, &request.remote_dir).await?;
    ensure_remote_targets_available(sftp, &plan.root_targets).await?;

    let mut progress = TransferProgress::new(
        &request.task_id,
        &request.session_id,
        "upload",
        plan.bytes_total,
        plan.files.len() as u64,
        transfer_label_from_local_paths(&request.local_paths),
    );
    progress.emit(app, "started", None, true);

    for directory in &plan.directories {
        ensure_remote_directory(sftp, directory).await?;
    }

    let mut errors = Vec::new();
    for file in &plan.files {
        progress.label = file.label.clone();
        if let Err(error) = upload_file(sftp, file, app, &mut progress).await {
            let _ = sftp.remove_file(file.remote_path.clone()).await;
            errors.push(format!("{}: {error}", file.label));
        }
    }

    if !errors.is_empty() {
        progress.fail(app, summarize_transfer_errors(errors));
        return Ok(());
    }

    progress.emit(app, "completed", None, true);
    Ok(())
}

async fn download_transfer(
    app: &AppHandle,
    session: Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>,
    request: RemoteDownloadRequest,
) -> Result<(), String> {
    let sftp = open_sftp_session(session).await?;
    let result = download_transfer_with_sftp(app, &sftp, &request).await;
    let _ = sftp.close().await;
    result
}

async fn download_transfer_with_sftp(
    app: &AppHandle,
    sftp: &SftpSession,
    request: &RemoteDownloadRequest,
) -> Result<(), String> {
    let plan = plan_remote_download(sftp, &request.remote_entries, &request.local_dir).await?;
    ensure_local_targets_available(&plan.root_targets)?;

    let mut progress = TransferProgress::new(
        &request.task_id,
        &request.session_id,
        "download",
        plan.bytes_total,
        plan.files.len() as u64,
        transfer_label_from_remote_entries(&request.remote_entries),
    );
    progress.emit(app, "started", None, true);

    for directory in &plan.directories {
        async_fs::create_dir_all(directory)
            .await
            .map_err(error_message)?;
    }

    let mut errors = Vec::new();
    for file in &plan.files {
        progress.label = file.label.clone();
        if let Err(error) = download_file(sftp, file, app, &mut progress).await {
            let _ = async_fs::remove_file(&file.local_path).await;
            errors.push(format!("{}: {error}", file.label));
        }
    }

    if !errors.is_empty() {
        progress.fail(app, summarize_transfer_errors(errors));
        return Ok(());
    }

    progress.emit(app, "completed", None, true);
    Ok(())
}

async fn upload_file(
    sftp: &SftpSession,
    file: &UploadFilePlan,
    app: &AppHandle,
    progress: &mut TransferProgress,
) -> Result<(), String> {
    let mut local_file = async_fs::File::open(&file.local_path)
        .await
        .map_err(error_message)?;
    let mut remote_file = sftp
        .open_with_flags(
            file.remote_path.clone(),
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(connection_error)?;
    let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];

    loop {
        let read = local_file.read(&mut buffer).await.map_err(error_message)?;
        if read == 0 {
            break;
        }

        remote_file
            .write_all(&buffer[..read])
            .await
            .map_err(error_message)?;
        progress.bytes_transferred += read as u64;
        progress.emit(app, "progress", Some(file.remote_path.clone()), false);
    }

    remote_file.flush().await.map_err(error_message)?;
    remote_file.shutdown().await.map_err(error_message)?;
    progress.files_done += 1;
    progress.emit(app, "progress", Some(file.remote_path.clone()), true);
    Ok(())
}

async fn download_file(
    sftp: &SftpSession,
    file: &DownloadFilePlan,
    app: &AppHandle,
    progress: &mut TransferProgress,
) -> Result<(), String> {
    if let Some(parent) = file.local_path.parent() {
        async_fs::create_dir_all(parent).await.map_err(error_message)?;
    }

    let mut remote_file = sftp
        .open(file.remote_path.clone())
        .await
        .map_err(connection_error)?;
    let mut local_file = async_fs::File::create(&file.local_path)
        .await
        .map_err(error_message)?;
    let mut buffer = vec![0_u8; TRANSFER_BUFFER_SIZE];

    loop {
        let read = remote_file.read(&mut buffer).await.map_err(error_message)?;
        if read == 0 {
            break;
        }

        local_file
            .write_all(&buffer[..read])
            .await
            .map_err(error_message)?;
        progress.bytes_transferred += read as u64;
        progress.emit(app, "progress", Some(file.remote_path.clone()), false);
    }

    local_file.flush().await.map_err(error_message)?;
    remote_file.shutdown().await.map_err(error_message)?;
    progress.files_done += 1;
    progress.emit(app, "progress", Some(file.remote_path.clone()), true);
    Ok(())
}

struct UploadPlan {
    root_targets: Vec<String>,
    directories: Vec<String>,
    files: Vec<UploadFilePlan>,
    bytes_total: u64,
}

struct UploadFilePlan {
    local_path: PathBuf,
    remote_path: String,
    label: String,
}

struct DownloadPlan {
    root_targets: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    files: Vec<DownloadFilePlan>,
    bytes_total: u64,
}

struct DownloadFilePlan {
    remote_path: String,
    local_path: PathBuf,
    label: String,
}

async fn plan_local_upload(local_paths: &[String], remote_dir: &str) -> Result<UploadPlan, String> {
    let remote_dir = normalize_remote_path(remote_dir);
    let mut root_targets = Vec::new();
    let mut seen_root_targets = HashSet::new();
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut bytes_total = 0;

    for local_path in local_paths {
        let root = PathBuf::from(local_path);
        let metadata = async_fs::metadata(&root).await.map_err(error_message)?;
        let name = local_file_name(&root)?;
        let remote_path = join_remote_path(&remote_dir, &name);

        if !seen_root_targets.insert(remote_path.clone()) {
            return Err(format!("Duplicate upload target: {remote_path}"));
        }
        root_targets.push(remote_path.clone());

        if metadata.is_dir() {
            directories.push(remote_path.clone());
            let mut stack = vec![(root, remote_path)];

            while let Some((local_dir, remote_parent)) = stack.pop() {
                let mut read_dir = async_fs::read_dir(&local_dir).await.map_err(error_message)?;

                while let Some(child) = read_dir.next_entry().await.map_err(error_message)? {
                    let child_path = child.path();
                    let child_file_type = child.file_type().await.map_err(error_message)?;
                    let child_name = local_file_name(&child_path)?;
                    let child_remote_path = join_remote_path(&remote_parent, &child_name);

                    if child_file_type.is_dir() {
                        directories.push(child_remote_path.clone());
                        stack.push((child_path, child_remote_path));
                    } else if child_file_type.is_file() {
                        let child_metadata = child.metadata().await.map_err(error_message)?;
                        bytes_total += child_metadata.len();
                        files.push(UploadFilePlan {
                            label: child_name,
                            local_path: child_path,
                            remote_path: child_remote_path,
                        });
                    }
                }
            }
        } else if metadata.is_file() {
            bytes_total += metadata.len();
            files.push(UploadFilePlan {
                label: name,
                local_path: root,
                remote_path,
            });
        }
    }

    Ok(UploadPlan {
        root_targets,
        directories,
        files,
        bytes_total,
    })
}

async fn plan_remote_download(
    sftp: &SftpSession,
    remote_entries: &[RemoteTransferEntry],
    local_dir: &str,
) -> Result<DownloadPlan, String> {
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
            return Err(format!(
                "Duplicate download target: {}",
                path_to_string(&local_path)
            ));
        }
        ensure_unique_local_download_path(&mut seen_local_paths, &local_path)?;
        root_targets.push(local_path.clone());

        let metadata = sftp
            .symlink_metadata(remote_path.clone())
            .await
            .map_err(connection_error)?;
        if metadata.file_type().is_dir() || entry.is_directory {
            directories.push(local_path.clone());
            let mut stack = vec![(remote_path, local_path)];

            while let Some((remote_dir, local_parent)) = stack.pop() {
                let read_dir = sftp
                    .read_dir(remote_dir.clone())
                    .await
                    .map_err(connection_error)?;

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
                        });
                    }
                }
            }
        } else {
            let size = entry.size.or(metadata.size).unwrap_or(0);
            bytes_total += size;
            files.push(DownloadFilePlan {
                label: entry.name.clone(),
                remote_path,
                local_path,
            });
        }
    }

    Ok(DownloadPlan {
        root_targets,
        directories,
        files,
        bytes_total,
    })
}

async fn ensure_remote_targets_available(
    sftp: &SftpSession,
    root_targets: &[String],
) -> Result<(), String> {
    for target in root_targets {
        if sftp
            .try_exists(target.clone())
            .await
            .map_err(connection_error)?
        {
            return Err(format!("Remote target already exists: {target}"));
        }
    }

    Ok(())
}

fn ensure_local_targets_available(root_targets: &[PathBuf]) -> Result<(), String> {
    for target in root_targets {
        if target.exists() {
            return Err(format!(
                "Local target already exists: {}",
                path_to_string(target)
            ));
        }
    }

    Ok(())
}

fn ensure_unique_local_download_path(
    seen_paths: &mut HashSet<PathBuf>,
    path: &Path,
) -> Result<(), String> {
    if seen_paths.insert(path.to_path_buf()) {
        return Ok(());
    }

    Err(format!(
        "Duplicate download target after name sanitizing: {}",
        path_to_string(path)
    ))
}

async fn ensure_remote_directory(sftp: &SftpSession, path: &str) -> Result<(), String> {
    if path == "/" {
        return Ok(());
    }

    if sftp
        .try_exists(path.to_string())
        .await
        .map_err(connection_error)?
    {
        let metadata = sftp
            .metadata(path.to_string())
            .await
            .map_err(connection_error)?;
        if metadata.file_type().is_dir() {
            return Ok(());
        }

        return Err(format!("Remote path is not a directory: {path}"));
    }

    sftp.create_dir(path.to_string())
        .await
        .map_err(connection_error)
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

    fn fail(&mut self, app: &AppHandle, error: String) {
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
}

async fn authenticate_session(
    session: &mut client::Handle<ConnectionTestClient>,
    username: String,
    auth_method: AuthMethod,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
) -> Result<(), String> {
    let auth_result = match auth_method {
        AuthMethod::Password => {
            let password = password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Password is required".to_string())?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Private key path is required".to_string())?;
            let passphrase = passphrase.as_deref().filter(|value| !value.is_empty());
            let key_pair =
                load_secret_key(&private_key_path, passphrase).map_err(connection_error)?;
            let rsa_hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(connection_error)?
                .flatten();

            session
                .authenticate_publickey(
                    username,
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), rsa_hash),
                )
                .await
                .map_err(connection_error)?
        }
    };

    if auth_result.success() {
        Ok(())
    } else {
        Err("Authentication failed".to_string())
    }
}

fn validate_connection_test_request(request: &ConnectionTestRequest) -> Result<(), String> {
    if request.host.trim().is_empty() {
        return Err("Host is required".to_string());
    }

    if request.username.trim().is_empty() {
        return Err("Username is required".to_string());
    }

    if request.port == 0 {
        return Err("Port must be greater than 0".to_string());
    }

    Ok(())
}

fn validate_terminal_start_request(request: &TerminalStartRequest) -> Result<(), String> {
    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.host.trim().is_empty() {
        return Err("Host is required".to_string());
    }

    if request.username.trim().is_empty() {
        return Err("Username is required".to_string());
    }

    if request.port == 0 {
        return Err("Port must be greater than 0".to_string());
    }

    match &request.auth_method {
        AuthMethod::Password => {
            if request.password.as_deref().unwrap_or_default().is_empty() {
                return Err("Password is required".to_string());
            }
        }
        AuthMethod::PrivateKey => {
            if request
                .private_key_path
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err("Private key path is required".to_string());
            }
        }
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

fn validate_upload_request(request: &RemoteUploadRequest) -> Result<(), String> {
    if request.task_id.trim().is_empty() {
        return Err("Task id is required".to_string());
    }

    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.local_paths.is_empty() {
        return Err("Choose at least one local file or folder".to_string());
    }

    if request.local_paths.iter().any(|path| path.trim().is_empty()) {
        return Err("Local path is required".to_string());
    }

    Ok(())
}

fn validate_download_request(request: &RemoteDownloadRequest) -> Result<(), String> {
    if request.task_id.trim().is_empty() {
        return Err("Task id is required".to_string());
    }

    if request.session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if request.remote_entries.is_empty() {
        return Err("Choose at least one remote file or folder".to_string());
    }

    if request.local_dir.trim().is_empty() {
        return Err("Choose a local destination folder".to_string());
    }

    Ok(())
}

fn normalize_remote_path(path: &str) -> String {
    let trimmed = path.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return "/".to_string();
    }

    let mut parts = Vec::new();
    for part in trimmed.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }

    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

fn join_remote_path(base: &str, child: &str) -> String {
    let base = normalize_remote_path(base);
    let child: String = child
        .chars()
        .map(|character| if character == '/' || character == '\\' { '_' } else { character })
        .collect();

    if base == "/" {
        format!("/{child}")
    } else {
        format!("{base}/{child}")
    }
}

fn remote_basename(path: &str) -> String {
    normalize_remote_path(path)
        .split('/')
        .filter(|part| !part.is_empty())
        .next_back()
        .unwrap_or("remote-item")
        .to_string()
}

fn local_file_name(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("Could not determine file name: {}", path_to_string(path)))?
        .to_string_lossy()
        .trim()
        .to_string();

    if name.is_empty() {
        Err(format!("Could not determine file name: {}", path_to_string(path)))
    } else {
        Ok(name)
    }
}

fn safe_local_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            value if value.is_control() => '_',
            value => value,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "remote-item".to_string()
    } else {
        sanitized
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

fn summarize_transfer_errors(errors: Vec<String>) -> String {
    let total = errors.len();
    let preview = errors
        .into_iter()
        .take(3)
        .collect::<Vec<_>>()
        .join("; ");

    if total > 3 {
        format!("{total} files failed. First errors: {preview}")
    } else {
        preview
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
    fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }
}

fn emit_terminal_event(
    app: &AppHandle,
    session_id: &str,
    kind: &str,
    data: Option<String>,
    error: Option<String>,
) {
    let _ = app.emit(
        TERMINAL_EVENT,
        TerminalEventPayload {
            session_id: session_id.to_string(),
            kind: kind.to_string(),
            data,
            error,
        },
    );
}

fn emit_transfer_event(app: &AppHandle, event: TransferEventPayload) {
    let _ = app.emit(TRANSFER_EVENT, event);
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Clone, Default)]
struct ServerKeySnapshot {
    fingerprint: Option<String>,
    algorithm: Option<String>,
}

struct ConnectionTestClient {
    server_key: Arc<Mutex<ServerKeySnapshot>>,
}

impl client::Handler for ConnectionTestClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        if let Ok(mut snapshot) = self.server_key.lock() {
            snapshot.fingerprint = Some(
                server_public_key
                    .fingerprint(ssh_key::HashAlg::Sha256)
                    .to_string(),
            );
            snapshot.algorithm = Some(server_public_key.algorithm().to_string());
        }

        Ok(true)
    }
}

fn load_workspace_from_dir(workspace: &Path) -> Result<WorkspaceSnapshot, String> {
    ensure_workspace(workspace)?;
    let settings = read_settings_file(workspace)?;
    let profiles_file = read_profiles_file(workspace)?;

    Ok(WorkspaceSnapshot {
        workspace_dir: path_to_string(workspace),
        vault_path: vault_path(workspace),
        settings,
        groups: profiles_file.groups,
        profiles: profiles_file.profiles,
    })
}

fn ensure_workspace(workspace: &Path) -> Result<(), String> {
    fs::create_dir_all(workspace).map_err(error_message)?;

    let profiles_path = workspace.join(PROFILES_FILE);
    if !profiles_path.exists() {
        write_profiles_file(workspace, &default_profiles_file())?;
    }

    let settings_path = workspace.join(SETTINGS_FILE);
    if !settings_path.exists() {
        write_settings_file(workspace, &default_settings())?;
    }

    Ok(())
}

fn workspace_files_exist(workspace: &Path) -> bool {
    workspace.join(PROFILES_FILE).exists() && workspace.join(SETTINGS_FILE).exists()
}

fn read_app_state(app: &AppHandle) -> Result<Option<AppStateFile>, String> {
    let path = app_state_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    read_json_file(&path).map(Some)
}

fn write_app_state(app: &AppHandle, workspace: &Path) -> Result<(), String> {
    let path = app_state_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_message)?;
    }

    write_json_file(
        &path,
        &AppStateFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            workspace_dir: Some(path_to_string(workspace)),
        },
    )
}

fn active_workspace_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let state = read_app_state(app)?.ok_or_else(|| "Workspace is not initialized".to_string())?;
    let workspace_dir = state
        .workspace_dir
        .ok_or_else(|| "Workspace directory is not configured".to_string())?;

    Ok(PathBuf::from(workspace_dir))
}

fn app_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join(APP_STATE_FILE))
}

fn device_vault_key_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join(DEVICE_VAULT_KEY_FILE))
}

fn default_workspace_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join("workspace"))
}

fn read_profiles_file(workspace: &Path) -> Result<ProfilesFile, String> {
    read_json_file(&workspace.join(PROFILES_FILE))
}

fn write_profiles_file(workspace: &Path, profiles_file: &ProfilesFile) -> Result<(), String> {
    write_json_file(&workspace.join(PROFILES_FILE), profiles_file)
}

fn read_settings_file(workspace: &Path) -> Result<WorkspaceSettings, String> {
    read_json_file(&workspace.join(SETTINGS_FILE))
}

fn write_settings_file(workspace: &Path, settings: &WorkspaceSettings) -> Result<(), String> {
    write_json_file(&workspace.join(SETTINGS_FILE), settings)
}

fn read_json_file<T>(path: &Path) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(error_message)?;
    serde_json::from_str(&content).map_err(error_message)
}

fn write_json_file<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: serde::Serialize,
{
    let content = serde_json::to_string_pretty(value).map_err(error_message)?;
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(error_message)?;

    if path.exists() {
        fs::remove_file(path).map_err(error_message)?;
    }

    fs::rename(temp_path, path).map_err(error_message)
}

fn default_profiles_file() -> ProfilesFile {
    let now = now_stamp();
    ProfilesFile {
        schema_version: CURRENT_SCHEMA_VERSION,
        groups: vec![ConnectionGroup {
            id: default_group_id(),
            sync_id: default_group_id(),
            name: "Ungrouped".to_string(),
            sort_order: 0,
            collapsed: false,
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        }],
        profiles: Vec::new(),
    }
}

fn default_settings() -> WorkspaceSettings {
    WorkspaceSettings {
        schema_version: CURRENT_SCHEMA_VERSION,
        locale: "zh-CN".to_string(),
        terminal_font_family: "Cascadia Code".to_string(),
        remember_vault_unlock: false,
    }
}

fn default_group_id() -> String {
    "group-default".to_string()
}

fn vault_path(workspace: &Path) -> String {
    path_to_string(&workspace.join(VAULT_FILE))
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn now_stamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    millis.to_string()
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn connection_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn generate_device_vault_key() -> Result<String, String> {
    let rng = SystemRandom::new();
    let mut key = [0_u8; DEVICE_VAULT_KEY_BYTES];
    rng.fill(&mut key)
        .map_err(|_| "Failed to generate local credential key".to_string())?;

    Ok(hex_encode(&key))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}
