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

// 终端输出、容器日志都按字节流到达，多字节 UTF-8 字符可能被拆在两个 chunk 之间：
// 返回结尾处不完整序列的字节数，这部分留到下一个 chunk 一起发出。
pub(crate) fn incomplete_utf8_suffix_len(bytes: &[u8]) -> usize {
    let len = bytes.len();
    let start = len.saturating_sub(3);

    for index in (start..len).rev() {
        let byte = bytes[index];
        if byte & 0b1100_0000 == 0b1000_0000 {
            continue;
        }

        let expected = if byte >= 0xF0 {
            4
        } else if byte >= 0xE0 {
            3
        } else if byte >= 0xC0 {
            2
        } else {
            return 0;
        };

        let available = len - index;
        return if available < expected { available } else { 0 };
    }

    0
}
