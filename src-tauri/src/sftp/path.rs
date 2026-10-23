use std::path::Path;

use crate::error::{codes, AppError};
use crate::util::path_to_string;

pub(crate) fn normalize_remote_path(path: &str) -> String {
    // POSIX 文件名允许空格和反斜杠，改写它们会让删除、保存等操作命中另一个文件。
    if path.is_empty() {
        return "/".to_string();
    }

    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }

    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

/// /proc/<PID>/root 是这个进程（比如容器）看到的根目录。经它访问时，里面的绝对软链接按宿主机的根解析，
/// 服务器端的 realpath 也会把它还原成宿主机路径。所以这类路径下不做 realpath、不跟随软链接，
/// 软链接交给 scoped_path 在容器根里自己解析。返回 /proc/<PID>/root 这一段前缀。
pub(crate) fn process_root_prefix(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/proc/")?;
    let (pid, tail) = rest.split_once('/')?;
    if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if tail != "root" && !tail.starts_with("root/") {
        return None;
    }
    Some(&path[.."/proc/".len() + pid.len() + "/root".len()])
}

pub(crate) fn is_process_root_path(path: &str) -> bool {
    process_root_prefix(path).is_some()
}

pub(crate) fn join_remote_path(base: &str, child: &str) -> String {
    let base = normalize_remote_path(base);
    let child: String = child
        .chars()
        .map(|character| if character == '/' { '_' } else { character })
        .collect();

    if base == "/" {
        format!("/{child}")
    } else {
        format!("{base}/{child}")
    }
}

pub(crate) fn remote_basename(path: &str) -> String {
    normalize_remote_path(path)
        .split('/')
        .filter(|part| !part.is_empty())
        .next_back()
        .unwrap_or("remote-item")
        .to_string()
}

pub(crate) fn remote_parent_path(path: &str) -> String {
    let path = normalize_remote_path(path);
    let mut parts = path.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>();
    parts.pop();

    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

pub(crate) fn local_file_name(path: &Path) -> Result<String, AppError> {
    let name = path
        .file_name()
        .ok_or_else(|| file_name_unknown(path))?
        .to_str()
        .ok_or_else(|| file_name_unknown(path))?
        .to_string();

    if name.is_empty() {
        Err(file_name_unknown(path))
    } else {
        Ok(name)
    }
}

fn file_name_unknown(path: &Path) -> AppError {
    AppError::new(
        codes::LOCAL_FILE_NAME_UNKNOWN,
        format!("Could not determine file name: {}", path_to_string(path)),
    )
    .with("path", path_to_string(path))
}

/// 把远端文件名安全化成本地文件名。
///
/// 只处理**当前平台真正非法**的字符：Windows 的保留字符集 + 结尾的点/空格，
/// 其它平台只挡路径分隔符和控制字符。
/// 注意结尾的点才是 Windows 的限制，前导点是合法的 —— 之前用 `trim_matches('.')`
/// 会把 `.bashrc` 存成 `bashrc`、`.ssh` 存成 `ssh`，dotfile 全被改名。
pub(crate) fn safe_local_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|character| match character {
            '/' => '_',
            '\\' if cfg!(windows) => '_',
            '<' | '>' | ':' | '"' | '|' | '?' | '*' if cfg!(windows) => '_',
            value if value.is_control() => '_',
            value => value,
        })
        .collect();

    let sanitized = if cfg!(windows) {
        // Windows 会静默丢掉结尾的点和空格，先剪掉避免落地成不同的名字
        sanitized.trim_end_matches(['.', ' '])
    } else {
        sanitized.as_str()
    };

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "remote-item".to_string()
    } else if cfg!(windows) && is_windows_device_name(sanitized) {
        format!("_{sanitized}")
    } else {
        sanitized.to_string()
    }
}

fn is_windows_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end().to_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(*prefix).is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "\u{00b9}" | "\u{00b2}" | "\u{00b3}")
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_process_root_paths() {
        assert_eq!(process_root_prefix("/proc/123/root"), Some("/proc/123/root"));
        assert_eq!(process_root_prefix("/proc/123/root/etc/nginx"), Some("/proc/123/root"));
        assert_eq!(process_root_prefix("/proc/123/rootfs"), None);
        assert_eq!(process_root_prefix("/proc/self/root/etc"), None);
        assert_eq!(process_root_prefix("/proc/123/cwd"), None);
        assert_eq!(process_root_prefix("/var/proc/1/root"), None);
    }
}
