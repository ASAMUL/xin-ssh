//! 列出服务器上正在跑的 Docker 容器。浏览容器文件不用 docker cp：
//! 拿到容器主进程的 PID，SFTP 直接走 /proc/<PID>/root 就是容器的根目录。

use tauri::State;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::error::{codes, required, AppError};
use crate::models::{DockerContainer, DockerContainerList};
use crate::remote_exec::{run_command, sh_command, CommandOptions, CommandOutcome, SshSession, MARKER};
use crate::ssh::registry::TerminalRegistry;

const LIST_TIMEOUT_SECS: u64 = 20;

// 顺带报告当前 uid：/proc/<PID>/root 只有 root（或和容器进程同一用户）才进得去，界面要先提醒。
// 名字和镜像里不会出现 |，拿它当分隔符。ps 和 inspect 之间有容器退出时 inspect 会报错，其余的照样列出来
const LIST_SCRIPT: &str = r#"command -v docker >/dev/null 2>&1 || { echo "@@xin missing docker"; exit 127; }
echo "@@xin uid $(id -u)"
ids=$(docker ps -q) || exit 1
[ -n "$ids" ] || exit 0
docker inspect --format '{{.Id}}|{{.Name}}|{{.Config.Image}}|{{.State.Pid}}|{{.State.StartedAt}}' $ids || true"#;

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
        return Err(docker_failure(&outcome));
    }
    containers.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(DockerContainerList { containers, is_root })
}

// docker 命令自己的报错只有英文文案，挑两种最常见的给出能照着做的提示
fn docker_failure(outcome: &CommandOutcome) -> AppError {
    let tail = outcome.tail.to_ascii_lowercase();
    if outcome.missing_tool().is_none() && tail.contains("permission denied") {
        return AppError::new(codes::DOCKER_PERMISSION_DENIED, "Permission denied while talking to Docker");
    }
    if tail.contains("cannot connect to the docker daemon") {
        return AppError::new(codes::DOCKER_DAEMON_UNAVAILABLE, "Docker daemon is not running");
    }
    outcome.failure("listContainers")
}

fn parse_container(line: &str) -> Option<DockerContainer> {
    let mut fields = line.trim().split('|');
    let id = fields.next()?;
    let name = fields.next()?.trim_start_matches('/');
    let image = fields.next()?;
    let pid = fields.next()?.parse::<u32>().ok()?;
    let started_at = fields.next()?;
    // 刚停掉的容器 PID 是 0，没有根目录可逛
    if id.len() < 12 || name.is_empty() || pid == 0 || fields.next().is_some() {
        return None;
    }

    Some(DockerContainer {
        id: id[..12].to_string(),
        name: name.to_string(),
        image: image.to_string(),
        pid,
        root: format!("/proc/{pid}/root"),
        started_at: started_at.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inspect_lines() {
        let line = "4f1c2d3e4b5a6978|/web-nginx|nginx:1.27|2481|2026-10-20T08:00:00Z";
        let container = parse_container(line).unwrap();
        assert_eq!(container.id, "4f1c2d3e4b5a");
        assert_eq!(container.name, "web-nginx");
        assert_eq!(container.root, "/proc/2481/root");
    }

    #[test]
    fn skips_stopped_containers_and_noise() {
        assert!(parse_container("4f1c2d3e4b5a6978|/old|nginx|0|2026-10-20T08:00:00Z").is_none());
        assert!(parse_container("@@xin uid 0").is_none());
        assert!(parse_container("Welcome to the box").is_none());
    }
}
