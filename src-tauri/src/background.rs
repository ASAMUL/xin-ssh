//! 终端背景图的落盘与读取。
//!
//! 设计取舍：图片**不**按用户选择的原始绝对路径引用，而是复制进
//! `<workspace>/backgrounds/`，settings.json / profiles.json 里只存文件名。
//! 原路径在换机器、清理下载目录、同步工作区之后都会失效，而工作区目录本身
//! 就是「可整体搬走」的单位，背景图跟着它走才不会断链。
//!
//! 图片字节走 `tauri::ipc::Response` 原样返回（前端转 Blob URL），不走 JSON
//! 序列化——几 MB 的图片走 JSON 数字数组会慢到不可接受；也因此不需要打开
//! asset protocol，省掉一层运行时 scope 授权。

use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{ipc::Response, AppHandle};

use crate::error::{codes, io_error, AppError};
use crate::util::now_millis;
use crate::workspace::active_workspace_dir;

const BACKGROUNDS_DIR: &str = "backgrounds";
const MAX_BACKGROUND_BYTES: u64 = 24 * 1024 * 1024;
const ALLOWED_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// 把用户选中的图片复制进工作区，返回落盘后的文件名。
#[tauri::command]
pub async fn import_terminal_background(
    app: AppHandle,
    source_path: String,
) -> Result<String, AppError> {
    let source = PathBuf::from(&source_path);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| ALLOWED_EXTENSIONS.contains(&value.as_str()))
        .ok_or_else(|| {
            AppError::new(
                codes::BACKGROUND_UNSUPPORTED_FORMAT,
                format!("Unsupported image format: {source_path}"),
            )
            .with("formats", ALLOWED_EXTENSIONS.join(" / "))
        })?;

    let metadata = fs::metadata(&source).map_err(io_error)?;
    if metadata.len() > MAX_BACKGROUND_BYTES {
        return Err(AppError::new(
            codes::BACKGROUND_TOO_LARGE,
            format!("Image is {} bytes", metadata.len()),
        )
        .with("limitMb", MAX_BACKGROUND_BYTES / 1024 / 1024));
    }

    let directory = backgrounds_dir(&app)?;
    fs::create_dir_all(&directory).map_err(io_error)?;

    // 时间戳前缀：同名图片重复导入不会互相覆盖，旧连接引用的那张仍然有效
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_stem)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "background".to_string());
    let file = format!("{}-{stem}.{extension}", now_millis());

    fs::copy(&source, directory.join(&file)).map_err(io_error)?;

    Ok(file)
}

/// 读取背景图原始字节，交给前端转 Blob URL。
#[tauri::command]
pub async fn read_terminal_background(app: AppHandle, file: String) -> Result<Response, AppError> {
    let path = backgrounds_dir(&app)?.join(safe_file_name(&file)?);
    if !path.is_file() {
        return Err(AppError::new(codes::BACKGROUND_MISSING, format!("Missing {file}"))
            .with("file", file));
    }

    let bytes = fs::read(&path).map_err(io_error)?;

    Ok(Response::new(bytes))
}

fn backgrounds_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(active_workspace_dir(app)?.join(BACKGROUNDS_DIR))
}

/// settings.json / profiles.json 是用户可手改的文本文件，文件名必须当成不可信输入：
/// 只接受单段纯文件名，任何目录分隔或 `..` 都会被挡在 backgrounds/ 之外。
fn safe_file_name(file: &str) -> Result<String, AppError> {
    let candidate = Path::new(file)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if candidate.is_empty() || candidate != file {
        return Err(
            AppError::new(codes::BACKGROUND_MISSING, format!("Invalid file name: {file}"))
                .with("file", file),
        );
    }

    Ok(candidate.to_string())
}

fn sanitize_stem(stem: &str) -> String {
    stem.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(48)
        .collect()
}
