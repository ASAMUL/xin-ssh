use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use russh::{client, ChannelMsg, Disconnect};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio::time::Duration;

use crate::models::{
    AuthMethod, TerminalEventPayload, TerminalInputRequest, TerminalResizeRequest,
    TerminalStartRequest,
};
use crate::sftp::ops::{
    create_directory_on_session, delete_paths_on_session, list_directory_on_session,
    rename_path_on_session,
};
use crate::sftp::transfer::{
    remove_transfer_cancellation, run_download_transfer, run_upload_transfer,
};
use crate::ssh::client::{authenticate_session, ConnectionTestClient, ServerKeySnapshot};
use crate::ssh::registry::{TerminalControl, TerminalRegistry};
use crate::system_info::collect_system_info_on_session;
use crate::util::{connection_error, shell_quote};

const TERMINAL_EVENT: &str = "terminal-event";
const SSH_KEEPALIVE_INTERVAL_SECS: u64 = 30;
const TERMINAL_CLOSE_REASON_CLIENT: &str = "client";
const TERMINAL_CLOSE_REASON_REMOTE: &str = "remote";
const TERMINAL_CLOSE_REASON_ERROR: &str = "error";

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
    let transfer_cancellations = Arc::clone(&registry.transfer_cancellations);

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
            run_terminal_shell(app.clone(), request, control_receiver, transfer_cancellations).await
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
    transfer_cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
) -> Result<(), String> {
    let session_id = request.session_id.clone();
    let attempt_id = request.attempt_id.clone();
    let mut close_reason = TERMINAL_CLOSE_REASON_REMOTE;
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(SSH_KEEPALIVE_INTERVAL_SECS)),
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
                        let result = create_directory_on_session(
                            Arc::clone(&session),
                            &path,
                        )
                        .await;
                        let _ = responder.send(result);
                    }
                    TerminalControl::RenamePath {
                        from_path,
                        to_path,
                        responder,
                    } => {
                        let result = rename_path_on_session(
                            Arc::clone(&session),
                            &from_path,
                            &to_path,
                        )
                        .await;
                        let _ = responder.send(result);
                    }
                    TerminalControl::DeletePaths {
                        entries,
                        recursive,
                        responder,
                    } => {
                        let result = delete_paths_on_session(
                            Arc::clone(&session),
                            &entries,
                            recursive,
                        )
                        .await;
                        let _ = responder.send(result);
                    }
                    TerminalControl::StartUpload {
                        request,
                        cancellation,
                    } => {
                        let app = app.clone();
                        let session = Arc::clone(&session);
                        let task_id = request.task_id.clone();
                        let transfer_cancellations = Arc::clone(&transfer_cancellations);
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
                        let transfer_cancellations = Arc::clone(&transfer_cancellations);
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
                        emit_terminal_event(
                            &app,
                            &session_id,
                            attempt_id.as_deref(),
                            "output",
                            Some(String::from_utf8_lossy(&data).to_string()),
                            None,
                            None,
                        );
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => {
                        close_reason = TERMINAL_CLOSE_REASON_REMOTE;
                        break;
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        emit_terminal_event(
                            &app,
                            &session_id,
                            attempt_id.as_deref(),
                            "output",
                            Some(format!("\r\n[exit status: {exit_status}]\r\n")),
                            None,
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
