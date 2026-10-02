// 液态玻璃强度：0~100 一条滑块（iOS 的做法），两端对应 iOS 26.1 起的「通透 / 着色」：
//   0   —— 通透：玻璃白度低、窗口底更透，Tauri 里能看到更多桌面壁纸（Mica）；
//   100 —— 着色：玻璃更实、对比更高，背景花的时候读字更轻松。
// JS 只往 <html> 写一个 0~1 的数 --xin-glass-tint；每个令牌在两端之间怎么插值，
// 写在令牌自己定义的地方（main.css / liquid-glass.css），JS 不碰任何颜色。

export const DEFAULT_GLASS_TINT = 0;

// settings.json 里的值可能被手改坏；Rust 端是 u8，越界或带小数的值绝不能写回去
export function normalizeGlassTint(value) {
  const tint = Number(value);
  if (!Number.isFinite(tint)) return DEFAULT_GLASS_TINT;
  return Math.round(Math.min(100, Math.max(0, tint)));
}

export function applyGlassTint(value) {
  const tint = normalizeGlassTint(value);
  document.documentElement.style.setProperty("--xin-glass-tint", String(tint / 100));
  return tint;
}
