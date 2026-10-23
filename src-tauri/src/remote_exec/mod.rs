//! 在服务器上执行一段 sh 脚本并按行读回输出：目录占用分析、压缩解压、Docker 容器列表共用。
//! 每次开一个临时 exec 通道，用完即关，不碰用户正在用的终端。

pub(crate) mod disk_usage;

use std::sync::Arc;

use russh::{client, ChannelMsg, Pty};
use tauri::AppHandle;
use tokio::sync::{oneshot, Mutex as AsyncMutex, Semaphore};
use tokio::time::{sleep_until, Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::error::{codes, connection_error, timeout as timeout_error, AppError};
use crate::models::DiskUsageRequest;
use crate::ssh::client::ConnectionTestClient;
use crate::util::shell_quote;

pub(crate) type SshSession = Arc<AsyncMutex<client::Handle<ConnectionTestClient>>>;

// 报错总在输出末尾；前面可能是成千上万行文件名，只留最后这一截给用户看
const OUTPUT_TAIL_BYTES: usize = 4096;
// 一行再长也只留这么多，防止没有换行的输出把内存吃满
const MAX_LINE_BYTES: usize = 64 * 1024;
// exec 通道不读 profile，/usr/sbin 下的工具、手装的 docker 可能不在默认 PATH 里
const SCRIPT_PREAMBLE: &str =
    "PATH=\"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH\"; export PATH\n";
// 脚本自己报告的信息（缺哪个工具、最终产物路径）以此开头，和命令本身的输出分开
pub(crate) const MARKER: &str = "@@xin ";
// 脚本约定：缺工具时以 127 退出，并先打印 `@@xin missing <工具名>`
pub(crate) const EXIT_TOOL_MISSING: u32 = 127;

/// 长任务要能停下来：申请 PTY 后关通道，sshd 收回终端，整组进程收到 SIGHUP 一起退出。
/// 不开 PTY 时关通道只断开管道，大目录的 du、tar 压缩这类不怎么输出的命令会在后台一直跑完。
pub(crate) struct CommandOptions {
    pub pty: bool,
    pub timeout: Option<Duration>,
}

pub(crate) struct CommandOutcome {
    pub exit_status: Option<u32>,
    pub tail: String,
}

impl CommandOutcome {
    pub(crate) fn succeeded(&self) -> bool {
        self.exit_status == Some(0)
    }

    /// 脚本报告的缺失工具名；用来给出「请先安装 xxx」而不是一句笼统的命令失败
    pub(crate) fn missing_tool(&self) -> Option<String> {
        if self.exit_status != Some(EXIT_TOOL_MISSING) {
            return None;
        }

        self.tail
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix(MARKER)?.strip_prefix("missing "))
            .map(|tool| tool.trim().to_string())
    }

    pub(crate) fn failure(&self, operation: &str) -> AppError {
        if let Some(tool) = self.missing_tool() {
            return AppError::new(codes::REMOTE_TOOL_MISSING, format!("{tool} is not installed on the server"))
                .with("tool", tool);
        }
        // exec 被换成 internal-sftp 的账号什么都不输出，退出码也不是脚本给的
        let detail = self
            .tail
            .lines()
            .filter(|line| !line.starts_with(MARKER) && !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if detail.is_empty() && self.exit_status.is_none_or(|status| status != 0) {
            return exec_unavailable();
        }

        AppError::new(codes::REMOTE_TASK_FAILED, format!("{operation} failed: {detail}"))
            .with("operation", operation)
            .with("detail", last_lines(&detail, 6))
    }
}

/// 投递到会话事件循环的后台命令任务：只有循环里拿得到这条 SSH 连接。
pub(crate) enum CommandTask {
    DiskUsage(DiskUsageRequest),
}

impl CommandTask {
    pub(crate) fn task_id(&self) -> &str {
        match self {
            Self::DiskUsage(request) => &request.task_id,
        }
    }

    pub(crate) fn session_id(&self) -> &str {
        match self {
            Self::DiskUsage(request) => &request.session_id,
        }
    }

    pub(crate) async fn run(self, app: AppHandle, session: SshSession, cancellation: CancellationToken) {
        match self {
            Self::DiskUsage(request) => disk_usage::run_disk_usage(app, session, request, cancellation).await,
        }
    }
}

/// 拼出交给登录 shell 的命令：脚本整段单引号包住，参数走位置参数，不拼进脚本。
pub(crate) fn sh_command(script: &str, args: &[&str]) -> String {
    let mut command = format!("sh -c {} xin", shell_quote(&format!("{SCRIPT_PREAMBLE}{script}")));
    for arg in args {
        command.push(' ');
        command.push_str(&shell_quote(arg));
    }
    command
}

pub(crate) async fn run_command(
    session: &SshSession,
    command: String,
    options: CommandOptions,
    cancellation: &CancellationToken,
    mut on_line: impl FnMut(&str),
) -> Result<CommandOutcome, AppError> {
    let mut channel = {
        let session = session.lock().await;
        session.channel_open_session().await.map_err(connection_error)?
    };
    if options.pty {
        // 关掉回显和 \n→\r\n 转换，输出才能按行解析；不要回复：服务器不给开终端（PermitTTY no）时照样执行
        let modes = [(Pty::ECHO, 0), (Pty::ONLCR, 0)];
        let _ = channel.request_pty(false, "dumb", 512, 24, 0, 0, &modes).await;
    }
    channel.exec(true, command).await.map_err(connection_error)?;
    // 只开了 SFTP 的账号会把命令换成 internal-sftp，不告诉它输入结束它就一直等
    let _ = channel.eof().await;

    let deadline = options.timeout.map(|limit| (Instant::now() + limit, limit.as_secs()));
    let mut output = OutputLines::default();
    let mut exit_status = None;

    let result = loop {
        let expire = async {
            match deadline {
                Some((at, _)) => sleep_until(at).await,
                None => std::future::pending::<()>().await,
            }
        };

        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break Err(command_canceled()),
            _ = expire => break Err(timeout_error("remoteCommand", deadline.map_or(0, |(_, secs)| secs))),
            message = channel.wait() => match message {
                Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. }) => {
                    output.push(&data, &mut on_line);
                }
                Some(ChannelMsg::ExitStatus { exit_status: status }) => exit_status = Some(status),
                // 服务器拒绝执行命令（没有 shell、被策略禁止）
                Some(ChannelMsg::Failure) => break Err(exec_unavailable()),
                // Eof 之后还可能有 ExitStatus，等通道真正关闭再结束
                Some(ChannelMsg::Close) | None => break Ok(()),
                Some(_) => {}
            },
        }
    };

    if result.is_err() {
        let _ = channel.close().await;
    }
    result?;
    output.finish(&mut on_line);

    Ok(CommandOutcome {
        exit_status,
        tail: output.tail(),
    })
}

/// 同一会话上的后台命令任务：先回执再执行，执行期间占一个名额。
/// sshd 默认 MaxSessions 是 10，终端、SFTP、传输、监控也要开通道，不能让后台命令把名额吃光。
pub(crate) async fn acknowledge_then_run<F>(
    responder: oneshot::Sender<Result<(), AppError>>,
    permits: Arc<Semaphore>,
    cancellation: CancellationToken,
    task: F,
) where
    F: std::future::Future<Output = ()>,
{
    if responder.send(Ok(())).is_err() {
        return;
    }
    let permit = tokio::select! {
        biased;
        _ = cancellation.cancelled() => None,
        permit = permits.acquire_owned() => permit.ok(),
    };
    // 排队时被取消也要让任务自己发出终态事件，所以不论拿没拿到名额都执行；任务开头会先检查取消
    let _permit = permit;
    task.await;
}

pub(crate) fn command_canceled() -> AppError {
    AppError::new(codes::FILE_OPERATION_CANCELED, "Remote command was canceled")
}

fn exec_unavailable() -> AppError {
    AppError::new(
        codes::REMOTE_EXEC_UNAVAILABLE,
        "The server does not allow running commands on this account",
    )
}

fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

/// 字节流切成行：多字节字符可能被拆在两个数据包之间，所以攒到换行再解码。
#[derive(Default)]
struct OutputLines {
    pending: Vec<u8>,
    tail: Vec<u8>,
}

impl OutputLines {
    fn push(&mut self, data: &[u8], on_line: &mut impl FnMut(&str)) {
        self.remember_tail(data);
        for &byte in data {
            if byte == b'\n' {
                self.emit(on_line);
            } else if self.pending.len() < MAX_LINE_BYTES {
                self.pending.push(byte);
            }
        }
    }

    fn finish(&mut self, on_line: &mut impl FnMut(&str)) {
        if !self.pending.is_empty() {
            self.emit(on_line);
        }
    }

    fn emit(&mut self, on_line: &mut impl FnMut(&str)) {
        let line = String::from_utf8_lossy(&self.pending);
        on_line(line.trim_end_matches('\r'));
        self.pending.clear();
    }

    fn remember_tail(&mut self, data: &[u8]) {
        self.tail.extend_from_slice(data);
        if self.tail.len() > OUTPUT_TAIL_BYTES * 2 {
            self.tail.drain(..self.tail.len() - OUTPUT_TAIL_BYTES);
        }
    }

    fn tail(&self) -> String {
        let start = self.tail.len().saturating_sub(OUTPUT_TAIL_BYTES);
        String::from_utf8_lossy(&self.tail[start..]).replace('\r', "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(chunks: &[&[u8]]) -> Vec<String> {
        let mut lines = Vec::new();
        let mut output = OutputLines::default();
        for chunk in chunks {
            output.push(chunk, &mut |line| lines.push(line.to_string()));
        }
        output.finish(&mut |line| lines.push(line.to_string()));
        lines
    }

    #[test]
    fn joins_lines_split_across_packets() {
        let text = "4\t/var/日志\r\n8\t/var\n";
        let bytes = text.as_bytes();
        let lines = collect(&[&bytes[..9], &bytes[9..]]);
        assert_eq!(lines, vec!["4\t/var/日志", "8\t/var"]);
    }

    #[test]
    fn keeps_a_trailing_line_without_newline() {
        assert_eq!(collect(&[b"done"]), vec!["done"]);
    }

    #[test]
    fn reports_the_missing_tool_from_the_marker() {
        let outcome = CommandOutcome {
            exit_status: Some(EXIT_TOOL_MISSING),
            tail: "noise\n@@xin missing unzip\n".to_string(),
        };
        assert_eq!(outcome.missing_tool().as_deref(), Some("unzip"));
        assert!(outcome.failure("extract").is(codes::REMOTE_TOOL_MISSING));
    }

    #[test]
    fn silent_failure_means_the_account_cannot_run_commands() {
        let outcome = CommandOutcome { exit_status: Some(1), tail: String::new() };
        assert!(outcome.failure("extract").is(codes::REMOTE_EXEC_UNAVAILABLE));
    }

    #[test]
    fn quotes_script_and_arguments() {
        let command = sh_command("echo \"$1\"", &["it's"]);
        assert!(command.starts_with("sh -c '"));
        assert!(command.ends_with(" xin 'it'\\''s'"));
    }
}
