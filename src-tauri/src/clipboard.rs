//! 读本机剪贴板：文件面板里按 Ctrl+V 时来取，决定是上传复制的文件，还是把截图、文字存成远程文件。

use serde::Serialize;
use tauri::ipc::Response;

use crate::error::{codes, io_error, AppError};
use crate::util::path_to_string;

// 和内置编辑器同一个上限：更大的文字不像是要存成一个配置文件
const MAX_TEXT_BYTES: usize = 5 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardContent {
    pub files: Vec<String>,
    pub text: Option<String>,
    // 图片本身走 read_clipboard_image 按原始字节取，几兆的像素不塞进 JSON
    pub has_image: bool,
}

#[tauri::command]
pub async fn read_clipboard() -> Result<ClipboardContent, AppError> {
    tauri::async_runtime::spawn_blocking(read_clipboard_blocking)
        .await
        .map_err(io_error)?
}

/// 返回「宽、高（各 4 字节小端）+ RGBA 像素」；前端用 canvas 编码成 PNG 再预览、上传。
#[tauri::command]
pub async fn read_clipboard_image() -> Result<Response, AppError> {
    tauri::async_runtime::spawn_blocking(|| {
        let image = open_clipboard()?.get_image().map_err(clipboard_error)?;
        let width = u32::try_from(image.width).map_err(io_error)?;
        let height = u32::try_from(image.height).map_err(io_error)?;
        let mut bytes = Vec::with_capacity(8 + image.bytes.len());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&image.bytes);
        Ok(Response::new(bytes))
    })
    .await
    .map_err(io_error)?
}

fn read_clipboard_blocking() -> Result<ClipboardContent, AppError> {
    let mut clipboard = open_clipboard()?;
    // 资源管理器里复制的文件优先：这时剪贴板上的文字、缩略图只是附带的
    let files = clipboard.get().file_list().unwrap_or_default();
    if !files.is_empty() {
        return Ok(ClipboardContent {
            files: files.iter().map(|path| path_to_string(path)).collect(),
            text: None,
            has_image: false,
        });
    }

    let text = clipboard.get_text().ok().filter(|text| !text.is_empty());
    if text.as_ref().is_some_and(|text| text.len() > MAX_TEXT_BYTES) {
        return Err(AppError::new(codes::CLIPBOARD_TOO_LARGE, "Clipboard text is too large")
            .with("limitMb", MAX_TEXT_BYTES / (1024 * 1024)));
    }
    // 表格软件复制单元格会同时放文字和一张渲染图，两样都报给前端，由用户挑
    let has_image = clipboard.get_image().is_ok();

    Ok(ClipboardContent { files: Vec::new(), text, has_image })
}

fn open_clipboard() -> Result<arboard::Clipboard, AppError> {
    arboard::Clipboard::new().map_err(clipboard_error)
}

// 别的程序正占着剪贴板时打不开，稍后再按一次就好
fn clipboard_error(error: arboard::Error) -> AppError {
    AppError::new(codes::CLIPBOARD_UNAVAILABLE, error.to_string()).with("detail", error)
}
