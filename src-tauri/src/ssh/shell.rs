use std::sync::{Arc, Mutex};

use russh::{client, ChannelMsg, Disconnect};
use russh_sftp::client::SftpSession;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio::time::{Duration, Instant};

use crate::models::{
    AuthMethod, TerminalEventPayload, TerminalInputRequest, TerminalResizeRequest,
    TerminalStartRequest,
};
use crate::sftp::ensure_sftp_session;
use crate::sftp::ops::{
    create_directory_on_session, create_file_on_session, delete_paths_on_session,
    list_directory_on_session, read_file_text_on_session, rename_path_on_session,
    stream_directory_on_session, write_file_text_on_session,
};
use crate::sftp::transfer::{
    remove_transfer_cancellation, run_download_transfer, run_upload_transfer,
};
use crate::ssh::client::{authenticate_session, ConnectionTestClient, ServerKeySnapshot};
use crate::ssh::known_hosts;
use crate::ssh::registry::{TerminalControl, TerminalRegistry};
use crate::system_info::collect_system_info_on_session;
use crate::util::{connection_error, shell_quote};

const TERMINAL_EVENT: &str = "terminal-event";
const SSH_KEEPALIVE_INTERVAL_SECS: u64 = 30;
const TERMINAL_CLOSE_REASON_CLIENT: &str = "client";
const TERMINAL_CLOSE_REASON_REMOTE: &str = "remote";
const TERMINAL_CLOSE_REASON_ERROR: &str = "error";
// 输出合批：攒到阈值立即发，否则最多等一个 flush 间隔，避免大流量输出打爆 IPC
const TERMINAL_FLUSH_INTERVAL_MS: u64 = 8;
const TERMINAL_FLUSH_THRESHOLD_BYTES: usize = 128 * 1024;

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
    let registry_handle = registry.inner().clone();

    let attempt_id = request.attempt_id.clone();

    emit_terminal_event(
        &app,
        &request.session_id,
        attempt_id.as_deref(),
        "connecting",
        None,
        None,
        None,
    );

    tauri::async_runtime::spawn(async move {
        if let Err(error) =
            run_terminal_shell(app.clone(), request, control_receiver, registry_handle).await
        {
            emit_terminal_event(
                &app,
                &session_id,
                attempt_id.as_deref(),
                "error",
                None,
                Some(error),
                Some(TERMINAL_CLOSE_REASON_ERROR),
            );
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

async fn run_terminal_shell(
    app: AppHandle,
    request: TerminalStartRequest,
    mut control_receiver: mpsc::UnboundedReceiver<TerminalControl>,
    registry: TerminalRegistry,
) -> Result<(), String> {
    let session_id = request.session_id.clone();
    let attempt_id = request.attempt_id.clone();
    let mut close_reason = TERMINAL_CLOSE_REASON_REMOTE;
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    // TOFU：已有记录则校验指纹，首次连接成功后记录
    let expected_fingerprint = known_hosts::lookup_fingerprint(&app, &host, request.port)?;
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
        expected_fingerprint: expected_fingerprint.clone(),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(SSH_KEEPALIVE_INTERVAL_SECS)),
        ..Default::default()
    });

    let mut session = match client::connect(config, (host.as_str(), request.port), handler).await {
        Ok(session) => session,
        Err(error) => {
            let snapshot = server_key
                .lock()
                .map(|snapshot| snapshot.clone())
                .unwrap_or_default();

            if let (Some(expected), Some(actual)) = (&expected_fingerprint, &snapshot.fingerprint) {
                if expected != actual {
                    return Err(known_hosts::host_key_changed_error(expected, actual));
                }
            }

            return Err(connection_error(error));
        }
    };

    if expected_fingerprint.is_none() {
        let snapshot = server_key
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default();
        if let Some(fingerprint) = &snapshot.fingerprint {
            let _ = known_hosts::record_host_key(
                &app,
                &host,
                request.port,
                fingerprint,
                snapshot.algorithm.as_deref(),
            );
        }
    }

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

    emit_terminal_event(
        &app,
        &session_id,
        attempt_id.as_deref(),
        "connected",
        None,
        None,
        None,
    );

    let initial_path = request.default_remote_path.trim();
    if !initial_path.is_empty() && initial_path != "/" {
        channel
            .data_bytes(format!("cd {}\r", shell_quote(initial_path)).into_bytes())
            .await
            .map_err(connection_error)?;
    }

    // 复用同一个 SFTP 子通道处理目录小操作，出错时丢弃下次重建；
    // 操作在独立任务里执行（共享缓存加锁），慢操作（如递归删除）不会阻塞终端事件循环
    let cached_sftp: Arc<AsyncMutex<Option<SftpSession>>> = Arc::new(AsyncMutex::new(None));
    let mut output_buffer: Vec<u8> = Vec::new();
    let mut flush_deadline: Option<Instant> = None;

    loop {
        // Option<Instant> 是 Copy，async move 拿到的是快照；每轮循环按最新 deadline 重建
        let flush_timer = async move {
            match flush_deadline {
                Some(deadline) => tokio::time::sleep_until(deadline).await,
                None => std::future::pending::<()>().await,
            }
        };

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
                    TerminalControl::StreamDirectory {
                        request_id,
                        path,
                        cancellation,
                    } => {
                        let app = app.clone();
                        let session = Arc::clone(&session);
                        let session_id = session_id.clone();
                        let registry = registry.clone();
                        tauri::async_runtime::spawn(async move {
                            stream_directory_on_session(
                                &app,
                                &session_id,
                                &request_id,
                                session,
                                &path,
                                &cancellation,
                            )
                            .await;
                            registry.finish_directory_stream(&session_id, &request_id);
                        });
                    }
                    TerminalControl::GetSystemInfo { responder } => {
                        let session = Arc::clone(&session);
                        let session_id = session_id.clone();
                        tauri::async_runtime::spawn(async move {
                            let result = collect_system_info_on_session(
                                &session_id,
                                session,
                            )
                            .await;
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::CreateDirectory { path, responder } => {
                        let session = Arc::clone(&session);
                        let cached_sftp = Arc::clone(&cached_sftp);
                        tauri::async_runtime::spawn(async move {
                            let mut cached = cached_sftp.lock().await;
                            let result = match ensure_sftp_session(&mut cached, &session).await {
                                Ok(sftp) => create_directory_on_session(sftp, &path).await,
                                Err(error) => Err(error),
                            };
                            if result.is_err() {
                                *cached = None;
                            }
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::CreateFile { path, responder } => {
                        let session = Arc::clone(&session);
                        let cached_sftp = Arc::clone(&cached_sftp);
                        tauri::async_runtime::spawn(async move {
                            let mut cached = cached_sftp.lock().await;
                            let result = match ensure_sftp_session(&mut cached, &session).await {
                                Ok(sftp) => create_file_on_session(sftp, &path).await,
                                Err(error) => Err(error),
                            };
                            if result.is_err() {
                                *cached = None;
                            }
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::ListDirectory { path, responder } => {
                        let session = Arc::clone(&session);
                        let cached_sftp = Arc::clone(&cached_sftp);
                        tauri::async_runtime::spawn(async move {
                            let mut cached = cached_sftp.lock().await;
                            let result = match ensure_sftp_session(&mut cached, &session).await {
                                Ok(sftp) => list_directory_on_session(sftp, &path).await,
                                Err(error) => Err(error),
                            };
                            if result.is_err() {
                                *cached = None;
                            }
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::ReadFileText { path, responder } => {
                        let session = Arc::clone(&session);
                        let cached_sftp = Arc::clone(&cached_sftp);
                        tauri::async_runtime::spawn(async move {
                            let mut cached = cached_sftp.lock().await;
                            let result = match ensure_sftp_session(&mut cached, &session).await {
                                Ok(sftp) => read_file_text_on_session(sftp, &path).await,
                                Err(error) => Err(error),
                            };
                            if result.is_err() {
                                *cached = None;
                            }
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::WriteFileText {
                        path,
                        content,
                        responder,
                    } => {
                        let session = Arc::clone(&session);
                        let cached_sftp = Arc::clone(&cached_sftp);
                        tauri::async_runtime::spawn(async move {
                            let mut cached = cached_sftp.lock().await;
                            let result = match ensure_sftp_session(&mut cached, &session).await {
                                Ok(sftp) => write_file_text_on_session(sftp, &path, &content).await,
                                Err(error) => Err(error),
                            };
                            if result.is_err() {
                                *cached = None;
                            }
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::RenamePath {
                        from_path,
                        to_path,
                        responder,
                    } => {
                        let session = Arc::clone(&session);
                        let cached_sftp = Arc::clone(&cached_sftp);
                        tauri::async_runtime::spawn(async move {
                            let mut cached = cached_sftp.lock().await;
                            let result = match ensure_sftp_session(&mut cached, &session).await {
                                Ok(sftp) => rename_path_on_session(sftp, &from_path, &to_path).await,
                                Err(error) => Err(error),
                            };
                            if result.is_err() {
                                *cached = None;
                            }
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::DeletePaths {
                        entries,
                        recursive,
                        responder,
                    } => {
                        let session = Arc::clone(&session);
                        let cached_sftp = Arc::clone(&cached_sftp);
                        tauri::async_runtime::spawn(async move {
                            let mut cached = cached_sftp.lock().await;
                            let result = match ensure_sftp_session(&mut cached, &session).await {
                                Ok(sftp) => delete_paths_on_session(sftp, &entries, recursive).await,
                                Err(error) => Err(error),
                            };
                            if result.is_err() {
                                *cached = None;
                            }
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::StartUpload {
                        request,
                        cancellation,
                    } => {
                        let app = app.clone();
                        let session = Arc::clone(&session);
                        let task_id = request.task_id.clone();
                        let transfer_cancellations = Arc::clone(&registry.transfer_cancellations);
                        tauri::async_runtime::spawn(async move {
                            run_upload_transfer(app, session, request, cancellation).await;
                            remove_transfer_cancellation(&transfer_cancellations, &task_id);
                        });
                    }
                    TerminalControl::StartDownload {
                        request,
                        cancellation,
                    } => {
                        let app = app.clone();
                        let session = Arc::clone(&session);
                        let task_id = request.task_id.clone();
                        let transfer_cancellations = Arc::clone(&registry.transfer_cancellations);
                        tauri::async_runtime::spawn(async move {
                            run_download_transfer(app, session, request, cancellation).await;
                            remove_transfer_cancellation(&transfer_cancellations, &task_id);
                        });
                    }
                    TerminalControl::Close => {
                        close_reason = TERMINAL_CLOSE_REASON_CLIENT;
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
                        output_buffer.extend_from_slice(&data);

                        if output_buffer.len() >= TERMINAL_FLUSH_THRESHOLD_BYTES {
                            flush_terminal_output(
                                &app,
                                &session_id,
                                attempt_id.as_deref(),
                                &mut output_buffer,
                            );
                            flush_deadline = None;
                        } else if flush_deadline.is_none() {
                            flush_deadline = Some(
                                Instant::now() + Duration::from_millis(TERMINAL_FLUSH_INTERVAL_MS),
                            );
                        }
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => {
                        close_reason = TERMINAL_CLOSE_REASON_REMOTE;
                        break;
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        // 追加进输出缓冲，保持与真实输出的先后顺序
                        output_buffer.extend_from_slice(
                            format!("\r\n[exit status: {exit_status}]\r\n").as_bytes(),
                        );
                        if flush_deadline.is_none() {
                            flush_deadline = Some(
                                Instant::now() + Duration::from_millis(TERMINAL_FLUSH_INTERVAL_MS),
                            );
                        }
                    }
                    _ => {}
                }
            }
            _ = flush_timer => {
                flush_terminal_output(&app, &session_id, attempt_id.as_deref(), &mut output_buffer);
                flush_deadline = None;
            }
        }
    }

    if !output_buffer.is_empty() {
        emit_terminal_event(
            &app,
            &session_id,
            attempt_id.as_deref(),
            "output",
            Some(String::from_utf8_lossy(&output_buffer).to_string()),
            None,
            None,
        );
    }

    if let Some(sftp) = cached_sftp.try_lock().ok().and_then(|mut cached| cached.take()) {
        let _ = sftp.close().await;
    }

    let _ = session
        .lock()
        .await
        .disconnect(Disconnect::ByApplication, "terminal closed", "en")
        .await;
    emit_terminal_event(
        &app,
        &session_id,
        attempt_id.as_deref(),
        "closed",
        None,
        None,
        Some(close_reason),
    );
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

// 发出缓冲中的完整 UTF-8 前缀，结尾的不完整多字节序列留待下一次 flush
fn flush_terminal_output(
    app: &AppHandle,
    session_id: &str,
    attempt_id: Option<&str>,
    buffer: &mut Vec<u8>,
) {
    let keep = incomplete_utf8_suffix_len(buffer);
    let emit_len = buffer.len() - keep;
    if emit_len == 0 {
        return;
    }

    let text = String::from_utf8_lossy(&buffer[..emit_len]).to_string();
    buffer.drain(..emit_len);
    emit_terminal_event(app, session_id, attempt_id, "output", Some(text), None, None);
}

fn emit_terminal_event(
    app: &AppHandle,
    session_id: &str,
    attempt_id: Option<&str>,
    kind: &str,
    data: Option<String>,
    error: Option<String>,
    reason: Option<&str>,
) {
    let _ = app.emit(
        TERMINAL_EVENT,
        TerminalEventPayload {
            session_id: session_id.to_string(),
            attempt_id: attempt_id.map(str::to_string),
            kind: kind.to_string(),
            data,
            error,
            reason: reason.map(str::to_string),
        },
    );
}

// 终端输出按字节流到达，多字节 UTF-8 字符可能被拆在两个 chunk 之间：
// 返回结尾处不完整序列的字节数，这部分留到下一个 chunk 一起发出。
fn incomplete_utf8_suffix_len(bytes: &[u8]) -> usize {
    let len = bytes.len();
    let start = len.saturating_sub(3);

    for index in (start..len).rev() {
        let byte = bytes[index];
        if byte & 0b1100_0000 == 0b1000_0000 {
            continue;
        }

        let expected = if byte >= 0xF0 {
            4
        } else if byte >= 0xE0 {
            3
        } else if byte >= 0xC0 {
            2
        } else {
            return 0;
        };

        let available = len - index;
        return if available < expected { available } else { 0 };
    }

    0
}
