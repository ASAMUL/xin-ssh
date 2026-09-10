use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use russh::client;
use tauri::{AppHandle, Emitter, State};
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::{interval, timeout, Duration, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use crate::error::{codes, lock_poisoned, required, AppError};
use crate::models::{TunnelEventPayload, TunnelStartRequest};
use crate::ssh::client::{
    authenticate_session, connect_with_tofu, disconnect_session, ConnectionTestClient,
};

const TUNNEL_EVENT: &str = "tunnel-event";

#[derive(Clone)]
struct TunnelHandle {
    attempt_id: String,
    cancellation: CancellationToken,
    finished: watch::Receiver<bool>,
}

// 启动中的隧道也占位，避免重复握手，并让停止覆盖端口绑定和认证阶段。
#[derive(Clone, Default)]
pub struct TunnelRegistry {
    inner: Arc<Mutex<HashMap<String, TunnelHandle>>>,
}

impl TunnelRegistry {
    fn insert(&self, tunnel_id: String, handle: TunnelHandle) -> Result<(), AppError> {
        let mut tunnels = self.inner.lock().map_err(|_| lock_poisoned())?;
        if tunnels.contains_key(&tunnel_id) {
            return Err(AppError::new(
                codes::TUNNEL_ALREADY_RUNNING,
                "Tunnel is already starting or running",
            ));
        }
        tunnels.insert(tunnel_id, handle);
        Ok(())
    }

    fn find_attempt(&self, tunnel_id: &str, attempt_id: &str) -> Result<Option<TunnelHandle>, AppError> {
        let tunnels = self.inner.lock().map_err(|_| lock_poisoned())?;
        Ok(tunnels.get(tunnel_id)
            .filter(|handle| handle.attempt_id == attempt_id)
            .cloned())
    }

    fn finish(&self, tunnel_id: &str, attempt_id: &str) {
        if let Ok(mut tunnels) = self.inner.lock() {
            if tunnels.get(tunnel_id).is_some_and(|handle| handle.attempt_id == attempt_id) {
                tunnels.remove(tunnel_id);
            }
        }
    }
}

#[tauri::command]
pub fn start_tunnel(
    app: AppHandle,
    registry: State<'_, TunnelRegistry>,
    request: TunnelStartRequest,
) -> Result<(), AppError> {
    validate_tunnel_request(&request)?;
    let cancellation = CancellationToken::new();
    let (finished_sender, finished) = watch::channel(false);
    registry.insert(request.tunnel_id.clone(), TunnelHandle {
        attempt_id: request.attempt_id.clone(),
        cancellation: cancellation.clone(),
        finished,
    })?;

    // IPC 只确认登记完成；前端据此保证停止请求不会抢在启动登记之前到达。
    let registry = registry.inner().clone();
    tauri::async_runtime::spawn(async move {
        let mut request = request;
        let result = run_tunnel(&app, &mut request, &cancellation).await;
        // run_tunnel 先释放监听和所有转发连接，再允许同一配置重新启动。
        registry.finish(&request.tunnel_id, &request.attempt_id);
        let error = if cancellation.is_cancelled() { None } else { result.err() };
        emit_tunnel_event(&app, &request, if error.is_some() { "error" } else { "stopped" }, error);
        let _ = finished_sender.send(true);
    });
    Ok(())
}

// 监听先于 SSH 握手建立，端口冲突立即报错；资源全部由这一任务持有和回收。
async fn run_tunnel(
    app: &AppHandle,
    request: &mut TunnelStartRequest,
    cancellation: &CancellationToken,
) -> Result<(), AppError> {
    let setup = async {
        // 仅监听回环地址，避免把服务器服务意外暴露给局域网。
        let listener = TcpListener::bind(("127.0.0.1", request.local_port))
            .await
            .map_err(|error| {
                AppError::new(codes::TUNNEL_PORT_UNAVAILABLE, error.to_string())
                    .with("port", request.local_port)
                    .with("detail", error)
            })?;
        let mut session = connect_with_tofu(
            app, request.host.trim(), request.port, request.keepalive_seconds,
        ).await?;
        authenticate_session(
            &mut session,
            request.username.trim().to_string(),
            request.auth_method.clone(),
            request.password.take(),
            request.private_key_path.take(),
            request.passphrase.take(),
            request.agent.take(),
        ).await?;
        Ok::<_, AppError>((listener, Arc::new(session)))
    };
    let (listener, session) = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Ok(()),
        result = setup => result?,
    };

    let mut connections = JoinSet::new();
    let mut health = interval(Duration::from_secs(1));
    health.set_missed_tick_behavior(MissedTickBehavior::Skip);
    emit_tunnel_event(app, request, "started", None);

    let result = loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break Ok(()),
            _ = health.tick() => {
                // 即使没有新的本地连接，也要把 russh 已检测到的断线同步到界面。
                if session.is_closed() {
                    break Err(AppError::new(codes::SSH_DISCONNECTED, "Tunnel SSH session closed"));
                }
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {},
            accepted = listener.accept() => {
                let (local_stream, peer_address) = match accepted {
                    Ok(connection) => connection,
                    Err(error) => break Err(AppError::new(codes::TUNNEL_LISTENER_FAILED, error.to_string())
                        .with("port", request.local_port).with("detail", error)),
                };
                connections.spawn(forward_connection(
                    Arc::clone(&session),
                    local_stream,
                    peer_address,
                    request.remote_host.trim().to_string(),
                    request.remote_port,
                ));
            }
        }
    };

    drop(listener);
    // 停止不能只关监听端口；已接受的本地连接和未完成的通道打开也必须回收。
    connections.abort_all();
    while connections.join_next().await.is_some() {}
    disconnect_session(&session, "tunnel closed").await;
    result
}

#[tauri::command]
pub async fn stop_tunnel(
    registry: State<'_, TunnelRegistry>,
    tunnel_id: String,
    attempt_id: String,
) -> Result<(), AppError> {
    if tunnel_id.trim().is_empty() { return Err(required("tunnelId")); }
    if attempt_id.trim().is_empty() { return Err(required("attemptId")); }
    let Some(mut handle) = registry.find_attempt(&tunnel_id, &attempt_id)? else {
        return Ok(());
    };
    handle.cancellation.cancel();
    // 等监听和转发连接释放后再确认停止，紧接着重启才不会撞到自己的旧端口。
    while !*handle.finished.borrow() {
        handle.finished.changed().await.map_err(|error| {
            AppError::new(codes::TUNNEL_STOP_FAILED, error.to_string()).with("detail", error)
        })?;
    }
    Ok(())
}

async fn forward_connection(
    session: Arc<client::Handle<ConnectionTestClient>>,
    mut local_stream: TcpStream,
    peer_address: std::net::SocketAddr,
    remote_host: String,
    remote_port: u16,
) {
    // 单次目标拒绝或不响应只结束该连接，整条隧道是否死亡由主任务统一判断。
    let channel = timeout(Duration::from_secs(12), session.channel_open_direct_tcpip(
        remote_host,
        u32::from(remote_port),
        peer_address.ip().to_string(),
        u32::from(peer_address.port()),
    )).await;
    if let Ok(Ok(channel)) = channel {
        let mut remote_stream = channel.into_stream();
        let _ = copy_bidirectional(&mut local_stream, &mut remote_stream).await;
    }
}

fn emit_tunnel_event(app: &AppHandle, request: &TunnelStartRequest, kind: &str, error: Option<AppError>) {
    let _ = app.emit(TUNNEL_EVENT, TunnelEventPayload {
        tunnel_id: request.tunnel_id.clone(),
        attempt_id: request.attempt_id.clone(),
        kind: kind.to_string(),
        error,
    });
}

fn validate_tunnel_request(request: &TunnelStartRequest) -> Result<(), AppError> {
    if request.tunnel_id.trim().is_empty() { return Err(required("tunnelId")); }
    if request.attempt_id.trim().is_empty() { return Err(required("attemptId")); }
    if request.host.trim().is_empty() { return Err(required("host")); }
    if request.username.trim().is_empty() { return Err(required("username")); }
    if request.remote_host.trim().is_empty() { return Err(required("remoteHost")); }
    if request.local_port == 0 || request.remote_port == 0 || request.port == 0 {
        return Err(crate::error::port_invalid());
    }
    Ok(())
}
