use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub(crate) fn now_stamp() -> String {
    now_millis().to_string()
}

pub(crate) fn now_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    millis as u64
}

pub(crate) fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub(crate) fn connection_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn summarize_transfer_errors(errors: Vec<String>) -> String {
    let total = errors.len();
    let preview = errors
        .into_iter()
        .take(3)
        .collect::<Vec<_>>()
        .join("; ");

    if total > 3 {
        format!("{total} files failed. First errors: {preview}")
    } else {
        preview
    }
}
