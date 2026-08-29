// 应用明暗模式。三档：system / light / dark。
//
// 落地方式是给 <html> 挂 .light / .dark 类，一个类名同时管三件事：
//   1. 本应用的设计令牌——main.css 的 .dark 块整体翻面；
//   2. 全部 Nuxt UI 控件——v4 不用 dark: 变体，而是靠 .dark 重定义 --ui-*
//      语义变量，所以我们不必为任何 Nuxt UI 组件写 CSS；
//   3. color-scheme——让原生滚动条、下拉箭头这类 UA 自绘控件跟着翻，
//      漏掉它的话深色下会冒出白色滚动条。
//
// Tauri 里还必须把**窗口主题**一起设掉：窗口开了 mica/acrylic，而 Tauri v2 的
// Effect 枚举没有 micaDark / micaLight，mica 只认系统明暗。只切 CSS 的话，
// 用户手动选深色而系统是浅色时，窗口边缘的背板仍是亮的，和内容对不上。

import { getCurrentWindow } from "@tauri-apps/api/window";

export const APPEARANCE_MODES = ["system", "light", "dark"];

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
let currentMode = "system";

export function normalizeAppearanceMode(mode) {
  return APPEARANCE_MODES.includes(mode) ? mode : "system";
}

function resolveMode(mode) {
  if (mode === "light" || mode === "dark") return mode;
  return darkQuery.matches ? "dark" : "light";
}

function paint(resolved) {
  const root = document.documentElement;
  root.classList.toggle("dark", resolved === "dark");
  root.classList.toggle("light", resolved === "light");
  root.style.colorScheme = resolved;

  if (!window.__TAURI_INTERNALS__) return;

  // 跟随系统时传 null，把决定权交回给系统，用户改系统设置时窗口会自己跟上
  getCurrentWindow()
    .setTheme(currentMode === "system" ? null : resolved)
    // 权限缺失或平台不支持时不该拖垮界面：CSS 那部分已经生效了
    .catch(() => {});
}

export function applyAppearanceMode(mode) {
  currentMode = normalizeAppearanceMode(mode);
  paint(resolveMode(currentMode));
  return currentMode;
}

export function resolvedAppearance() {
  return resolveMode(currentMode);
}

// 只在「跟随系统」档下响应系统切换；用户显式选了浅/深就不该被系统覆盖
darkQuery.addEventListener("change", () => {
  if (currentMode === "system") paint(resolveMode(currentMode));
});
