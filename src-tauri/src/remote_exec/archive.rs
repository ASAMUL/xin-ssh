//! 右键压缩 / 解压：直接在服务器上跑 tar、zip、unzip，
//! 省掉「下载到本地 → 解压 → 几万个小文件再一个个传上去」。

use tauri::{AppHandle, Emitter, State};
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::error::{codes, required, root_forbidden, AppError};
use crate::models::{FileOperationEventPayload, RemoteCompressRequest, RemoteExtractRequest};
use crate::remote_exec::{
    command_canceled, run_command, sh_command, CommandOptions, CommandTask, SshSession, MARKER,
};
use crate::sftp::path::{join_remote_path, normalize_remote_path, remote_basename, remote_parent_path};
use crate::ssh::registry::TerminalRegistry;

const FILE_OPERATION_EVENT: &str = "file-operation-event";
const EMIT_INTERVAL_MS: u64 = 120;

// 参数：目录、格式、压缩包名（不带扩展名）、要压缩的名字……
// 同名压缩包已存在就依次试 -2、-3，绝不覆盖；取消或失败时删掉没做完的压缩包。
// zip 不一定装了，退一步用 python3 自带的 zipfile；两个都没有就报缺 zip
const COMPRESS_SCRIPT: &str = r#"cd -- "$1" || exit 3
format="$2"; stem="$3"; shift 3
tool=
if [ "$format" = zip ]; then
  ext=.zip
  if command -v zip >/dev/null 2>&1; then tool=zip
  elif command -v python3 >/dev/null 2>&1; then tool=python3
  else echo "@@xin missing zip"; exit 127; fi
else
  ext=.tar.gz
fi
out="$stem$ext"; n=1
while [ -e "$out" ] || [ -L "$out" ]; do n=$((n + 1)); out="$stem-$n$ext"; done
echo "@@xin output $out"
trap 'rm -f -- "$out"; exit 130' HUP INT TERM
case "$tool" in
  zip) zip -r -y "$out" "$@" ;;
  python3) python3 -m zipfile -c "$out" "$@" ;;
  *) tar -czvf "$out" "$@" ;;
esac
status=$?
[ "$status" -eq 0 ] || rm -f -- "$out"
exit "$status""#;

// 参数：目录、压缩包名、类型、folder|here、输出名主干、输出名扩展名。
// folder：解到新建的同名目录（已存在就 -2、-3），取消时整个删掉；
// here：解到当前目录，已有的文件一律跳过不覆盖（GNU tar 用 --skip-old-files，unzip 用 -n）。
// 单个 .gz 不是归档，解出来就是一个文件，同样起一个不重名的名字。
// 解压失败时保留已解出的内容，交给用户判断；只有取消才清理。
const EXTRACT_SCRIPT: &str = r#"cd -- "$1" || exit 3
archive="$2"; kind="$3"; mode="$4"; stem="$5"; ext="$6"
tool=
case "$kind" in
  zip)
    if command -v unzip >/dev/null 2>&1; then tool=unzip
    elif command -v python3 >/dev/null 2>&1; then tool=python3
    else echo "@@xin missing unzip"; exit 127; fi ;;
  gz)
    command -v gzip >/dev/null 2>&1 || { echo "@@xin missing gzip"; exit 127; } ;;
esac
out=.
if [ "$kind" = gz ] || [ "$mode" = folder ]; then
  out="$stem$ext"; n=1
  while [ -e "$out" ] || [ -L "$out" ]; do n=$((n + 1)); out="$stem-$n$ext"; done
fi
if [ "$kind" = gz ]; then
  # 先独占地建出空文件：失败时删的只能是自己建的那个，不能误删刚好同名冒出来的别人的文件
  ( set -C; : > "$out" ) || exit 5
  echo "@@xin output $out"
  trap 'rm -f -- "$out"; exit 130' HUP INT TERM
  gzip -dc "$archive" > "$out"
  status=$?
  [ "$status" -eq 0 ] || rm -f -- "$out"
  exit "$status"
fi
# python3 的 zipfile 会直接覆盖同名文件，解到当前目录时不能拿它兜底
if [ "$tool" = python3 ] && [ "$out" = . ]; then echo "@@xin missing unzip"; exit 127; fi
if [ "$out" != . ]; then
  mkdir -- "$out" || exit 5
  echo "@@xin output $out"
  trap 'rm -rf -- "$out"; exit 130' HUP INT TERM
fi
case "$kind" in
  zip)
    if [ "$tool" = unzip ]; then
      # 加密的包会弹出密码提示、把任务一直卡着：给个假密码，加密的条目直接报错跳过
      unzip -n -P xin-no-password "$archive" -d "$out"
    else
      python3 -m zipfile -e "$archive" "$out"
    fi ;;
  *)
    case "$kind" in tgz) flag=z ;; tbz) flag=j ;; txz) flag=J ;; *) flag= ;; esac
    opts=
    if tar --version 2>/dev/null | grep -q GNU; then
      opts=--no-same-owner
      if [ "$out" = . ]; then
        if tar --help 2>/dev/null | grep -q skip-old-files; then opts="$opts --skip-old-files"; else opts="$opts -k"; fi
      fi
    elif [ "$out" = . ]; then
      opts=-k
    fi
    tar -x${flag}v -f "$archive" -C "$out" $opts ;;
esac"#;

#[tauri::command]
pub async fn start_remote_compress(
    registry: State<'_, TerminalRegistry>,
    request: RemoteCompressRequest,
) -> Result<(), AppError> {
    validate_task_ids(&request.task_id, &request.session_id)?;
    if request.names.is_empty() {
        return Err(AppError::new(
            codes::VALIDATION_SELECT_REMOTE_ITEM,
            "Choose at least one remote item",
        ));
    }
    for name in request.names.iter().chain([&request.archive_name]) {
        validate_entry_name(name)?;
    }
    let format = match request.format.as_str() {
        "zip" => "zip",
        "tar.gz" => "tar.gz",
        other => {
            return Err(AppError::new(codes::ARCHIVE_UNSUPPORTED, "Unsupported archive format")
                .with("name", other))
        }
    };

    let directory = normalize_remote_path(&request.directory);
    let mut args = vec![directory.clone(), format.to_string(), request.archive_name.clone()];
    args.extend(request.names.iter().map(|name| argument_safe_name(name)));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    registry
        .start_command_task(CommandTask::Archive(ArchiveTask {
            task_id: request.task_id,
            session_id: request.session_id,
            directory,
            command: sh_command(COMPRESS_SCRIPT, &args),
            operation: "compress",
            // tar -v 每行一个成员名；zip 每个文件一行「adding: 名字」
            progress: if format == "zip" { ProgressStyle::Verbs } else { ProgressStyle::Lines },
        }))
        .await
}

#[tauri::command]
pub async fn start_remote_extract(
    registry: State<'_, TerminalRegistry>,
    request: RemoteExtractRequest,
) -> Result<(), AppError> {
    validate_task_ids(&request.task_id, &request.session_id)?;
    let archive_path = normalize_remote_path(&request.archive_path);
    if archive_path == "/" {
        return Err(root_forbidden("extract"));
    }

    let name = remote_basename(&archive_path);
    let Some(archive) = detect_archive(&name) else {
        return Err(AppError::new(codes::ARCHIVE_UNSUPPORTED, "Unsupported archive format")
            .with("name", &name));
    };
    let (stem, ext) = archive.output_name();
    let directory = remote_parent_path(&archive_path);
    let mode = if request.into_folder { "folder" } else { "here" };
    let archive_argument = argument_safe_name(&name);
    let args = [
        directory.as_str(),
        archive_argument.as_str(),
        archive.kind,
        mode,
        stem.as_str(),
        ext.as_str(),
    ];

    registry
        .start_command_task(CommandTask::Archive(ArchiveTask {
            task_id: request.task_id,
            session_id: request.session_id,
            command: sh_command(EXTRACT_SCRIPT, &args),
            directory,
            operation: "extract",
            progress: if archive.kind == "zip" { ProgressStyle::Verbs } else { ProgressStyle::Lines },
        }))
        .await
}

pub(crate) struct ArchiveTask {
    pub task_id: String,
    pub session_id: String,
    directory: String,
    command: String,
    operation: &'static str,
    progress: ProgressStyle,
}

#[derive(Clone, Copy)]
enum ProgressStyle {
    // tar -v：每行就是一个成员名
    Lines,
    // zip / unzip：「adding: 名字」「inflating: 名字」，其它行是提示或告警
    Verbs,
}

pub(crate) async fn run_archive_task(
    app: AppHandle,
    session: SshSession,
    task: ArchiveTask,
    cancellation: CancellationToken,
) {
    let mut progress = ArchiveProgress::new(&task);
    let result = if cancellation.is_cancelled() {
        Err(command_canceled())
    } else {
        progress.emit(&app, "started", true);
        // 几万个文件要跑很久，PTY 保证取消时 tar / zip 跟着退出，脚本的 trap 再把半成品删掉
        let options = CommandOptions { pty: true, timeout: None };
        run_command(&session, task.command, options, &cancellation, |line| progress.accept(&app, line)).await
    };

    let kind = match result {
        Ok(outcome) if outcome.succeeded() => "completed",
        Ok(outcome) => {
            progress.event.error = Some(outcome.failure(task.operation));
            "failed"
        }
        Err(error) if error.is(codes::FILE_OPERATION_CANCELED) => {
            // 脚本已经把没做完的产物删了
            progress.event.result_path = None;
            "canceled"
        }
        Err(error) => {
            progress.event.error = Some(error);
            "failed"
        }
    };
    progress.emit(&app, kind, true);
}

struct ArchiveProgress {
    event: FileOperationEventPayload,
    directory: String,
    style: ProgressStyle,
    last_emit: Instant,
}

impl ArchiveProgress {
    fn new(task: &ArchiveTask) -> Self {
        Self {
            event: FileOperationEventPayload {
                task_id: task.task_id.clone(),
                session_id: task.session_id.clone(),
                kind: "started".to_string(),
                current_path: None,
                completed_entries: 0,
                error: None,
                result_path: None,
            },
            directory: task.directory.clone(),
            style: task.progress,
            last_emit: Instant::now(),
        }
    }

    fn accept(&mut self, app: &AppHandle, line: &str) {
        if let Some(output) = line.strip_prefix(MARKER).and_then(|rest| rest.strip_prefix("output ")) {
            self.event.result_path = Some(join_remote_path(&self.directory, output));
            return;
        }
        if let Some(member) = progress_member(line, self.style) {
            self.event.completed_entries += 1;
            self.event.current_path = Some(member.to_string());
            self.emit(app, "progress", false);
        }
    }

    fn emit(&mut self, app: &AppHandle, kind: &str, force: bool) {
        if !force && self.last_emit.elapsed() < Duration::from_millis(EMIT_INTERVAL_MS) {
            return;
        }
        self.event.kind = kind.to_string();
        let _ = app.emit(FILE_OPERATION_EVENT, self.event.clone());
        self.last_emit = Instant::now();
    }
}

// 进度行里的成员名；工具自己的提示、报错不算进度
fn progress_member(line: &str, style: ProgressStyle) -> Option<&str> {
    const VERBS: [&str; 5] = ["adding:", "inflating:", "extracting:", "creating:", "linking:"];
    const TOOL_PREFIXES: [&str; 8] = ["tar:", "gzip:", "bzip2:", "xz:", "sh:", "mkdir:", "warning", "error"];

    let trimmed = line.trim();
    if trimmed.is_empty() || line.starts_with(MARKER) {
        return None;
    }

    match style {
        ProgressStyle::Lines => {
            let is_message = TOOL_PREFIXES.iter().any(|prefix| trimmed.starts_with(prefix));
            (!is_message).then_some(trimmed)
        }
        ProgressStyle::Verbs => {
            let rest = VERBS.iter().find_map(|verb| trimmed.strip_prefix(verb))?.trim();
            // zip 在行尾补压缩率「名字 (deflated 63%)」，unzip 的链接行是「名字  -> 目标」
            let rest = rest
                .rsplit_once(" (")
                .filter(|(_, tail)| tail.ends_with("%)"))
                .map_or(rest, |(name, _)| name);
            Some(rest.split_once("  -> ").map_or(rest, |(name, _)| name))
        }
    }
}

struct DetectedArchive {
    kind: &'static str,
    stem: String,
}

impl DetectedArchive {
    // 解压产物的名字：归档解到同名目录；单个 .gz 解出来是去掉 .gz 的那个文件，
    // 重名时把序号插在扩展名前面（access-2.log），所以拆成主干和扩展名两段交给脚本
    fn output_name(&self) -> (String, String) {
        if self.kind != "gz" {
            return (self.stem.clone(), String::new());
        }
        match self.stem.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => (stem.to_string(), format!(".{ext}")),
            _ => (self.stem.clone(), String::new()),
        }
    }
}

// 认得的格式要和前端 utils/archive.js 保持一致：右键菜单据此决定显不显示「解压」
fn detect_archive(name: &str) -> Option<DetectedArchive> {
    const SUFFIXES: [(&str, &str); 10] = [
        (".tar.gz", "tgz"),
        (".tgz", "tgz"),
        (".tar.bz2", "tbz"),
        (".tbz2", "tbz"),
        (".tbz", "tbz"),
        (".tar.xz", "txz"),
        (".txz", "txz"),
        (".tar", "tar"),
        (".zip", "zip"),
        (".gz", "gz"),
    ];

    let lower = name.to_ascii_lowercase();
    SUFFIXES.iter().find_map(|&(suffix, kind)| {
        if lower.len() <= suffix.len() || !lower.ends_with(suffix) {
            return None;
        }
        // 后缀全是 ASCII，按字节截掉不会切坏前面的中文名
        Some(DetectedArchive {
            kind,
            stem: name[..name.len() - suffix.len()].to_string(),
        })
    })
}

// 以 - 开头的名字会被 tar / zip 当成选项，前面补 ./
fn argument_safe_name(name: &str) -> String {
    if name.starts_with('-') {
        format!("./{name}")
    } else {
        name.to_string()
    }
}

fn validate_task_ids(task_id: &str, session_id: &str) -> Result<(), AppError> {
    if task_id.trim().is_empty() {
        return Err(required("taskId"));
    }
    if session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    Ok(())
}

fn validate_entry_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        return Err(AppError::new(codes::ARCHIVE_INVALID_NAME, "Invalid file name").with("name", name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_archive_kinds_case_insensitively() {
        let archive = detect_archive("备份.TAR.GZ").unwrap();
        assert_eq!((archive.kind, archive.stem.as_str()), ("tgz", "备份"));
        assert_eq!(detect_archive("site.zip").unwrap().kind, "zip");
        assert_eq!(detect_archive("db.tar").unwrap().kind, "tar");
        assert!(detect_archive(".zip").is_none());
        assert!(detect_archive("notes.txt").is_none());
    }

    #[test]
    fn single_gzip_files_keep_their_inner_extension() {
        assert_eq!(
            detect_archive("access.log.gz").unwrap().output_name(),
            ("access".to_string(), ".log".to_string())
        );
        assert_eq!(
            detect_archive("logs.tar.gz").unwrap().output_name(),
            ("logs".to_string(), String::new())
        );
    }

    #[test]
    fn reads_members_from_tool_output() {
        assert_eq!(progress_member("  adding: app/main.js (deflated 63%)", ProgressStyle::Verbs), Some("app/main.js"));
        assert_eq!(progress_member("  inflating: dist/index.html", ProgressStyle::Verbs), Some("dist/index.html"));
        assert_eq!(progress_member("    linking: lib/a.so  -> a.so.1", ProgressStyle::Verbs), Some("lib/a.so"));
        assert_eq!(progress_member("Archive:  site.zip", ProgressStyle::Verbs), None);
        assert_eq!(progress_member("logs/app.log", ProgressStyle::Lines), Some("logs/app.log"));
        assert_eq!(progress_member("tar: Removing leading `/'", ProgressStyle::Lines), None);
        assert_eq!(progress_member("@@xin output logs.tar.gz", ProgressStyle::Lines), None);
    }

    #[test]
    fn dash_names_are_not_taken_as_options() {
        assert_eq!(argument_safe_name("-rf"), "./-rf");
        assert_eq!(argument_safe_name("logs"), "logs");
    }
}
