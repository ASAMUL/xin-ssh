// 液态玻璃强度，对应 Apple iOS 26.1 起的「通透 / 着色」开关：
//   clear  —— 通透：玻璃白度低、窗口底更透，Tauri 里能看到更多桌面壁纸（Mica）；
//   tinted —— 着色：玻璃更实、对比更高，背景花的时候读字更轻松。
// 落地是给 <html> 挂 data-glass，具体数值全在 liquid-glass.css 的「玻璃强度」一节，JS 不碰任何颜色。

export const GLASS_STYLES = ["clear", "tinted"];

export function normalizeGlassStyle(style) {
  return GLASS_STYLES.includes(style) ? style : "clear";
}

export function applyGlassStyle(style) {
  const normalized = normalizeGlassStyle(style);
  document.documentElement.dataset.glass = normalized;
  return normalized;
}
