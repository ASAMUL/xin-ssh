use std::sync::Arc;

use russh::{ChannelMsg, Disconnect};
use tauri::{AppHandle, Emitter, State};
use tauri::ipc::Channel;
use tokio::sync::{mpsc, Mutex as AsyncMutex, Semaphore};
use tokio::time::{timeout, Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::error::{connection_error, required, timeout as timeout_error, AppError};
use crate::models::{
    AuthMethod, TerminalEventPayload, TerminalInputRequest, TerminalResizeRequest,
    TerminalStartRequest,
};
use crate::sftp::delete::run_delete_task;
use crate::sftp::session::CachedSftp;
use crate::sftp::ops::{
    create_directory_on_session, create_file_on_session,
    chmod_path_on_session, copy_path_on_session, list_directory_on_session, read_file_text_on_session, rename_path_on_session,
    stream_directory_on_session, write_file_text_on_session,
};
use crate::sftp::transfer::{
    remove_transfer_cancellation, run_download_transfer, run_upload_transfer,
};
use crate::ssh::client::{authenticate_session, connect_with_tofu, disconnect_session};
use crate::ssh::interactive::{AuthenticationContext, AuthenticationEvent};
use crate::ssh::registry::{TerminalControl, TerminalRegistry};
use crate::system_info::{collect_system_info_on_session, SystemInfoSampler};
use crate::util::shell_quote;

const TERMINAL_EVENT: &str = "terminal-event";
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
    authentication: Channel<AuthenticationEvent>,
) -> Result<(), AppError> {
    let authentication = AuthenticationContext::new(&app, authentication);
    validate_terminal_start_request(&request)?;

    let session_id = request.session_id.clone();
    let (control_sender, control_receiver) = mpsc::unbounded_channel();
    let cancellation = CancellationToken::new();
    registry.replace(session_id.clone(), control_sender, cancellation.clone())?;
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
            run_terminal_shell(app.clone(), request, control_receiver, registry_handle, cancellation, authentication).await
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
) -> Result<(), AppError> {
    registry.send(
        &request.session_id,
        TerminalControl::Input(request.data.into_bytes()),
    )
}

#[tauri::command]
pub fn resize_terminal(
    registry: State<'_, TerminalRegistry>,
    request: TerminalResizeRequest,
) -> Result<(), AppError> {
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
) -> Result<(), AppError> {
    registry.close(&session_id)
}

async fn run_terminal_shell(
    app: AppHandle,
    request: TerminalStartRequest,
    mut control_receiver: mpsc::UnboundedReceiver<TerminalControl>,
    registry: TerminalRegistry,
    cancellation: CancellationToken,
    authentication: AuthenticationContext,
) -> Result<(), AppError> {
    // 包括握手失败和事件循环提前返回在内，任何退出都必须停止所属文件操作。
    let _cancel_on_exit = cancellation.clone().drop_guard();
    let session_id = request.session_id.clone();
    let attempt_id = request.attempt_id.clone();
    let mut close_reason = TERMINAL_CLOSE_REASON_REMOTE;
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    // 关闭或替换标签必须覆盖建连阶段，不能等握手结束才响应取消并短暂复活旧会话。
    let setup = async {
        let mut session = connect_with_tofu(&app, &host, request.port, request.keepalive_seconds).await?;
        authenticate_session(
            &mut session,
            username,
            request.auth_method.clone(),
            request.password,
            request.private_key_path,
            request.passphrase,
            request.agent,
            authentication,
        )
        .await?;

        let channel = timeout(Duration::from_secs(12), async {
            let channel = session.channel_open_session().await.map_err(connection_error)?;
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
            // sshd 可能未放行 COLORTERM；真彩提示被忽略时，不能阻断终端启动。
            let _ = channel.set_env(false, "COLORTERM", "truecolor").await;
            channel.request_shell(true).await.map_err(connection_error)?;
            Ok::<_, AppError>(channel)
        })
        .await
        .map_err(|_| timeout_error("openTerminal", 12))
        .and_then(|result| result);
        if channel.is_err() {
            disconnect_session(&session, "terminal setup failed").await;
        }
        Ok::<_, AppError>((Arc::new(AsyncMutex::new(session)), channel?))
    };
    let (session, mut channel) = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Ok(()),
        result = setup => result?,
    };

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

    let cached_sftp = CachedSftp::new(Arc::clone(&session), cancellation.clone());
    // 删除独占子通道，并单独限流，不能让大目录删除占满普通操作的并发名额。
    let delete_permits = Arc::new(Semaphore::new(2));
    let system_info_sampler = Arc::new(AsyncMutex::new(SystemInfoSampler::default()));
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
            _ = cancellation.cancelled() => {
                close_reason = TERMINAL_CLOSE_REASON_CLIENT;
                let _ = channel.close().await;
                break;
            }
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
                    TerminalControl::GetSystemInfo { mut responder } => {
                        let session = Arc::clone(&session);
                        let session_id = session_id.clone();
                        let sampler = Arc::clone(&system_info_sampler);
                        let cancellation = cancellation.clone();
                        tauri::async_runtime::spawn(async move {
                            // 同一连接串行采样，接收方超时或关闭连接后不再排队执行远端命令。
                            let result = tokio::select! {
                                biased;
                                _ = cancellation.cancelled() => return,
                                _ = responder.closed() => return,
                                result = async {
                                    let mut sampler = sampler.lock().await;
                                    collect_system_info_on_session(&session_id, session, &mut sampler).await
                                } => result,
                            };
                            let _ = responder.send(result);
                        });
                    }
                    TerminalControl::CreateDirectory { path, responder } => {
                        cached_sftp.spawn(responder, true, |sftp| async move {
                            create_directory_on_session(&sftp, &path).await
                        });
                    }
                    TerminalControl::CreateFile { path, responder } => {
                        cached_sftp.spawn(responder, true, |sftp| async move {
                            create_file_on_session(&sftp, &path).await
                        });
                    }
                    TerminalControl::ListDirectory { path, responder } => {
                        cached_sftp.spawn(responder, false, |sftp| async move {
                            list_directory_on_session(&sftp, &path).await
                        });
                    }
                    TerminalControl::ReadFileText { path, responder } => {
                        cached_sftp.spawn(responder, false, |sftp| async move {
                            read_file_text_on_session(&sftp, &path).await
                        });
                    }
                    TerminalControl::WriteFileText {
                        path,
                        content,
                        responder,
                    } => {
                        cached_sftp.spawn(responder, true, |sftp| async move {
                            write_file_text_on_session(&sftp, &path, &content).await
                        });
                    }
                    TerminalControl::RenamePath {
                        from_path,
                        to_path,
                        responder,
                    } => {
                        cached_sftp.spawn(responder, true, |sftp| async move {
                            rename_path_on_session(&sftp, &from_path, &to_path).await
                        });
                    }
                    TerminalControl::CopyPath {
                        from_path,
                        to_path,
                        recursive,
                        responder,
                    } => {
                        cached_sftp.spawn(responder, true, |sftp| async move {
                            copy_path_on_session(&sftp, &from_path, &to_path, recursive).await
                        });
                    }
                    TerminalControl::ChmodPath { path, mode, responder } => {
                        cached_sftp.spawn(responder, true, |sftp| async move {
                            chmod_path_on_session(&sftp, &path, mode).await
                        });
                    }
                    TerminalControl::DeletePaths {
                        request,
                        cancellation,
                        responder,
                    } => {
                        let app = app.clone();
                        let session = Arc::clone(&session);
                        let permits = Arc::clone(&delete_permits);
                        let registry = registry.clone();
                        let task_id = request.task_id.clone();
                        tauri::async_runtime::spawn(async move {
                            if responder.send(Ok(())).is_ok() {
                                run_delete_task(app, session, request, cancellation, permits).await;
                            }
                            registry.finish_delete(&task_id);
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

    cached_sftp.close().await;

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

fn validate_terminal_start_request(request: &TerminalStartRequest) -> Result<(), AppError> {
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }

    if request.host.trim().is_empty() {
        return Err(required("host"));
    }

    if request.username.trim().is_empty() {
        return Err(required("username"));
    }

    if request.port == 0 {
        return Err(crate::error::port_invalid());
    }

    match &request.auth_method {
        AuthMethod::Agent | AuthMethod::KeyboardInteractive => {}
        AuthMethod::Password => {
            if request.password.as_deref().unwrap_or_default().is_empty() {
                return Err(required("password"));
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
                return Err(required("privateKeyPath"));
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
    error: Option<AppError>,
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
