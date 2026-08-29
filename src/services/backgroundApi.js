import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export const TERMINAL_BACKGROUND_FITS = ["cover", "contain", "tile"];
export const TERMINAL_BACKGROUND_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];

const MIME_BY_EXTENSION = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  webp: "image/webp",
  gif: "image/gif",
  bmp: "image/bmp",
};

// file -> object URL。图片只在工作区里被替换，同一个文件名的内容不会变
// （导入时带时间戳前缀），所以缓存可以一直留到进程结束。
const objectUrlCache = new Map();

export function pickBackgroundImage(title) {
  return open({
    title,
    multiple: false,
    directory: false,
    filters: [{ name: "Image", extensions: TERMINAL_BACKGROUND_EXTENSIONS }],
  });
}

export function importTerminalBackground(sourcePath) {
  return invoke("import_terminal_background", { sourcePath });
}

// 拿到可直接塞进 CSS url() 的地址；同一文件重复调用复用同一个 Blob。
export async function resolveTerminalBackgroundUrl(file) {
  if (!file) return null;

  const cached = objectUrlCache.get(file);
  if (cached) return cached;

  const buffer = await invoke("read_terminal_background", { file });
  const extension = file.split(".").pop()?.toLowerCase() ?? "";
  const blob = new Blob([buffer], { type: MIME_BY_EXTENSION[extension] ?? "image/png" });
  const url = URL.createObjectURL(blob);
  objectUrlCache.set(file, url);

  return url;
}

export function normalizeTerminalBackground(background) {
  if (!background?.file) return null;

  return {
    file: background.file,
    opacity: clamp(Number(background.opacity), 0.04, 1, 0.35),
    blur: Math.round(clamp(Number(background.blur), 0, 30, 0)),
    fit: TERMINAL_BACKGROUND_FITS.includes(background.fit) ? background.fit : "cover",
  };
}

function clamp(value, min, max, fallback) {
  if (!Number.isFinite(value)) return fallback;

  return Math.min(max, Math.max(min, value));
}
