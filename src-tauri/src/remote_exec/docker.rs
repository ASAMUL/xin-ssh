//! Docker 容器：列出容器、浏览容器文件、看容器日志。
//! 浏览文件不用 docker cp：拿到容器主进程的 PID，SFTP 直接走 /proc/<PID>/root 就是容器的根目录。
//! 日志就是 docker logs -f 的输出，原样推给前端的 xterm——日志里常带 ANSI 颜色，xterm 能直接画出来。

use std::path::Path;

use tauri::ipc::Channel;
use tauri::State;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::error::{codes, required, AppError};
use crate::models::{ContainerLogEvent, ContainerLogsRequest, DockerContainer, DockerContainerList};
use crate::remote_exec::{
    last_lines, run_command, run_command_streaming, sh_command, CommandOptions, CommandOutcome, SshSession, MARKER,
};
use crate::ssh::registry::TerminalRegistry;
use crate::util::incomplete_utf8_suffix_len;

const LIST_TIMEOUT_SECS: u64 = 20;
// 不加 --tail 的 docker logs 会把容器有史以来的日志全倒出来，跑了几个月的容器能有几个 G
const MAX_LOG_TAIL_LINES: u32 = 10_000;
// docker 的报错在输出最后；再往前是容器自己的日志，里面也可能出现 permission denied 之类的字样
const ERROR_TAIL_LINES: usize = 4;

// 停掉的容器也列出来：查日志最常见的场景就是「容器起不来 / 刚崩了，为什么」。
// 顺带报告当前 uid：/proc/<PID>/root 只有 root（或和容器进程同一用户）才进得去，界面要先提醒。
// 名字和镜像里不会出现 |，拿它当分隔符。ps 和 inspect 之间有容器被删时 inspect 会报错，其余的照样列出来
const LIST_SCRIPT: &str = r#"command -v docker >/dev/null 2>&1 || { echo "@@xin missing docker"; exit 127; }
echo "@@xin uid $(id -u)"
ids=$(docker ps -aq) || exit 1
[ -n "$ids" ] || exit 0
docker inspect --format '{{.Id}}|{{.Name}}|{{.Config.Image}}|{{.State.Pid}}|{{.State.Status}}|{{.State.ExitCode}}|{{.State.StartedAt}}' $ids || true"#;

// 参数由 Rust 拼好、逐个加引号后原样交给 docker logs。开了 PTY，stdout 和 stderr 按先后混在一条流里
const LOGS_SCRIPT: &str = r#"command -v docker >/dev/null 2>&1 || { echo "@@xin missing docker"; exit 127; }
exec docker logs "$@""#;

#[tauri::command]
pub async fn list_docker_containers(
    registry: State<'_, TerminalRegistry>,
    session_id: String,
) -> Result<DockerContainerList, AppError> {
    if session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    registry.list_containers(&session_id).await
}

pub(crate) async fn list_containers_on_session(
    session: &SshSession,
    cancellation: &CancellationToken,
) -> Result<DockerContainerList, AppError> {
    let mut containers = Vec::new();
    let mut is_root = false;
    let options = CommandOptions {
        pty: false,
        timeout: Some(Duration::from_secs(LIST_TIMEOUT_SECS)),
    };
    let outcome = run_command(session, sh_command(LIST_SCRIPT, &[]), options, cancellation, |line| {
        if let Some(uid) = line.strip_prefix(MARKER).and_then(|rest| rest.strip_prefix("uid ")) {
            is_root = uid.trim() == "0";
        } else if let Some(container) = parse_container(line) {
            containers.push(container);
        }
    })
    .await?;

    if !outcome.succeeded() {
        return Err(docker_failure(&outcome, "listContainers"));
    }
    // 跑着的在前：进文件只能进它们，停掉的多半是来查日志的
    containers.sort_by(|left, right| {
        right
            .root
            .is_some()
            .cmp(&left.root.is_some())
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(DockerContainerList { containers, is_root })
}

#[tauri::command]
pub async fn start_container_logs(
    registry: State<'_, TerminalRegistry>,
    request: ContainerLogsRequest,
    events: Channel<ContainerLogEvent>,
) -> Result<(), AppError> {
    if request.task_id.trim().is_empty() {
        return Err(required("taskId"));
    }
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    // ID 会原样成为 docker logs 的参数，只认列表给出的十六进制 ID
    if !is_container_id(&request.container_id) {
        return Err(required("containerId"));
    }
    registry.start_container_logs(request, events).await
}

pub(crate) async fn run_container_logs(
    session: SshSession,
    request: ContainerLogsRequest,
    events: Channel<ContainerLogEvent>,
    cancellation: CancellationToken,
) {
    let event = if cancellation.is_cancelled() {
        ContainerLogEvent::Canceled
    } else {
        let tail = request.tail.clamp(1, MAX_LOG_TAIL_LINES).to_string();
        let since = request.since_seconds.map(|seconds| format!("{seconds}s"));
        // 停掉的容器也带 --follow：docker 读完已有的日志就退出，不用前端分两种情况
        let mut args = vec!["--follow", "--tail", tail.as_str()];
        if let Some(since) = &since {
            args.extend(["--since", since.as_str()]);
        }
        if request.timestamps {
            args.push("--timestamps");
        }
        args.extend(["--", request.container_id.as_str()]);

        // docker logs -f 等不到新日志时不会自己退出，只有收回 PTY、让它收到 SIGHUP 才停得下来
        let options = CommandOptions { pty: true, timeout: None };
        let mut output = LogOutput::new(&events);
        let result = run_command_streaming(&session, sh_command(LOGS_SCRIPT, &args), options, &cancellation, |data| {
            output.push(data);
        })
        .await;
        output.finish();

        match result {
            Ok(outcome) if outcome.succeeded() => ContainerLogEvent::Ended,
            Ok(outcome) => ContainerLogEvent::Failed { error: logs_failure(&outcome) },
            Err(error) if error.is(codes::FILE_OPERATION_CANCELED) => ContainerLogEvent::Canceled,
            Err(error) => ContainerLogEvent::Failed { error },
        }
    };
    let _ = events.send(event);
}

// 导出的是窗口里看得到的那些行：前端从 xterm 缓冲区取出纯文本，这里只负责写到用户选的位置
#[tauri::command]
pub async fn export_container_logs(path: String, content: String) -> Result<(), AppError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(required("localPath"));
    }
    tokio::fs::write(Path::new(path), content).await.map_err(|error| {
        AppError::new(codes::DOCKER_LOGS_EXPORT_FAILED, error.to_string())
            .with("path", path)
            .with("detail", error)
    })
}

/// 一个数据包推一条消息：初次回看几千行时 SSH 自己就把输出攒成大包了，跟随时一行一包也来得及。
/// 多字节字符可能被拆在两个包之间，残缺的尾巴留到下一包再解码。
struct LogOutput<'a> {
    events: &'a Channel<ContainerLogEvent>,
    pending: Vec<u8>,
}

impl<'a> LogOutput<'a> {
    fn new(events: &'a Channel<ContainerLogEvent>) -> Self {
        Self { events, pending: Vec::new() }
    }

    fn push(&mut self, data: &[u8]) {
        self.pending.extend_from_slice(data);
        let ready = self.pending.len() - incomplete_utf8_suffix_len(&self.pending);
        self.send(ready);
    }

    fn finish(&mut self) {
        self.send(self.pending.len());
    }

    fn send(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        let data = String::from_utf8_lossy(&self.pending[..len]).into_owned();
        self.pending.drain(..len);
        let _ = self.events.send(ContainerLogEvent::Output { data });
    }
}

// docker 命令自己的报错只有英文文案，挑最常见的几种给出能照着做的提示
fn docker_failure(outcome: &CommandOutcome, operation: &str) -> AppError {
    let tail = last_lines(&outcome.tail, ERROR_TAIL_LINES).to_ascii_lowercase();
    if outcome.missing_tool().is_none() && tail.contains("permission denied") {
        return AppError::new(codes::DOCKER_PERMISSION_DENIED, "Permission denied while talking to Docker");
    }
    if tail.contains("cannot connect to the docker daemon") {
        return AppError::new(codes::DOCKER_DAEMON_UNAVAILABLE, "Docker daemon is not running");
    }
    outcome.failure(operation)
}

fn logs_failure(outcome: &CommandOutcome) -> AppError {
    let tail = last_lines(&outcome.tail, ERROR_TAIL_LINES).to_ascii_lowercase();
    // 列表打开之后容器被删了
    if tail.contains("no such container") {
        return AppError::new(codes::DOCKER_CONTAINER_NOT_FOUND, "The container no longer exists");
    }
    // 日志驱动是 none、syslog 这类不在本机留底的
    if tail.contains("does not support reading") {
        return AppError::new(codes::DOCKER_LOGS_UNSUPPORTED, "The logging driver of this container does not support reading");
    }
    docker_failure(outcome, "containerLogs")
}

fn is_container_id(id: &str) -> bool {
    (12..=64).contains(&id.len()) && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_container(line: &str) -> Option<DockerContainer> {
    let mut fields = line.trim().split('|');
    let id = fields.next()?;
    let name = fields.next()?.trim_start_matches('/');
    let image = fields.next()?;
    let pid = fields.next()?.parse::<u32>().ok()?;
    let status = fields.next()?;
    let exit_code = fields.next()?.parse::<i32>().ok()?;
    let started_at = fields.next()?;
    if !is_container_id(id) || name.is_empty() || status.is_empty() || fields.next().is_some() {
        return None;
    }

    Some(DockerContainer {
        id: id[..12].to_string(),
        name: name.to_string(),
        image: image.to_string(),
        pid,
        // 停掉的容器 PID 是 0，没有根目录可逛
        root: (pid != 0).then(|| format!("/proc/{pid}/root")),
        status: status.to_string(),
        exit_code,
        started_at: started_at.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inspect_lines() {
        let line = "4f1c2d3e4b5a6978|/web-nginx|nginx:1.27|2481|running|0|2026-10-20T08:00:00Z";
        let container = parse_container(line).unwrap();
        assert_eq!(container.id, "4f1c2d3e4b5a");
        assert_eq!(container.name, "web-nginx");
        assert_eq!(container.root.as_deref(), Some("/proc/2481/root"));
    }

    #[test]
    fn keeps_stopped_containers_without_a_root() {
        let line = "4f1c2d3e4b5a6978|/old|nginx|0|exited|137|2026-10-20T08:00:00Z";
        let container = parse_container(line).unwrap();
        assert!(container.root.is_none());
        assert_eq!(container.exit_code, 137);
    }

    #[test]
    fn skips_noise() {
        assert!(parse_container("@@xin uid 0").is_none());
        assert!(parse_container("Welcome to the box").is_none());
        assert!(parse_container("not-hex-id-0000|/x|nginx|1|running|0|2026-10-20T08:00:00Z").is_none());
    }

    #[test]
    fn recognizes_docker_log_errors() {
        let outcome = CommandOutcome {
            exit_status: Some(1),
            tail: "Error response from daemon: No such container: 4f1c2d3e4b5a\n".to_string(),
        };
        assert!(logs_failure(&outcome).is(codes::DOCKER_CONTAINER_NOT_FOUND));

        let outcome = CommandOutcome {
            exit_status: Some(1),
            tail: "Error response from daemon: configured logging driver does not support reading\n".to_string(),
        };
        assert!(logs_failure(&outcome).is(codes::DOCKER_LOGS_UNSUPPORTED));
    }
}
