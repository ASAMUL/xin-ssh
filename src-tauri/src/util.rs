use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{codes, AppError};

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

pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// 批量操作里多个条目分别失败时，合并成一条带条目数与前几条详情的错误。
pub(crate) fn summarize_entry_errors(errors: Vec<(String, AppError)>) -> AppError {
    let total = errors.len();
    let mut remaining = errors.into_iter();

    // 只有一个条目失败时保留原始错误码，前端仍能按码精确处理（比如弹冲突对话框）
    if total == 1 {
        if let Some((label, error)) = remaining.next() {
            return error.with("label", label);
        }
    }

    let preview = remaining
        .take(3)
        .map(|(label, error)| format!("{label}: {}", error.message))
        .collect::<Vec<_>>()
        .join("; ");

    AppError::new(
        codes::TRANSFER_PARTIAL_FAILURE,
        format!("{total} items failed. First errors: {preview}"),
    )
    .with("count", total)
    .with("detail", preview)
}
