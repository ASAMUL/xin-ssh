use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::{
    client,
    keys::{load_secret_key, ssh_key, PrivateKeyWithHashAlg, PublicKey},
};
use tauri::AppHandle;
use tokio::net::{lookup_host, TcpStream};
use tokio::time::timeout;

use crate::error::{codes, connection_error, required, timeout as timeout_error, AppError};
use crate::models::{AgentConfig, AuthMethod};
use crate::ssh::agent::authenticate_agent;
use crate::ssh::known_hosts;

pub(crate) const SSH_KEEPALIVE_INTERVAL_SECS: u64 = 30;
const SSH_CONNECT_TIMEOUT_SECS: u64 = 12;
const SSH_AUTH_TIMEOUT_SECS: u64 = 12;
const SSH_AGENT_AUTH_TIMEOUT_SECS: u64 = 120;

// russh 在密钥交换期间启动内部任务；超时或取消时主动关闭套接字，避免只停止外层等待。
struct PendingConnectionSocket(Option<std::net::TcpStream>);

impl Drop for PendingConnectionSocket {
    fn drop(&mut self) {
        if let Some(socket) = self.0.take() {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct ServerKeySnapshot {
    pub(crate) fingerprint: Option<String>,
    pub(crate) algorithm: Option<String>,
}

pub(crate) struct ConnectionTestClient {
    pub(crate) server_key: Arc<Mutex<ServerKeySnapshot>>,
    // Some(fingerprint) 时启用 TOFU 校验：指纹不一致直接拒绝连接（None = 首次连接，记录后信任）
    pub(crate) expected_fingerprint: Option<String>,
}

impl client::Handler for ConnectionTestClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key
            .fingerprint(ssh_key::HashAlg::Sha256)
            .to_string();

        if let Ok(mut snapshot) = self.server_key.lock() {
            snapshot.fingerprint = Some(fingerprint.clone());
            snapshot.algorithm = Some(server_public_key.algorithm().to_string());
        }

        if let Some(expected) = &self.expected_fingerprint {
            return Ok(&fingerprint == expected);
        }

        Ok(true)
    }
}

// 三种入口共用 DNS、TCP 和 SSH 握手时限，测试成功不能依赖正式连接没有的等待策略。
pub(crate) async fn connect_transport(
    host: &str,
    port: u16,
    keepalive_seconds: Option<u64>,
    handler: ConnectionTestClient,
) -> Result<client::Handle<ConnectionTestClient>, AppError> {
    timeout(Duration::from_secs(SSH_CONNECT_TIMEOUT_SECS), async {
        let addresses: Vec<_> = lookup_host((host, port))
            .await
            .map_err(|error| {
                AppError::new(codes::SSH_RESOLVE_FAILED, error.to_string())
                    .with("host", host)
                    .with("detail", error)
            })?
            .collect();
        if addresses.is_empty() {
            return Err(AppError::new(codes::SSH_RESOLVE_FAILED, "No addresses resolved")
                .with("host", host)
                .with("detail", "No addresses resolved"));
        }
        let socket = TcpStream::connect(addresses.as_slice())
            .await
            .map_err(connection_error)?
            .into_std()
            .map_err(connection_error)?;
        let mut pending =
            PendingConnectionSocket(Some(socket.try_clone().map_err(connection_error)?));
        let socket = TcpStream::from_std(socket).map_err(connection_error)?;
        let keepalive = keepalive_seconds.unwrap_or(SSH_KEEPALIVE_INTERVAL_SECS);
        let config = Arc::new(client::Config {
            // 空闲终端不应被建连时限踢下线，运行期仍由 keepalive 判断连接健康。
            inactivity_timeout: None,
            keepalive_interval: (keepalive > 0).then(|| Duration::from_secs(keepalive)),
            ..Default::default()
        });
        if config.nodelay {
            socket.set_nodelay(true).map_err(connection_error)?;
        }
        let session = client::connect_stream(config, socket, handler)
            .await
            .map_err(connection_error)?;
        drop(pending.0.take());
        Ok(session)
    })
    .await
    .map_err(|_| timeout_error("connectSsh", SSH_CONNECT_TIMEOUT_SECS))?
}

// 建立 SSH TCP 连接并做 TOFU 指纹校验：已有记录则校验（不一致报 HOST_KEY_CHANGED），
// 首次连接成功后记录指纹。终端与隧道共用。
// keepalive_seconds: None = 默认 30s，Some(0) = 禁用。
pub(crate) async fn connect_with_tofu(
    app: &AppHandle,
    host: &str,
    port: u16,
    keepalive_seconds: Option<u64>,
) -> Result<client::Handle<ConnectionTestClient>, AppError> {
    let expected_fingerprint = known_hosts::lookup_fingerprint(app, host, port)?;
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
        expected_fingerprint: expected_fingerprint.clone(),
    };
    let session = match connect_transport(host, port, keepalive_seconds, handler).await {
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

            return Err(error);
        }
    };

    if expected_fingerprint.is_none() {
        let snapshot = server_key
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default();
        if let Some(fingerprint) = &snapshot.fingerprint {
            let _ = known_hosts::record_host_key(
                app,
                host,
                port,
                fingerprint,
                snapshot.algorithm.as_deref(),
            );
        }
    }

    Ok(session)
}

// 认证服务器不响应时及时结束；私钥读取和解密移出异步 worker，连接测试复用同一条路径。
pub(crate) async fn authenticate_session(
    session: &mut client::Handle<ConnectionTestClient>,
    username: String,
    auth_method: AuthMethod,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
    agent: Option<AgentConfig>,
) -> Result<(), AppError> {
    // agent 可能等待系统解锁或硬件触摸，允许交互确认但仍保留整体时限。
    let auth_timeout = if matches!(&auth_method, AuthMethod::Agent) {
        SSH_AGENT_AUTH_TIMEOUT_SECS
    } else {
        SSH_AUTH_TIMEOUT_SECS
    };
    let result = timeout(
        Duration::from_secs(auth_timeout),
        authenticate_with_credentials(
            session,
            username,
            auth_method,
            password,
            private_key_path,
            passphrase,
            agent,
        ),
    )
    .await
    .map_err(|_| timeout_error("authenticateSsh", auth_timeout))
    .and_then(|result| result);
    if result.is_err() {
        disconnect_session(session, "authentication failed").await;
    }
    result
}

// 所有入口共用同一套凭据校验和算法协商，避免测试成功但正式认证行为不同。
async fn authenticate_with_credentials(
    session: &mut client::Handle<ConnectionTestClient>,
    username: String,
    auth_method: AuthMethod,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
    agent: Option<AgentConfig>,
) -> Result<(), AppError> {
    let auth_result = match auth_method {
        AuthMethod::Agent => {
            return authenticate_agent(session, &username, &agent.unwrap_or_default()).await;
        }
        AuthMethod::Password => {
            let password = password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| required("password"))?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| required("privateKeyPath"))?;
            let key_path = private_key_path.clone();
            let key_pair = tokio::task::spawn_blocking(move || {
                let passphrase = passphrase.as_deref().filter(|value| !value.is_empty());
                load_secret_key(&key_path, passphrase).map_err(|error| {
                    AppError::new(codes::AUTH_KEY_FILE, error.to_string())
                        .with("path", &key_path)
                        .with("detail", error)
                })
            })
            .await
            .map_err(|error| {
                AppError::new(codes::AUTH_KEY_FILE, error.to_string())
                    .with("path", &private_key_path)
                    .with("detail", error)
            })??;
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
        Err(AppError::new(codes::AUTH_FAILED, "Authentication failed"))
    }
}

pub(crate) async fn disconnect_session(session: &client::Handle<ConnectionTestClient>, reason: &str) {
    // 失败清理不能成为另一处无限等待，也不能让测试结果被断开确认拖住。
    let _ = timeout(
        Duration::from_secs(1),
        session.disconnect(russh::Disconnect::ByApplication, reason, "en"),
    )
    .await;
}
