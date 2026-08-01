use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use russh::client;
use russh::Disconnect;
use tauri::{AppHandle, Emitter, State};
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::models::{TunnelEventPayload, TunnelStartRequest};
use crate::ssh::client::{authenticate_session, connect_with_tofu, ConnectionTestClient};
use crate::util::connection_error;

const TUNNEL_EVENT: &str = "tunnel-event";

struct TunnelHandle {
    shutdown: watch::Sender<bool>,
    // russh 的 Handle 未实现 Clone，所有隧道操作都只需 &self，用 Arc 共享
    session: Arc<client::Handle<ConnectionTestClient>>,
}

// 活跃隧道注册表：tunnelId → 停止信号 + SSH 会话句柄
#[derive(Clone, Default)]
pub struct TunnelRegistry {
    inner: Arc<Mutex<HashMap<String, TunnelHandle>>>,
}

impl TunnelRegistry {
    fn insert(&self, tunnel_id: String, handle: TunnelHandle) -> Result<(), String> {
        let mut tunnels = self.inner.lock().map_err(|_| "Tunnel registry poisoned")?;
        if tunnels.contains_key(&tunnel_id) {
            return Err("Tunnel is already running".to_string());
        }

        tunnels.insert(tunnel_id, handle);
        Ok(())
    }

    fn remove(&self, tunnel_id: &str) -> Option<TunnelHandle> {
        self.inner.lock().ok()?.remove(tunnel_id)
    }
}

#[tauri::command]
pub async fn start_tunnel(
    app: AppHandle,
    registry: State<'_, TunnelRegistry>,
    request: TunnelStartRequest,
) -> Result<(), String> {
    validate_tunnel_request(&request)?;

    let tunnel_id = request.tunnel_id.clone();
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    let remote_host = request.remote_host.trim().to_string();
    let remote_port = request.remote_port;

    let mut session = connect_with_tofu(&app, &host, request.port).await?;
    authenticate_session(
        &mut session,
        username,
        request.auth_method.clone(),
        request.password.clone(),
        request.private_key_path.clone(),
        request.passphrase.clone(),
    )
    .await?;
    let session = Arc::new(session);

    // 只监听回环地址：隧道不对局域网暴露本地端口
    let listener = TcpListener::bind(("127.0.0.1", request.local_port))
        .await
        .map_err(|error| format!("Local port {} unavailable: {error}", request.local_port))?;

    let (shutdown_sender, mut shutdown_receiver) = watch::channel(false);
    registry.insert(
        tunnel_id.clone(),
        TunnelHandle {
            shutdown: shutdown_sender,
            session: Arc::clone(&session),
        },
    )?;

    let registry_handle = registry.inner().clone();
    let accept_app = app.clone();
    let accept_tunnel_id = tunnel_id.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_receiver.changed() => break,
                accepted = listener.accept() => {
                    let (local_stream, peer_address) = match accepted {
                        Ok(connection) => connection,
                        Err(error) => {
                            fail_tunnel(
                                &accept_app,
                                &registry_handle,
                                &accept_tunnel_id,
                                connection_error(error),
                            );
                            break;
                        }
                    };

                    spawn_forwarding(
                        accept_app.clone(),
                        registry_handle.clone(),
                        accept_tunnel_id.clone(),
                        session.clone(),
                        local_stream,
                        peer_address,
                        remote_host.clone(),
                        remote_port,
                    );
                }
            }
        }
    });

    emit_tunnel_event(&app, &tunnel_id, "started", None);
    Ok(())
}

#[tauri::command]
pub async fn stop_tunnel(
    app: AppHandle,
    registry: State<'_, TunnelRegistry>,
    tunnel_id: String,
) -> Result<(), String> {
    let Some(handle) = registry.remove(&tunnel_id) else {
        return Ok(());
    };

    let _ = handle.shutdown.send(true);
    let _ = handle
        .session
        .disconnect(Disconnect::ByApplication, "tunnel closed", "en")
        .await;
    emit_tunnel_event(&app, &tunnel_id, "stopped", None);
    Ok(())
}

// 每个进入本地端口的连接开一个 direct-tcpip 子通道做双向拷贝
#[allow(clippy::too_many_arguments)]
fn spawn_forwarding(
    app: AppHandle,
    registry: TunnelRegistry,
    tunnel_id: String,
    session: Arc<client::Handle<ConnectionTestClient>>,
    mut local_stream: tokio::net::TcpStream,
    peer_address: std::net::SocketAddr,
    remote_host: String,
    remote_port: u16,
) {
    tauri::async_runtime::spawn(async move {
        let channel = session
            .channel_open_direct_tcpip(
                remote_host,
                u32::from(remote_port),
                peer_address.ip().to_string(),
                u32::from(peer_address.port()),
            )
            .await;

        match channel {
            Ok(channel) => {
                let mut remote_stream = channel.into_stream();
                let _ = copy_bidirectional(&mut local_stream, &mut remote_stream).await;
            }
            Err(error) => {
                // SSH 连接已死时整条隧道判失败并停掉；单次目标端口拒绝不影响隧道
                if session.is_closed() {
                    fail_tunnel(&app, &registry, &tunnel_id, connection_error(error));
                }
            }
        }
    });
}

fn fail_tunnel(app: &AppHandle, registry: &TunnelRegistry, tunnel_id: &str, error: String) {
    let Some(handle) = registry.remove(tunnel_id) else {
        return;
    };

    let _ = handle.shutdown.send(true);
    emit_tunnel_event(app, tunnel_id, "error", Some(error));
}

fn emit_tunnel_event(app: &AppHandle, tunnel_id: &str, kind: &str, error: Option<String>) {
    let _ = app.emit(
        TUNNEL_EVENT,
        TunnelEventPayload {
            tunnel_id: tunnel_id.to_string(),
            kind: kind.to_string(),
            error,
        },
    );
}

fn validate_tunnel_request(request: &TunnelStartRequest) -> Result<(), String> {
    if request.tunnel_id.trim().is_empty() {
        return Err("Tunnel id is required".to_string());
    }
    if request.host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    if request.username.trim().is_empty() {
        return Err("Username is required".to_string());
    }
    if request.remote_host.trim().is_empty() {
        return Err("Remote host is required".to_string());
    }
    if request.local_port == 0 || request.remote_port == 0 {
        return Err("Ports must be between 1 and 65535".to_string());
    }

    Ok(())
}
