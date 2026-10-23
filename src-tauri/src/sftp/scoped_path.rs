//! 在容器根目录里解析路径。经 /proc/<PID>/root 访问时，容器里的绝对软链接会被服务器按宿主机的根解析：
//! 以为在看「容器里的 /etc/localtime」，读到的其实是宿主机的文件。这里一段段 lstat + readlink，
//! 把链接目标重新接到容器根下面（和 Docker 自己 FollowSymlinkInScope 的做法一样），
//! 得到一条不含软链接的路径，之后交给服务器的都是它。

use std::collections::VecDeque;

use russh_sftp::client::SftpSession;
use tauri::State;
use tokio::time::{timeout, Duration};

use crate::error::{codes, required, timeout as timeout_error, AppError};
use crate::models::RemoteFileEntry;
use crate::sftp::error::sftp_error;
use crate::sftp::path::{normalize_remote_path, process_root_prefix};
use crate::sftp::remote_file_entry_from_attrs;
use crate::ssh::registry::TerminalRegistry;

// 和 Linux 内核一样，最多跟 40 层链接，再多就当成链接成环
const MAX_SYMLINK_HOPS: usize = 40;

/// root 是 /proc/<PID>/root，path 是容器里的绝对路径；返回解析后那一项在宿主机上的完整路径和属性。
#[tauri::command]
pub async fn resolve_container_path(
    registry: State<'_, TerminalRegistry>,
    session_id: String,
    root: String,
    path: String,
) -> Result<RemoteFileEntry, AppError> {
    if session_id.trim().is_empty() {
        return Err(required("sessionId"));
    }
    let root = normalize_remote_path(&root);
    if process_root_prefix(&root) != Some(root.as_str()) {
        return Err(AppError::new(codes::CONTAINER_ROOT_INVALID, "Not a container root").with("path", &root));
    }

    timeout(Duration::from_secs(12), registry.resolve_scoped_path(&session_id, root, path))
        .await
        .map_err(|_| timeout_error("resolvePath", 12))?
}

pub(crate) async fn resolve_scoped_path_on_session(
    sftp: &SftpSession,
    root: &str,
    path: &str,
) -> Result<RemoteFileEntry, AppError> {
    let mut pending: VecDeque<String> = path.split('/').map(str::to_string).collect();
    let mut resolved: Vec<String> = Vec::new();
    let mut hops = 0;
    let mut last_metadata = None;

    while let Some(component) = pending.pop_front() {
        match component.as_str() {
            "" | "." => continue,
            // 退到容器根就停住，不会退到宿主机的 /proc/<PID>
            ".." => {
                resolved.pop();
                last_metadata = None;
                continue;
            }
            _ => {}
        }

        let candidate = scoped_join(root, &resolved, Some(&component));
        let metadata = sftp
            .symlink_metadata(candidate.clone())
            .await
            .map_err(|error| sftp_error(error, &candidate))?;
        if !metadata.file_type().is_symlink() {
            resolved.push(component);
            last_metadata = Some(metadata);
            continue;
        }

        hops += 1;
        if hops > MAX_SYMLINK_HOPS {
            return Err(AppError::new(codes::REMOTE_SYMLINK_LOOP, "Too many levels of symbolic links")
                .with("path", &candidate));
        }
        let target = sftp
            .read_link(candidate.clone())
            .await
            .map_err(|error| sftp_error(error, &candidate))?;
        // 绝对链接从容器根重新走；相对链接接在链接所在目录后面
        if target.starts_with('/') {
            resolved.clear();
        }
        for part in target.split('/').rev() {
            pending.push_front(part.to_string());
        }
        last_metadata = None;
    }

    let full_path = scoped_join(root, &resolved, None);
    // 最后一步是「..」或者就是容器根本身时还没有属性；此时路径里已没有软链接（根是内核的魔法链接，跟进去是安全的）
    let metadata = match last_metadata {
        Some(metadata) => metadata,
        None => sftp
            .metadata(full_path.clone())
            .await
            .map_err(|error| sftp_error(error, &full_path))?,
    };
    let name = resolved.last().cloned().unwrap_or_else(|| "/".to_string());

    Ok(remote_file_entry_from_attrs(full_path, name, &metadata))
}

fn scoped_join(root: &str, resolved: &[String], next: Option<&str>) -> String {
    let mut path = root.to_string();
    for part in resolved.iter().map(String::as_str).chain(next) {
        path.push('/');
        path.push_str(part);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_components_under_the_container_root() {
        let resolved = vec!["etc".to_string(), "nginx".to_string()];
        assert_eq!(scoped_join("/proc/7/root", &resolved, None), "/proc/7/root/etc/nginx");
        assert_eq!(scoped_join("/proc/7/root", &resolved, Some("conf.d")), "/proc/7/root/etc/nginx/conf.d");
        assert_eq!(scoped_join("/proc/7/root", &[], None), "/proc/7/root");
    }
}
