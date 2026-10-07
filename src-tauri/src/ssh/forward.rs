// 终端会话上的临时端口转发：终端里 Ctrl+点击服务器上的端口，就在这条已经登录的 SSH 连接上开 direct-tcpip。
// 和「隧道」的区别：不用再握手、再认证一次，也不留配置；会话断开时一起回收。
// 只转发到服务器自己的 127.0.0.1——终端里认出来的是「这台机器上跑着的服务」。
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use russh::client;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinSet;
use tokio::time::{timeout, Duration};
use tokio_util::sync::CancellationToken;

use tauri::State;

use crate::error::{codes, lock_poisoned, port_invalid, required, AppError};
use crate::ssh::client::ConnectionTestClient;
use crate::ssh::registry::TerminalRegistry;

type SharedSession = Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>;

const CHANNEL_OPEN_TIMEOUT_SECS: u64 = 12;
static NEXT_FORWARD_ID: AtomicU64 = AtomicU64::new(1);

// 返回本机端口：优先和服务器同号，被占了就是系统挑的那个
#[tauri::command]
pub async fn forward_terminal_port(
    registry: State<'_, TerminalRegistry>,
    session_id: String,
    remote_port: u16,
) -> Result<u16, AppError> {
    if session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    if remote_port == 0 {
        return Err(port_invalid());
    }
    // 开通道自己有时限；这里再兜一层，会话卡死时请求也能结束
    timeout(Duration::from_secs(CHANNEL_OPEN_TIMEOUT_SECS + 3), registry.forward_port(&session_id, remote_port))
        .await
        .map_err(|_| crate::error::timeout("forwardPort", CHANNEL_OPEN_TIMEOUT_SECS + 3))?
}

#[tauri::command]
pub fn stop_terminal_forward(
    registry: State<'_, TerminalRegistry>,
    session_id: String,
    local_port: u16,
) -> Result<(), AppError> {
    registry.stop_forward(&session_id, local_port)
}

#[derive(Clone, Default)]
pub(crate) struct PortForwards {
    // 本机端口 → (这次转发的编号, 停止信号)。编号用来认出「是不是同一次」：
    // 停掉以后同一个端口可能马上又被新的转发占上，旧任务收尾时不能把新记录删掉
    active: Arc<Mutex<HashMap<u16, (u64, CancellationToken)>>>,
}

impl PortForwards {
    // 本机端口先试和服务器同号，好记；被占了就让系统挑一个。
    // 绑定后先试开一次通道：sshd 不允许转发、或者那个端口上根本没有服务，当场就能告诉用户，
    // 而不是等浏览器连不上才发现
    pub(crate) async fn start(
        &self,
        session: SharedSession,
        remote_port: u16,
        parent: &CancellationToken,
    ) -> Result<u16, AppError> {
        let listener = match TcpListener::bind(("127.0.0.1", remote_port)).await {
            Ok(listener) => listener,
            Err(_) => TcpListener::bind(("127.0.0.1", 0))
                .await
                .map_err(|error| listener_error(remote_port, error))?,
        };
        let local_port = listener
            .local_addr()
            .map_err(|error| listener_error(remote_port, error))?
            .port();
        // 裸 Channel 丢掉不会自己关，得明说，不然服务器那头这条试探连接一直挂着
        let probe = open_channel(&session, remote_port, local_port).await?;
        let _ = probe.close().await;

        let id = NEXT_FORWARD_ID.fetch_add(1, Ordering::Relaxed);
        let cancellation = parent.child_token();
        self.active
            .lock()
            .map_err(|_| lock_poisoned())?
            .insert(local_port, (id, cancellation.clone()));

        let active = Arc::clone(&self.active);
        tauri::async_runtime::spawn(async move {
            serve(listener, session, remote_port, cancellation).await;
            if let Ok(mut active) = active.lock() {
                if active.get(&local_port).is_some_and(|(current, _)| *current == id) {
                    active.remove(&local_port);
                }
            }
        });
        Ok(local_port)
    }

    pub(crate) fn stop(&self, local_port: u16) -> Result<(), AppError> {
        if let Some((_, cancellation)) = self.active.lock().map_err(|_| lock_poisoned())?.remove(&local_port) {
            cancellation.cancel();
        }
        Ok(())
    }
}

async fn serve(listener: TcpListener, session: SharedSession, remote_port: u16, cancellation: CancellationToken) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept() => {
                let Ok((local_stream, peer)) = accepted else { break };
                connections.spawn(forward_connection(Arc::clone(&session), local_stream, remote_port, peer.port()));
            }
        }
    }
    // 停止不能只关监听端口：浏览器开着的长连接也要一起断掉
    drop(listener);
    connections.abort_all();
    while connections.join_next().await.is_some() {}
}

// 单个连接打不开只结束这一个，转发本身还在：服务可能只是在重启
async fn forward_connection(session: SharedSession, mut local_stream: TcpStream, remote_port: u16, peer_port: u16) {
    if let Ok(channel) = open_channel(&session, remote_port, peer_port).await {
        let mut remote_stream = channel.into_stream();
        let _ = copy_bidirectional(&mut local_stream, &mut remote_stream).await;
    }
}

async fn open_channel(
    session: &SharedSession,
    remote_port: u16,
    originator_port: u16,
) -> Result<russh::Channel<client::Msg>, AppError> {
    // 时限把等锁也算进去：会话正忙着开别的子通道时，不能让浏览器那头无限等
    let open = async {
        let handle = session.lock().await;
        handle
            .channel_open_direct_tcpip("127.0.0.1", u32::from(remote_port), "127.0.0.1", u32::from(originator_port))
            .await
    };
    match timeout(Duration::from_secs(CHANNEL_OPEN_TIMEOUT_SECS), open).await {
        Ok(Ok(channel)) => Ok(channel),
        Ok(Err(russh::Error::ChannelOpenFailure(russh::ChannelOpenFailure::AdministrativelyProhibited))) => {
            Err(AppError::new(codes::FORWARD_PROHIBITED, "The server does not allow port forwarding")
                .with("port", remote_port))
        }
        Ok(Err(russh::Error::ChannelOpenFailure(russh::ChannelOpenFailure::ConnectFailed))) => {
            Err(AppError::new(codes::FORWARD_CONNECT_FAILED, format!("Nothing is listening on port {remote_port}"))
                .with("port", remote_port))
        }
        Ok(Err(error)) => Err(AppError::new(codes::FORWARD_FAILED, error.to_string())
            .with("port", remote_port)
            .with("detail", error)),
        Err(_) => Err(crate::error::timeout("forwardPort", CHANNEL_OPEN_TIMEOUT_SECS)),
    }
}

fn listener_error(port: u16, error: std::io::Error) -> AppError {
    AppError::new(codes::TUNNEL_PORT_UNAVAILABLE, error.to_string())
        .with("port", port)
        .with("detail", error)
}
