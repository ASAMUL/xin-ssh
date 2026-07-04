use std::path::Path;

use crate::util::path_to_string;

pub(crate) fn normalize_remote_path(path: &str) -> String {
    let trimmed = path.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return "/".to_string();
    }

    let mut parts = Vec::new();
    for part in trimmed.split('/') {
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

pub(crate) fn join_remote_path(base: &str, child: &str) -> String {
    let base = normalize_remote_path(base);
    let child: String = child
        .chars()
        .map(|character| if character == '/' || character == '\\' { '_' } else { character })
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

pub(crate) fn local_file_name(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("Could not determine file name: {}", path_to_string(path)))?
        .to_string_lossy()
        .trim()
        .to_string();

    if name.is_empty() {
        Err(format!("Could not determine file name: {}", path_to_string(path)))
    } else {
        Ok(name)
    }
}

pub(crate) fn safe_local_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            value if value.is_control() => '_',
            value => value,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "remote-item".to_string()
    } else {
        sanitized
    }
}
