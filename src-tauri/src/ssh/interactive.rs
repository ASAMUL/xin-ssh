use std::{
    collections::HashMap,
    sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex},
    time::Duration,
};

use russh::client::{Handle, KeyboardInteractiveAuthResponse, Prompt};
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tokio::sync::oneshot;
use tokio::time::{interval, timeout};

use crate::error::{codes, connection_error, timeout as timeout_error, AppError};
use crate::ssh::client::ConnectionTestClient;

const INTERACTIVE_TIMEOUT_SECS: u64 = 120;
const MAX_ROUNDS: usize = 16;
const MAX_PROMPTS: usize = 32;
static NEXT_CHALLENGE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationPrompt {
    prompt: String,
    echo: bool,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AuthenticationEvent {
    Challenge {
        challenge_id: String,
        name: String,
        instructions: String,
        prompts: Vec<AuthenticationPrompt>,
    },
    Dismissed { challenge_id: String },
    Finished,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationResponse {
    challenge_id: String,
    answers: Option<Vec<String>>,
}

struct PendingChallenge {
    prompt_count: usize,
    sender: oneshot::Sender<Option<Vec<String>>>,
}

#[derive(Clone, Default)]
pub struct AuthenticationRegistry(Arc<Mutex<HashMap<String, PendingChallenge>>>);

// 只接受当前轮次且数量匹配的答案；取消可重复调用，延迟提交不能进入下一轮认证。
#[tauri::command]
pub fn respond_authentication(
    registry: State<'_, AuthenticationRegistry>,
    request: AuthenticationResponse,
) -> Result<(), AppError> {
    let mut pending = registry.inner().0.lock().unwrap_or_else(|error| error.into_inner());
    let Some(challenge) = pending.get(&request.challenge_id) else {
        return if request.answers.is_none() { Ok(()) } else { Err(expired_challenge()) };
    };
    if let Some(answers) = &request.answers {
        if answers.len() != challenge.prompt_count || answers.iter().any(|answer| answer.len() > 16_384) {
            return Err(AppError::new(codes::AUTH_INVALID_RESPONSE, "Invalid authentication responses"));
        }
    }
    if let Some(challenge) = pending.remove(&request.challenge_id) {
        challenge.sender.send(request.answers).map_err(|_| expired_challenge())?;
    }
    Ok(())
}

pub(crate) struct AuthenticationContext {
    registry: AuthenticationRegistry,
    channel: Channel<AuthenticationEvent>,
}

impl AuthenticationContext {
    pub(crate) fn new(app: &AppHandle, channel: Channel<AuthenticationEvent>) -> Self {
        Self { registry: app.state::<AuthenticationRegistry>().inner().clone(), channel }
    }

    // responder 先于通知登记，避免快速提交在问题尚未注册时丢失。
    fn challenge(
        &self,
        name: String,
        instructions: String,
        prompts: Vec<Prompt>,
    ) -> Result<(ChallengeGuard, oneshot::Receiver<Option<Vec<String>>>), AppError> {
        let challenge_id = NEXT_CHALLENGE.fetch_add(1, Ordering::Relaxed).to_string();
        let (sender, receiver) = oneshot::channel();
        self.registry.0.lock().unwrap_or_else(|error| error.into_inner())
            .insert(challenge_id.clone(), PendingChallenge { prompt_count: prompts.len(), sender });
        let guard = ChallengeGuard {
            challenge_id: challenge_id.clone(),
            registry: self.registry.clone(),
            channel: self.channel.clone(),
        };
        self.channel.send(AuthenticationEvent::Challenge {
            challenge_id,
            name,
            instructions,
            prompts: prompts.into_iter().map(|prompt| AuthenticationPrompt {
                prompt: prompt.prompt, echo: prompt.echo,
            }).collect(),
        }).map_err(|_| canceled_authentication())?;
        Ok((guard, receiver))
    }
}

impl Drop for AuthenticationContext {
    fn drop(&mut self) {
        let _ = self.channel.send(AuthenticationEvent::Finished);
    }
}

struct ChallengeGuard {
    challenge_id: String,
    registry: AuthenticationRegistry,
    channel: Channel<AuthenticationEvent>,
}

impl Drop for ChallengeGuard {
    fn drop(&mut self) {
        // future 被超时或取消丢弃也必须撤销输入窗口和 responder，避免泄漏 OTP 等待任务。
        self.registry.0.lock().unwrap_or_else(|error| error.into_inner()).remove(&self.challenge_id);
        let _ = self.channel.send(AuthenticationEvent::Dismissed { challenge_id: self.challenge_id.clone() });
    }
}

// 多轮问题由 russh 解析；不猜测哪个问题是密码，也不自动填入旧密码或验证码。
pub(crate) async fn authenticate_interactive(
    session: &mut Handle<ConnectionTestClient>,
    username: &str,
    context: &AuthenticationContext,
) -> Result<(), AppError> {
    timeout(Duration::from_secs(INTERACTIVE_TIMEOUT_SECS), async {
        let mut response = session.authenticate_keyboard_interactive_start(username, None::<String>)
            .await.map_err(connection_error)?;
        let mut rounds = 0;
        loop {
            match response {
                KeyboardInteractiveAuthResponse::Success => return Ok(()),
                KeyboardInteractiveAuthResponse::Failure { partial_success, .. } => {
                    return Err(AppError::new(
                        if partial_success { codes::AUTH_ADDITIONAL_REQUIRED } else { codes::AUTH_INTERACTIVE_FAILED },
                        "Interactive authentication did not complete",
                    ));
                }
                KeyboardInteractiveAuthResponse::InfoRequest { name, instructions, prompts } => {
                    rounds += 1;
                    if rounds > MAX_ROUNDS || prompts.len() > MAX_PROMPTS {
                        return Err(AppError::new(codes::AUTH_INVALID_CHALLENGE, "Too many authentication prompts"));
                    }
                    let no_input = prompts.is_empty();
                    let (guard, receiver) = context.challenge(name, instructions, prompts)?;
                    if no_input {
                        // 推送确认等零输入轮次须立即回空数组，但仍展示服务器说明并允许取消。
                        response = tokio::select! {
                            biased;
                            _ = receiver => return Err(canceled_authentication()),
                            result = session.authenticate_keyboard_interactive_respond(Vec::new()) => {
                                result.map_err(connection_error)?
                            }
                        };
                    } else {
                        let answers = wait_for_answers(session, receiver).await?;
                        drop(guard);
                        response = session.authenticate_keyboard_interactive_respond(answers)
                            .await.map_err(connection_error)?;
                        continue;
                    }
                    drop(guard);
                }
            }
        }
    }).await.map_err(|_| timeout_error("interactiveAuthentication", INTERACTIVE_TIMEOUT_SECS))?
}

async fn wait_for_answers(
    session: &Handle<ConnectionTestClient>,
    mut receiver: oneshot::Receiver<Option<Vec<String>>>,
) -> Result<Vec<String>, AppError> {
    // 用户停留在输入框时也要感知 SSH 已断开，避免继续索取无效验证码。
    let mut health = interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            result = &mut receiver => return result.ok().flatten().ok_or_else(canceled_authentication),
            _ = health.tick() => {
                if session.is_closed() {
                    return Err(AppError::new(codes::SSH_DISCONNECTED, "SSH disconnected during authentication"));
                }
            }
        }
    }
}

fn canceled_authentication() -> AppError {
    AppError::new(codes::AUTH_CANCELED, "Authentication canceled")
}

fn expired_challenge() -> AppError {
    AppError::new(codes::AUTH_CHALLENGE_EXPIRED, "Authentication challenge expired")
}
