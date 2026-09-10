use std::collections::HashSet;
use std::time::Duration;

use russh::{
    client::{AuthResult, Handle},
    keys::{
        agent::{
            client::{AgentClient, AgentStream},
            AgentIdentity,
        },
        ssh_key,
    },
    AgentAuthError,
};
use tokio::time::timeout;

use crate::error::{codes, connection_error, timeout as timeout_error, AppError};
use crate::models::{AgentConfig, AgentIdentityInfo, AgentProvider};
use crate::ssh::client::ConnectionTestClient;

const AGENT_CONNECT_TIMEOUT_SECS: u64 = 12;
type LocalAgent = AgentClient<Box<dyn AgentStream + Send + Unpin>>;

// 只暴露公钥摘要，私钥与签名确认始终由用户选择的本机 agent 管理。
#[tauri::command]
pub async fn list_agent_identities(
    config: Option<AgentConfig>,
) -> Result<Vec<AgentIdentityInfo>, AppError> {
    let (_, identities) = load_identities(&config.unwrap_or_default()).await?;
    let mut seen = HashSet::new();
    Ok(identities
        .into_iter()
        .filter_map(|identity| {
            let fingerprint = identity_fingerprint(&identity);
            if !seen.insert(fingerprint.clone()) {
                return None;
            }
            Some(AgentIdentityInfo {
                fingerprint,
                comment: identity.comment().to_string(),
                algorithm: identity.public_key().algorithm().to_string(),
            })
        })
        .collect())
}

// 提供指纹时只尝试对应密钥，避免其他身份消耗 sshd 的 MaxAuthTries。
// 同一公钥的证书和普通身份均保留，按 agent 返回顺序交给服务器判断。
pub(crate) async fn authenticate_agent(
    session: &mut Handle<ConnectionTestClient>,
    username: &str,
    config: &AgentConfig,
) -> Result<(), AppError> {
    let (mut agent, mut identities) = load_identities(config).await?;
    if identities.is_empty() {
        return Err(AppError::new(
            codes::AUTH_AGENT_EMPTY,
            "The SSH agent has no identities",
        ));
    }
    if let Some(fingerprint) = config.key_fingerprint.as_deref()
        .map(str::trim).filter(|key| !key.is_empty())
    {
        identities.retain(|identity| identity_fingerprint(identity) == fingerprint);
        if identities.is_empty() {
            return Err(AppError::new(codes::AUTH_AGENT_KEY_MISSING, "Selected identity is absent")
                .with("fingerprint", fingerprint));
        }
    }
    let rsa_hash = session.best_supported_rsa_hash().await
        .map_err(connection_error)?.flatten();
    for identity in identities {
        let result = match identity {
            AgentIdentity::PublicKey { key, .. } => {
                session.authenticate_publickey_with(username, key, rsa_hash, &mut agent)
                    .await
            }
            AgentIdentity::Certificate { certificate, .. } => {
                session.authenticate_certificate_with(username, certificate, rsa_hash, &mut agent)
                    .await
            }
        }.map_err(|error| match error {
            AgentAuthError::Send(error) => connection_error(error),
            AgentAuthError::Key(error) => agent_error(codes::AUTH_AGENT_FAILED, error),
        })?;
        match result {
            AuthResult::Success => return Ok(()),
            AuthResult::Failure { partial_success: true, .. } => {
                // 服务器已接受密钥时继续枚举会重复认证，二次认证须由后续交互流程处理。
                return Err(AppError::new(
                    codes::AUTH_ADDITIONAL_REQUIRED,
                    "Additional authentication is required",
                ));
            }
            AuthResult::Failure { remaining_methods, .. } => {
                if !remaining_methods.contains(&russh::MethodKind::PublicKey) {
                    break;
                }
            }
        }
    }
    Err(AppError::new(
        codes::AUTH_AGENT_REJECTED,
        "The server rejected the agent identities",
    ))
}

async fn load_identities(config: &AgentConfig) -> Result<(LocalAgent, Vec<AgentIdentity>), AppError> {
    // 包住管道等待和列表读取；失联或忙碌的 agent 不能让连接一直停在准备阶段。
    timeout(Duration::from_secs(AGENT_CONNECT_TIMEOUT_SECS), async {
        let mut agent = connect_agent(config).await?;
        let identities = agent.request_identities().await
            .map_err(|error| agent_error(codes::AUTH_AGENT_FAILED, error))?;
        Ok((agent, identities))
    }).await.map_err(|_| timeout_error("connectAgent", AGENT_CONNECT_TIMEOUT_SECS))?
}

// 平台传输使用 russh 的实现，显式选择来源以免悄悄改用另一个 agent 中的身份。
async fn connect_agent(config: &AgentConfig) -> Result<LocalAgent, AppError> {
    match &config.provider {
        AgentProvider::System => {
            let path = config.socket_path.as_deref()
                .map(str::trim).filter(|path| !path.is_empty());
            #[cfg(windows)]
            {
                AgentClient::connect_named_pipe(path.unwrap_or(r"\\.\pipe\openssh-ssh-agent"))
                    .await.map(AgentClient::dynamic)
                    .map_err(|error| agent_error(codes::AUTH_AGENT_UNAVAILABLE, error))
            }
            #[cfg(unix)]
            {
                let agent = match path {
                    Some(path) => AgentClient::connect_uds(path).await,
                    None => AgentClient::connect_env().await,
                };
                agent.map(AgentClient::dynamic)
                    .map_err(|error| agent_error(codes::AUTH_AGENT_UNAVAILABLE, error))
            }
            #[cfg(not(any(windows, unix)))]
            {
                let _ = path;
                Err(unsupported_agent())
            }
        }
        AgentProvider::Pageant => {
            #[cfg(windows)]
            {
                AgentClient::connect_pageant().await.map(AgentClient::dynamic)
                    .map_err(|error| agent_error(codes::AUTH_AGENT_UNAVAILABLE, error))
            }
            #[cfg(not(windows))]
            {
                Err(unsupported_agent())
            }
        }
    }
}

#[cfg(not(windows))]
fn unsupported_agent() -> AppError {
    AppError::new(
        codes::AUTH_AGENT_UNSUPPORTED,
        "This agent provider is unsupported on this platform",
    )
}

fn identity_fingerprint(identity: &AgentIdentity) -> String {
    identity.public_key().fingerprint(ssh_key::HashAlg::Sha256).to_string()
}

fn agent_error(code: &'static str, error: impl std::fmt::Display) -> AppError {
    AppError::new(code, error.to_string()).with("detail", error)
}
