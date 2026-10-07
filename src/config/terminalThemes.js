// 终端外观 = 调色板 × 明暗 × 材质。
//
//   调色板（TERMINAL_THEMES 里的一项，叫「家族」）：一套 ANSI 配色，最多深浅两版。
//   明暗：auto 跟随应用明暗，只有一版的家族（Nord、Dracula…）不管选哪档都用那一版。
//   材质：solid 实底；clear 通透——底色降到八成不透明加模糊，应用壁纸从底下透上来。
//
// 2026-09-30 起不再用 CSS 画场景（极光、星野、透视网格、扫描线、辉光）。和 iOS 26 的
// 液态玻璃摆在一起，那些画出来的光效显得廉价；终端的氛围交给用户自己的壁纸，
// 主题只负责把字配好色。面板描边、投影、胶囊材质都从调色板派生（buildTerminalChrome），
// 主题里不再手写 ui 字段——之前每套主题各染一圈描边，终端像一块贴上去的色板。

// 带 .js 扩展名：scripts/check-theme-palettes.mjs 在 Node 里直接导入这个文件
import { mixHex } from "../utils/hexColor.js";

export const DEFAULT_TERMINAL_THEME_ID = "xin";
export const TERMINAL_THEME_GROUPS = ["xin", "classic"];

export const TERMINAL_APPEARANCES = ["auto", "light", "dark"];
export const DEFAULT_TERMINAL_APPEARANCE = "auto";
export const TERMINAL_MATERIALS = ["solid", "clear"];
export const DEFAULT_TERMINAL_MATERIAL = "solid";

// 已下线主题的老存档：场景 / 黑客 / 玻璃组都并进 Xin，经典组深色版并回各自的家族
const LEGACY_THEME_IDS = {
  "xin-glass-dark": "xin",
  "xin-glass-light": "xin",
  aurora: "xin",
  "deep-space": "xin",
  synthwave: "xin",
  ember: "xin",
  daylight: "xin",
  matrix: "xin",
  "amber-crt": "xin",
  blueprint: "xin",
  "zero-day": "xin",
  "catppuccin-mocha": "catppuccin",
};

export const TERMINAL_FONTS = [
  { name: "Cascadia Code", stack: '"Cascadia Code", Consolas, monospace' },
  { name: "JetBrains Mono", stack: '"JetBrains Mono", "Cascadia Code", Consolas, monospace' },
  { name: "Fira Code", stack: '"Fira Code", "Cascadia Code", Consolas, monospace' },
  { name: "Victor Mono", stack: '"Victor Mono", "JetBrains Mono", Consolas, monospace' },
  { name: "IBM Plex Mono", stack: '"IBM Plex Mono", "Cascadia Code", Consolas, monospace' },
  { name: "Geist Mono", stack: '"Geist Mono", "Cascadia Code", Consolas, monospace' },
  { name: "Space Mono", stack: '"Space Mono", "IBM Plex Mono", Consolas, monospace' },
];

export const TERMINAL_FONT_SIZES = [12, 13, 14, 15, 16, 18];

export function resolveTerminalFontStack(fontName) {
  const font = TERMINAL_FONTS.find((item) => item.name === fontName);
  return (font ?? TERMINAL_FONTS[0]).stack;
}

export function normalizeTerminalThemeId(themeId) {
  const id = LEGACY_THEME_IDS[themeId] ?? themeId;
  return TERMINAL_THEMES.some((theme) => theme.id === id) ? id : DEFAULT_TERMINAL_THEME_ID;
}

export function normalizeTerminalAppearance(value) {
  return TERMINAL_APPEARANCES.includes(value) ? value : DEFAULT_TERMINAL_APPEARANCE;
}

export function normalizeTerminalMaterial(value) {
  return TERMINAL_MATERIALS.includes(value) ? value : DEFAULT_TERMINAL_MATERIAL;
}

export function getTerminalTheme(themeId) {
  const id = normalizeTerminalThemeId(themeId);
  return TERMINAL_THEMES.find((theme) => theme.id === id);
}

// 家族缺哪一版就退回仅有的那一版：Nord、Dracula 都只有深色，硬造浅色版就不是它了
export function resolveThemeVariant(theme, appearance, appAppearance) {
  const mode = appearance === "auto" ? appAppearance : appearance;
  return theme.variants[mode] ?? theme.variants.dark ?? theme.variants.light;
}

// xterm6 的滚动条是自绘滑块，颜色只认 theme.scrollbarSlider*（::-webkit-scrollbar
// 已不生效），所以并进 xterm 主题一起下发。xterm 自己解析颜色，只给 hex / rgba。
//
// 通透档的 xterm 底色必须是全透明：真正的底色画在面板上（带模糊），
// xterm 再铺一层实色就把壁纸盖死了。实底档给不透明底色，WebGL 字形图集
// 会在实色上光栅化，字边比透明底上的灰度抗锯齿更利落。
export function buildXtermTheme(variant, material) {
  const { xterm } = variant;
  return {
    ...xterm,
    background: material === "clear" ? "rgba(0, 0, 0, 0)" : xterm.background,
    scrollbarSliderBackground: withAlpha(xterm.foreground, 0.26),
    scrollbarSliderHoverBackground: withAlpha(xterm.foreground, 0.38),
    scrollbarSliderActiveBackground: variant.accent,
  };
}

// 面板 chrome 一律由调色板派生：描边、投影只分明暗，不再染主题色——
// 终端和文件面板、监控卡是同一层内容，外框应该说同一种话。
export function buildTerminalChrome(variant, material) {
  const { background, foreground } = variant.xterm;
  const isLight = variant.mode === "light";
  return {
    "--term-panel-bg":
      material === "clear" ? withAlpha(background, isLight ? 0.72 : 0.78) : background,
    "--term-panel-border": isLight ? "rgba(0, 0, 0, 0.08)" : "rgba(255, 255, 255, 0.09)",
    "--term-glow": isLight ? "rgba(24, 24, 27, 0.12)" : "rgba(0, 0, 0, 0.3)",
    "--term-accent": variant.accent,
    "--term-text": foreground,
    "--term-text-muted": withAlpha(foreground, 0.62),
    "--term-soft": withAlpha(foreground, 0.1),
    // 液态玻璃的高光：浅色底上要亮白，深色底上收着，否则一圈白边像贴纸
    "--term-highlight": isLight ? "rgb(255 255 255 / 0.92)" : "rgb(255 255 255 / 0.18)",
  };
}

// 远程编辑器和终端用同一套配色：背景、前景、光标、选区直接取，语法高亮映射到 ANSI 色——
// 和终端里 ls、git 的着色是一个色板，深浅也跟着终端的明暗走（以前写死一块深蓝，浅色下很突兀）。
// 映射沿用 base16 的分工：关键字 magenta、字符串 green、数字 yellow、函数 blue、类型 cyan、
// 属性 / 标签 red（JSON、YAML 的键是这个色，和绿色的值一眼分开）、注释 brightBlack。
// 编辑器总是实底，不跟终端的通透材质：代码区底下透出壁纸会影响读字
export function buildEditorTheme(variant) {
  const { xterm, accent } = variant;
  const tint = (weight) => mixHex(xterm.foreground, xterm.background, weight);
  return {
    "--xin-editor-bg": xterm.background,
    "--xin-editor-bg-gutter": tint(0.025),
    "--xin-editor-bg-panel": tint(0.06),
    "--xin-editor-bg-field": tint(0.04),
    "--xin-editor-text": xterm.foreground,
    "--xin-editor-text-muted": tint(0.45),
    "--xin-editor-edge": tint(0.12),
    "--xin-editor-accent": accent,
    "--xin-editor-accent-soft": withAlpha(accent, 0.08),
    "--xin-editor-accent-line": withAlpha(accent, 0.7),
    "--xin-editor-selection": xterm.selectionBackground,
    "--xin-editor-match": withAlpha(xterm.yellow, 0.2),
    "--xin-editor-search-match": withAlpha(xterm.yellow, 0.3),
    "--xin-editor-search-current": withAlpha(accent, 0.4),
    "--xin-editor-bracket": withAlpha(xterm.yellow, 0.18),
    "--xin-editor-invalid": withAlpha(xterm.red, 0.2),
    // 和终端自绘滚动条同一个配方（buildXtermTheme）
    "--xin-editor-scrollbar": withAlpha(xterm.foreground, 0.26),
    "--xin-editor-scrollbar-hover": withAlpha(xterm.foreground, 0.38),
    "--xin-syntax-keyword": xterm.magenta,
    "--xin-syntax-string": xterm.green,
    "--xin-syntax-number": xterm.yellow,
    "--xin-syntax-comment": xterm.brightBlack,
    "--xin-syntax-function": xterm.blue,
    "--xin-syntax-type": xterm.cyan,
    "--xin-syntax-property": xterm.red,
    "--xin-syntax-operator": tint(0.72),
    "--xin-syntax-meta": xterm.brightMagenta,
    "--xin-syntax-invalid": xterm.red,
  };
}

function withAlpha(hex, alpha) {
  const value = hex.replace("#", "");
  const [r, g, b] = [0, 2, 4].map((offset) => parseInt(value.slice(offset, offset + 2), 16));
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function variant(mode, spec) {
  return { mode, ...spec };
}

export const TERMINAL_THEMES = [
  // —— Xin ——
  // 墨樱：品牌自己的主题，深浅两版。
  // 色板在 OKLCH 里配：各色相亮度拉平、饱和度封顶，没有哪个颜色会「跳」出来。
  // 樱色只落在光标、选区、magenta（也是 root 提醒色）上，和应用的墨樱分工一致。
  {
    id: "xin",
    group: "xin",
    variants: {
      dark: variant("dark", {
        id: "xin-ink-rose",
        nameKey: "terminal.themeName.inkRose",
        accent: "#ec7aa4",
        xterm: {
          // zinc-900：和应用的中性灰阶同源，比深色画布高半档，面板才浮得起来
          background: "#18181b",
          foreground: "#e4e4e7",
          cursor: "#ec7aa4",
          cursorAccent: "#18181b",
          selectionBackground: "rgba(236, 122, 164, 0.26)",
          black: "#3f3f46",
          red: "#ed8174",
          green: "#7fd492",
          yellow: "#eece74",
          blue: "#73aff1",
          magenta: "#ed95bc",
          cyan: "#72d6e1",
          white: "#d4d4d8",
          // 自动补全提示、注释都走 brightBlack：要暗，但不能暗到读不出（≈4:1）
          brightBlack: "#787880",
          brightRed: "#fd978a",
          brightGreen: "#99e7a9",
          brightYellow: "#ffe291",
          brightBlue: "#8bc2ff",
          brightMagenta: "#fdabce",
          brightCyan: "#91e8f2",
          brightWhite: "#fafafa",
        },
      }),
      // 昼：深色版翻过来，色相不动，只把明度压下去。之前那版浅色「宣 Paper」用户不喜欢
      // （浅色终端不适应，名字也不属于墨樱）；后来还是决定给浅色留一个选择，名字跟着墨樱走。
      // 底色是冷中性的 zinc-50，不走暖纸色，和应用的灰阶同源。
      light: variant("light", {
        id: "xin-ink-rose-day",
        nameKey: "terminal.themeName.inkRoseDay",
        // 樱色跟应用浅色的强调色同一个值，深色版也是这样
        accent: "#c93a72",
        xterm: {
          background: "#fafafa",
          foreground: "#27272a",
          cursor: "#c93a72",
          cursorAccent: "#fafafa",
          selectionBackground: "rgba(201, 58, 114, 0.18)",
          black: "#3f3f46",
          // 六个彩色对底色都是 5.2:1 左右（深色版各色亮度不一，浅色下按对比度拉平更稳）。
          // 黄色在这个明度只能是芥末棕，色相往橙偏一点，免得发绿
          red: "#b5443a",
          green: "#18793b",
          yellow: "#8d6105",
          blue: "#106bbd",
          magenta: "#ad4379",
          cyan: "#09757f",
          // 很多程序默认背景是黑的，会拿 white 写字：给中灰而不是近白，浅底上还读得出
          white: "#919197",
          // 自动补全提示、注释：和深色版一样 ≈4:1
          brightBlack: "#7b7b83",
          // bright 在深色版是更亮，在这里是更深（≈6.8:1）——都是「更显眼」。
          // 加粗的目录、可执行文件默认画成 bright 色，所以它们比普通色更沉
          brightRed: "#a9241e",
          brightGreen: "#03662d",
          brightYellow: "#765102",
          brightBlue: "#0159a2",
          brightMagenta: "#a02468",
          brightCyan: "#04616a",
          brightWhite: "#a6a7ac",
        },
      }),
    },
  },

  // —— classics ——
  // 上游原版配色，一个色值都不改——原汁原味是这一组存在的理由。有官方浅色版的配成一对。
  {
    id: "catppuccin",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        accent: "#89b4fa",
        xterm: {
          background: "#1e1e2e",
          foreground: "#cdd6f4",
          cursor: "#f5e0dc",
          cursorAccent: "#1e1e2e",
          selectionBackground: "#585b70",
          black: "#45475a",
          red: "#f38ba8",
          green: "#a6e3a1",
          yellow: "#f9e2af",
          blue: "#89b4fa",
          magenta: "#f5c2e7",
          cyan: "#94e2d5",
          white: "#bac2de",
          brightBlack: "#585b70",
          brightRed: "#f38ba8",
          brightGreen: "#a6e3a1",
          brightYellow: "#f9e2af",
          brightBlue: "#89b4fa",
          brightMagenta: "#f5c2e7",
          brightCyan: "#94e2d5",
          brightWhite: "#a6adc8",
        },
      }),
      light: variant("light", {
        id: "catppuccin-latte",
        name: "Catppuccin Latte",
        accent: "#1e66f5",
        xterm: {
          background: "#eff1f5",
          foreground: "#4c4f69",
          cursor: "#dc8a78",
          cursorAccent: "#eff1f5",
          selectionBackground: "#acb0be",
          black: "#5c5f77",
          red: "#d20f39",
          green: "#40a02b",
          yellow: "#df8e1d",
          blue: "#1e66f5",
          magenta: "#ea76cb",
          cyan: "#179299",
          white: "#acb0be",
          brightBlack: "#6c6f85",
          brightRed: "#d20f39",
          brightGreen: "#40a02b",
          brightYellow: "#df8e1d",
          brightBlue: "#1e66f5",
          brightMagenta: "#ea76cb",
          brightCyan: "#179299",
          brightWhite: "#bcc0cc",
        },
      }),
    },
  },
  {
    id: "tokyo-night",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "tokyo-night",
        name: "Tokyo Night",
        accent: "#7aa2f7",
        xterm: {
          background: "#1a1b26",
          foreground: "#c0caf5",
          cursor: "#c0caf5",
          cursorAccent: "#1a1b26",
          selectionBackground: "#33467c",
          black: "#15161e",
          red: "#f7768e",
          green: "#9ece6a",
          yellow: "#e0af68",
          blue: "#7aa2f7",
          magenta: "#bb9af7",
          cyan: "#7dcfff",
          white: "#a9b1d6",
          brightBlack: "#414868",
          brightRed: "#f7768e",
          brightGreen: "#9ece6a",
          brightYellow: "#e0af68",
          brightBlue: "#7aa2f7",
          brightMagenta: "#bb9af7",
          brightCyan: "#7dcfff",
          brightWhite: "#c0caf5",
        },
      }),
      light: variant("light", {
        id: "tokyo-night-day",
        name: "Tokyo Night Day",
        accent: "#2e7de9",
        xterm: {
          background: "#e1e2e7",
          foreground: "#3760bf",
          cursor: "#3760bf",
          cursorAccent: "#e1e2e7",
          selectionBackground: "#b7c1e3",
          black: "#b4b5b9",
          red: "#f52a65",
          green: "#587539",
          yellow: "#8c6c3e",
          blue: "#2e7de9",
          magenta: "#9854f1",
          cyan: "#007197",
          white: "#6172b0",
          brightBlack: "#a1a6c5",
          brightRed: "#ff4774",
          brightGreen: "#5c8524",
          brightYellow: "#a27629",
          brightBlue: "#358aff",
          brightMagenta: "#a463ff",
          brightCyan: "#007ea8",
          brightWhite: "#3760bf",
        },
      }),
    },
  },
  {
    id: "rose-pine",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "rose-pine",
        name: "Rosé Pine",
        accent: "#c4a7e7",
        xterm: {
          background: "#191724",
          foreground: "#e0def4",
          cursor: "#e0def4",
          cursorAccent: "#191724",
          selectionBackground: "#403d52",
          black: "#26233a",
          red: "#eb6f92",
          green: "#31748f",
          yellow: "#f6c177",
          blue: "#9ccfd8",
          magenta: "#c4a7e7",
          cyan: "#ebbcba",
          white: "#e0def4",
          brightBlack: "#6e6a86",
          brightRed: "#eb6f92",
          brightGreen: "#31748f",
          brightYellow: "#f6c177",
          brightBlue: "#9ccfd8",
          brightMagenta: "#c4a7e7",
          brightCyan: "#ebbcba",
          brightWhite: "#e0def4",
        },
      }),
      light: variant("light", {
        id: "rose-pine-dawn",
        name: "Rosé Pine Dawn",
        accent: "#907aa9",
        xterm: {
          background: "#faf4ed",
          foreground: "#575279",
          cursor: "#575279",
          cursorAccent: "#faf4ed",
          selectionBackground: "#dfdad9",
          black: "#f2e9e1",
          red: "#b4637a",
          green: "#286983",
          yellow: "#ea9d34",
          blue: "#56949f",
          magenta: "#907aa9",
          cyan: "#d7827e",
          white: "#575279",
          brightBlack: "#9893a5",
          brightRed: "#b4637a",
          brightGreen: "#286983",
          brightYellow: "#ea9d34",
          brightBlue: "#56949f",
          brightMagenta: "#907aa9",
          brightCyan: "#d7827e",
          brightWhite: "#575279",
        },
      }),
    },
  },
  // Gruvbox Material（sainnhe）：原版 Gruvbox 的柔化版，暖色低眩光，长时间盯着最省眼。
  // brightBlack 用它的 grey 而不是 bg1，自动补全提示才看得见。
  {
    id: "gruvbox-material",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "gruvbox-material-dark",
        name: "Gruvbox Material",
        accent: "#d8a657",
        xterm: {
          background: "#282828",
          foreground: "#d4be98",
          cursor: "#d4be98",
          cursorAccent: "#282828",
          selectionBackground: "#45403d",
          black: "#3c3836",
          red: "#ea6962",
          green: "#a9b665",
          yellow: "#d8a657",
          blue: "#7daea3",
          magenta: "#d3869b",
          cyan: "#89b482",
          white: "#d4be98",
          brightBlack: "#928374",
          brightRed: "#ea6962",
          brightGreen: "#a9b665",
          brightYellow: "#d8a657",
          brightBlue: "#7daea3",
          brightMagenta: "#d3869b",
          brightCyan: "#89b482",
          brightWhite: "#ddc7a1",
        },
      }),
      light: variant("light", {
        id: "gruvbox-material-light",
        name: "Gruvbox Material Light",
        accent: "#b47109",
        xterm: {
          background: "#fbf1c7",
          foreground: "#654735",
          cursor: "#654735",
          cursorAccent: "#fbf1c7",
          selectionBackground: "#e6d5ae",
          black: "#654735",
          red: "#c14a4a",
          green: "#6c782e",
          yellow: "#b47109",
          blue: "#45707a",
          magenta: "#945e80",
          cyan: "#4c7a5d",
          white: "#f2e5bc",
          brightBlack: "#928374",
          brightRed: "#c14a4a",
          brightGreen: "#6c782e",
          brightYellow: "#b47109",
          brightBlue: "#45707a",
          brightMagenta: "#945e80",
          brightCyan: "#4c7a5d",
          brightWhite: "#eee0b7",
        },
      }),
    },
  },
  {
    id: "nord",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "nord",
        name: "Nord",
        accent: "#88c0d0",
        xterm: {
          background: "#2e3440",
          foreground: "#d8dee9",
          cursor: "#d8dee9",
          cursorAccent: "#2e3440",
          selectionBackground: "#434c5e",
          black: "#3b4252",
          red: "#bf616a",
          green: "#a3be8c",
          yellow: "#ebcb8b",
          blue: "#81a1c1",
          magenta: "#b48ead",
          cyan: "#88c0d0",
          white: "#e5e9f0",
          brightBlack: "#616e88",
          brightRed: "#bf616a",
          brightGreen: "#a3be8c",
          brightYellow: "#ebcb8b",
          brightBlue: "#81a1c1",
          brightMagenta: "#b48ead",
          brightCyan: "#8fbcbb",
          brightWhite: "#eceff4",
        },
      }),
    },
  },
  {
    id: "vesper",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "vesper",
        name: "Vesper",
        accent: "#ffc799",
        xterm: {
          background: "#101010",
          foreground: "#dbdbdb",
          cursor: "#ffc799",
          cursorAccent: "#101010",
          selectionBackground: "#343434",
          black: "#282828",
          red: "#ff8080",
          green: "#99ffe4",
          yellow: "#ffc799",
          blue: "#90b0d0",
          magenta: "#c8a2d8",
          cyan: "#7adfcf",
          white: "#cdcdcd",
          brightBlack: "#575757",
          brightRed: "#ff9f9f",
          brightGreen: "#b8ffe9",
          brightYellow: "#ffd7b1",
          brightBlue: "#b0c8e8",
          brightMagenta: "#ddc0ea",
          brightCyan: "#a3efe0",
          brightWhite: "#ffffff",
        },
      }),
    },
  },
  {
    id: "dracula",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "dracula",
        name: "Dracula",
        accent: "#bd93f9",
        xterm: {
          background: "#282a36",
          foreground: "#f8f8f2",
          cursor: "#f8f8f2",
          cursorAccent: "#282a36",
          selectionBackground: "#44475a",
          black: "#21222c",
          red: "#ff5555",
          green: "#50fa7b",
          yellow: "#f1fa8c",
          blue: "#bd93f9",
          magenta: "#ff79c6",
          cyan: "#8be9fd",
          white: "#f8f8f2",
          brightBlack: "#6272a4",
          brightRed: "#ff6e6e",
          brightGreen: "#69ff94",
          brightYellow: "#ffffa5",
          brightBlue: "#d6acff",
          brightMagenta: "#ff92df",
          brightCyan: "#a4ffff",
          brightWhite: "#ffffff",
        },
      }),
    },
  },
  {
    id: "poimandres",
    group: "classic",
    variants: {
      dark: variant("dark", {
        id: "poimandres",
        name: "Poimandres",
        accent: "#5de4c7",
        xterm: {
          background: "#1b1e28",
          foreground: "#a6accd",
          cursor: "#5de4c7",
          cursorAccent: "#1b1e28",
          selectionBackground: "#303340",
          black: "#303340",
          red: "#d0679d",
          green: "#5de4c7",
          yellow: "#fffac2",
          blue: "#89ddff",
          magenta: "#fcc5e9",
          cyan: "#add7ff",
          white: "#ffffff",
          brightBlack: "#767c9d",
          brightRed: "#d0679d",
          brightGreen: "#5de4c7",
          brightYellow: "#fffac2",
          brightBlue: "#89ddff",
          brightMagenta: "#fcc5e9",
          brightCyan: "#add7ff",
          brightWhite: "#ffffff",
        },
      }),
    },
  },
];
