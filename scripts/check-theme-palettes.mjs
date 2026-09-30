// 主题调色板体检：ls / vim 之类的程序把语义压在固定的 ANSI 槽位上
//   blue=目录  green=可执行  cyan=符号链接  red=归档  magenta=图片  yellow=管道/设备
// 所以这几个槽位之间必须**肉眼可分**，否则主题再好看，`ls` 出来也是一坨。
// 这里算三件事：
//   1. 槽位两两之间的感知色差 ΔE（CIE76，粗筛用），撞色即报
//   2. 每个颜色对背景的 WCAG 对比度，太低判为看不清
//   3. brightBlack（自动补全提示、注释）对背景的对比度——要暗，但得读得出
//
// 用法：node scripts/check-theme-palettes.mjs

import { TERMINAL_THEMES } from "../src/config/terminalThemes.js";

const SEMANTIC_SLOTS = ["red", "green", "yellow", "blue", "magenta", "cyan"];
// blue=目录 green=可执行 cyan=符号链接 yellow=管道/设备 red=归档——这四五个槽位
// 在 `ls` 里天天并排出现，必须拉开；magenta（图片）没那么要紧，按宽档走。
const CRITICAL_SLOTS = ["red", "green", "yellow", "blue", "cyan"];
// 阈值取 26 的依据（用实测定的，不是拍脑袋）：Dracula 的关键对最低 51，
// 而公认好用的上游配色（Catppuccin 27.9 / Tokyo Night 29.5 / Synthwave '84 29.0）
// 落在 28~30。取 26 既放得过这些真配色，又能抓出明显撞色的。
// 注：CIE76 对高饱和色的色相差会低估，所以只当「抓离群值」用，别当精确度量。
const MIN_DELTA_E_CRITICAL = 26;
const MIN_DELTA_E = 12;
// 自家的 Xin 按正文标准（WCAG AA 4.5:1）要求；经典组按宽档只做提示
const MIN_CONTRAST = { xin: 4.5, classic: 3 };
const MIN_DIM_CONTRAST = 3;
// 经典组是上游原版配色（Vesper / Poimandres 的 green/cyan 本来就贴得很近），
// 「原汁原味」正是这一组存在的理由，撞色也不该由我们去改——只提示不算失败。
const PALETTE_EXEMPT_GROUPS = ["classic"];

function parseHex(value) {
  const hex = value.replace("#", "");
  return [0, 2, 4].map((offset) => parseInt(hex.slice(offset, offset + 2), 16));
}

function srgbToLinear(channel) {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function relativeLuminance(hex) {
  const [r, g, b] = parseHex(hex).map(srgbToLinear);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrastRatio(foreground, background) {
  const a = relativeLuminance(foreground);
  const b = relativeLuminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

function toLab(hex) {
  const [lr, lg, lb] = parseHex(hex).map(srgbToLinear);
  // sRGB -> XYZ (D65)
  const x = (0.4124 * lr + 0.3576 * lg + 0.1805 * lb) / 0.95047;
  const y = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
  const z = (0.0193 * lr + 0.1192 * lg + 0.9505 * lb) / 1.08883;
  const f = (t) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  return [116 * f(y) - 16, 500 * (f(x) - f(y)), 200 * (f(y) - f(z))];
}

function deltaE(left, right) {
  const a = toLab(left);
  const b = toLab(right);
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

let failures = 0;

const variants = TERMINAL_THEMES.flatMap((theme) =>
  Object.values(theme.variants).map((variant) => ({ ...variant, group: theme.group })),
);

for (const theme of variants) {
  // 通透档的面板底色是同一个颜色降透明度，按实底算即可
  const background = theme.xterm.background;
  const exempt = PALETTE_EXEMPT_GROUPS.includes(theme.group);
  const minContrast = MIN_CONTRAST[theme.group];
  const notes = [];

  for (let i = 0; i < SEMANTIC_SLOTS.length; i += 1) {
    for (let j = i + 1; j < SEMANTIC_SLOTS.length; j += 1) {
      const [left, right] = [SEMANTIC_SLOTS[i], SEMANTIC_SLOTS[j]];
      const critical = CRITICAL_SLOTS.includes(left) && CRITICAL_SLOTS.includes(right);
      const floor = critical ? MIN_DELTA_E_CRITICAL : MIN_DELTA_E;
      const distance = deltaE(theme.xterm[left], theme.xterm[right]);
      if (distance < floor) {
        notes.push(
          `撞色${critical ? "(ls 关键)" : ""} ${left}/${right} ΔE=${distance.toFixed(1)} < ${floor} (${theme.xterm[left]} vs ${theme.xterm[right]})`,
        );
      }
    }
  }

  for (const slot of [...SEMANTIC_SLOTS, "foreground"]) {
    const ratio = contrastRatio(theme.xterm[slot], background);
    if (ratio < minContrast) {
      notes.push(`低对比 ${slot} ${ratio.toFixed(2)}:1 < ${minContrast} (${theme.xterm[slot]})`);
    }
  }

  const dimRatio = contrastRatio(theme.xterm.brightBlack, background);
  if (dimRatio < MIN_DIM_CONTRAST) {
    notes.push(`提示色太暗 brightBlack ${dimRatio.toFixed(2)}:1 (${theme.xterm.brightBlack})`);
  }

  if (notes.length) {
    if (!exempt) failures += notes.length;
    console.log(
      `${exempt ? "…" : "✗"} ${theme.name ?? theme.id} (${theme.id})${exempt ? " —— 上游原版配色，仅提示" : ""}`,
    );
    notes.forEach((note) => console.log(`    ${note}`));
  } else {
    console.log(`✓ ${theme.name ?? theme.id}`);
  }
}

console.log(`\n${failures ? `${failures} 处需要修` : "全部通过"}`);
process.exitCode = failures ? 1 : 0;
