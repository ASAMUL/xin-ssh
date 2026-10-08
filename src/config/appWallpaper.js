import darkWallpaper from "../assets/wallpapers/dark.png";
import lightWallpaper from "../assets/wallpapers/light.png";

// 应用壁纸：浅色 / 深色各一张，跟着应用的实际明暗切换。
// 某一档是 null 就用内置图，所以窗口底下永远有图——液态玻璃靠折射底下的东西才显得出来，
// 压在一片空白上就只剩一块灰。内置图随安装包走、不复制进工作区。
export const WALLPAPER_MODES = ["light", "dark"];

export const BUILT_IN_WALLPAPERS = {
  light: lightWallpaper,
  dark: darkWallpaper,
};

// 想要安静一点的背景就把可见度往下调，而不是把壁纸整个拿掉，所以下限不是 0
export const MIN_WALLPAPER_OPACITY = 0.1;
export const MAX_WALLPAPER_BLUR = 30;

// settings.json 可能是旧版的单图形状（Rust 读盘时已折算）或被手改坏；Rust 端 blur 是 u16，小数不能写回去
export function normalizeAppWallpaper(value) {
  return {
    light: normalizeFile(value?.light),
    dark: normalizeFile(value?.dark),
    opacity: clamp(Number(value?.opacity), MIN_WALLPAPER_OPACITY, 1, 1),
    blur: Math.round(clamp(Number(value?.blur), 0, MAX_WALLPAPER_BLUR, 0)),
  };
}

function normalizeFile(file) {
  return typeof file === "string" && file ? file : null;
}

function clamp(value, min, max, fallback) {
  if (!Number.isFinite(value)) return fallback;

  return Math.min(max, Math.max(min, value));
}
