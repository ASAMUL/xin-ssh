//! 目录占用分析：在服务器上跑 du，只统计当前目录的直接子项，边算边推给前端。

use tauri::{AppHandle, Emitter, State};
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::error::{codes, required, AppError};
use crate::models::{DiskUsageEntry, DiskUsageEventPayload, DiskUsageRequest};
use crate::remote_exec::{command_canceled, run_command, sh_command, CommandOptions, CommandTask, SshSession};
use crate::sftp::path::normalize_remote_path;
use crate::ssh::registry::TerminalRegistry;

const DISK_USAGE_EVENT: &str = "disk-usage-event";
// 子目录一个个算完就往前端推，但不必每行一次事件
const EMIT_INTERVAL_MS: u64 = 150;

// -x 不跨文件系统：从 / 开始分析时不会钻进 /proc 和挂载的网盘；-a 把当前层的文件也算上。
// -H 只跟随命令行上给的那个路径：容器根 /proc/<PID>/root 本身是个链接，不跟进去就只量到链接自己。
// 老版 GNU du（CentOS 6）不认 -d，只认 --max-depth
const DISK_USAGE_SCRIPT: &str = r#"if du -d 0 /dev/null >/dev/null 2>&1; then
  exec du -k -x -a -H -d 1 -- "$1"
fi
exec du -k -x -a -H --max-depth=1 -- "$1""#;

#[tauri::command]
pub async fn start_disk_usage(
    registry: State<'_, TerminalRegistry>,
    request: DiskUsageRequest,
) -> Result<(), AppError> {
    if request.task_id.trim().is_empty() {
        return Err(required("taskId"));
    }
    if request.session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    if request.path.trim().is_empty() {
        return Err(required("remotePath"));
    }

    let request = DiskUsageRequest {
        path: normalize_remote_path(&request.path),
        ..request
    };
    registry
        .start_command_task(CommandTask::DiskUsage(request))
        .await
}

pub(crate) async fn run_disk_usage(
    app: AppHandle,
    session: SshSession,
    request: DiskUsageRequest,
    cancellation: CancellationToken,
) {
    let mut report = DiskUsageReport::new(request);
    let result = if cancellation.is_cancelled() {
        Err(command_canceled())
    } else {
        let command = sh_command(DISK_USAGE_SCRIPT, &[report.event.path.as_str()]);
        // 大目录要算很久，PTY 保证取消时 du 跟着退出
        let options = CommandOptions { pty: true, timeout: None };
        run_command(&session, command, options, &cancellation, |line| report.accept(&app, line)).await
    };

    let kind = match result {
        // du 碰到读不了的子目录会以 1 退出，但只要算出了总数，结果就可用（读不了的部分单独计数）
        Ok(_) if report.event.total_bytes.is_some() => "completed",
        Ok(outcome) => {
            report.event.error = Some(outcome.failure("diskUsage"));
            "failed"
        }
        Err(error) if error.is(codes::FILE_OPERATION_CANCELED) => "canceled",
        Err(error) => {
            report.event.error = Some(error);
            "failed"
        }
    };
    report.emit(&app, kind);
}

struct DiskUsageReport {
    event: DiskUsageEventPayload,
    last_emit: Instant,
}

impl DiskUsageReport {
    fn new(request: DiskUsageRequest) -> Self {
        Self {
            event: DiskUsageEventPayload {
                task_id: request.task_id,
                session_id: request.session_id,
                path: request.path,
                kind: "progress".to_string(),
                entries: Vec::new(),
                total_bytes: None,
                unreadable: 0,
                error: None,
            },
            last_emit: Instant::now(),
        }
    }

    fn accept(&mut self, app: &AppHandle, line: &str) {
        match parse_du_line(line, &self.event.path) {
            Some(DuLine::Child(entry)) => self.event.entries.push(entry),
            Some(DuLine::Total(bytes)) => self.event.total_bytes = Some(bytes),
            // 各家 du 的报错都以「du:」开头，多半是没权限读的子目录
            None if line.starts_with("du:") => self.event.unreadable += 1,
            None => {}
        }

        if !self.event.entries.is_empty()
            && self.last_emit.elapsed() >= Duration::from_millis(EMIT_INTERVAL_MS)
        {
            self.emit(app, "progress");
        }
    }

    fn emit(&mut self, app: &AppHandle, kind: &str) {
        self.event.kind = kind.to_string();
        let _ = app.emit(DISK_USAGE_EVENT, self.event.clone());
        self.event.entries.clear();
        self.last_emit = Instant::now();
    }
}

#[derive(Debug, PartialEq)]
enum DuLine {
    Child(DiskUsageEntry),
    Total(u64),
}

// du 的输出是「KiB<Tab>路径」；路径就是传进去的目录本身时是总数
fn parse_du_line(line: &str, root: &str) -> Option<DuLine> {
    let (size, path) = line.split_once('\t')?;
    let bytes = size.trim().parse::<u64>().ok()?.saturating_mul(1024);
    if path == root {
        return Some(DuLine::Total(bytes));
    }

    let prefix = if root == "/" { "/".to_string() } else { format!("{root}/") };
    let name = path.strip_prefix(&prefix)?;
    if name.is_empty() || name.contains('/') {
        return None;
    }

    Some(DuLine::Child(DiskUsageEntry {
        name: name.to_string(),
        bytes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child(name: &str, bytes: u64) -> Option<DuLine> {
        Some(DuLine::Child(DiskUsageEntry { name: name.to_string(), bytes }))
    }

    #[test]
    fn parses_children_and_total() {
        assert_eq!(parse_du_line("12\t/var/log", "/var"), child("log", 12 * 1024));
        assert_eq!(parse_du_line("40\t/var", "/var"), Some(DuLine::Total(40 * 1024)));
    }

    #[test]
    fn handles_the_root_directory() {
        assert_eq!(parse_du_line("8\t/etc", "/"), child("etc", 8 * 1024));
        assert_eq!(parse_du_line("99\t/", "/"), Some(DuLine::Total(99 * 1024)));
    }

    #[test]
    fn keeps_names_with_spaces_and_tabs() {
        assert_eq!(parse_du_line("4\t/srv/my files", "/srv"), child("my files", 4096));
        assert_eq!(parse_du_line("4\t/srv/a\tb", "/srv"), child("a\tb", 4096));
    }

    #[test]
    fn ignores_errors_and_noise() {
        assert_eq!(parse_du_line("du: cannot read directory '/root': Permission denied", "/"), None);
        assert_eq!(parse_du_line("Welcome to the box", "/"), None);
        assert_eq!(parse_du_line("4\t/elsewhere/x", "/srv"), None);
    }
}
