// Terminal appearance presets. A theme drives the xterm palette, the terminal
// screen backdrop and the panel chrome via --term-* CSS variables injected by
// TerminalWorkspace.vue.
//
// 两条约定，为的是「选择器里看到的 == 终端里看到的」：
//   1. screenBg 里的渐变一律用百分比尺寸/位置，缩到 145×76 的预览卡里能等比还原；
//      px 单位只留给 repeating-* 纹理（扫描线、网格），它们本来就该保持物理密度。
//   2. 每个主题挑一种**结构**上不同的纹理（帘幕 / 星野 / 透视网格 / 地平线 /
//      扫描线 / 蓝图网格 / 颗粒），而不是只换色相——否则再多主题也是换汤不换药。
//
// Groups:
//   glass   — transparent terminal over the app's liquid-glass ambience
//   scene   — layered artwork behind a transparent terminal
//   hacker  — phosphor / CRT / blueprint fantasies
//   classic — beloved editor palettes with their authentic solid background

export const DEFAULT_TERMINAL_THEME_ID = "aurora";

export const TERMINAL_THEME_GROUPS = ["glass", "scene", "hacker", "classic"];

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

export function getTerminalTheme(themeId) {
  return (
    TERMINAL_THEMES.find((theme) => theme.id === themeId) ??
    TERMINAL_THEMES.find((theme) => theme.id === DEFAULT_TERMINAL_THEME_ID)
  );
}

// 预览卡直接烧主题真正的背景栈；只有玻璃组的屏幕是透明的（靠应用底色透过来），
// 才需要 theme.preview 这个专门画的「环境缩影」。
export function getThemePreviewBackground(theme) {
  return theme.preview ?? theme.ui.screenBg;
}

// xterm6 的滚动条是自绘滑块，颜色只认 theme.scrollbarSlider*（::-webkit-scrollbar
// 已不生效），所以把 UI 侧的滑块配色并进 xterm 主题一起下发。
export function buildXtermTheme(theme) {
  return {
    ...theme.xterm,
    scrollbarSliderBackground: theme.ui.scrollbarThumb,
    scrollbarSliderHoverBackground: theme.ui.scrollbarThumb,
    scrollbarSliderActiveBackground: theme.accent,
  };
}

// CSS background 是「先写的盖在后写的上面」，用它组装图层比手写一长串好读。
function stack(...layers) {
  return layers.filter(Boolean).join(", ");
}

// —— 共享纹理 ——
// 扫描线 / 暗角画在文字**之上**（screenOverlay），CRT 的味道才对；
// 网格、颗粒、星野画在文字**之下**（screenBg），保证字口干净。

const SCANLINES =
  "repeating-linear-gradient(180deg, rgba(0, 0, 0, 0.22) 0 1px, transparent 1px 3px)";
const SCANLINES_SOFT =
  "repeating-linear-gradient(180deg, rgba(0, 0, 0, 0.15) 0 1px, transparent 1px 4px)";
const VIGNETTE =
  "radial-gradient(126% 106% at 50% 46%, transparent 42%, rgba(0, 0, 0, 0.5) 100%)";
const VIGNETTE_SOFT =
  "radial-gradient(130% 110% at 50% 44%, transparent 52%, rgba(0, 0, 0, 0.32) 100%)";
const CRT_GLARE =
  "linear-gradient(180deg, rgba(255, 226, 184, 0.07) 0%, transparent 24%)";
// feTurbulence 颗粒：比再叠一层半透明色更像胶片，且不需要额外图片资源
const GRAIN =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.82' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='160' height='160' filter='url(%23n)' opacity='0.4'/%3E%3C/svg%3E\")";

// 星野：沿一条斜穿画面的银河带加密，带外稀疏；坐标全部钉死，保证每次渲染一致。
// [x%, y%, 半径px, 亮度, 色温]——色温混着排才像真的星空，全白会假。
const STAR_TINTS = {
  w: "255, 255, 255", // 白
  b: "188, 208, 255", // 蓝白（年轻恒星）
  g: "255, 234, 198", // 暖金（老年恒星）
};

const DEEP_SPACE_STARS = [
  // 银河带：左下 → 右上
  [8, 89, 1.2, 0.7, "b"],
  [13, 84, 1, 0.5, "w"],
  [17, 81, 1.5, 0.85, "w"],
  [21, 77, 1, 0.45, "b"],
  [25, 74, 1.3, 0.72, "w"],
  [29, 70, 1, 0.5, "g"],
  [33, 67, 1.4, 0.8, "b"],
  [37, 63, 1, 0.46, "w"],
  [41, 59, 1.2, 0.66, "w"],
  [45, 55, 1, 0.5, "b"],
  [49, 52, 1.5, 0.88, "w"],
  [53, 48, 1, 0.44, "g"],
  [57, 45, 1.3, 0.7, "b"],
  [61, 41, 1, 0.5, "w"],
  [65, 37, 1.4, 0.8, "w"],
  [69, 34, 1, 0.46, "b"],
  [73, 30, 1.2, 0.66, "g"],
  [77, 26, 1, 0.5, "w"],
  [81, 23, 1.5, 0.85, "b"],
  [85, 19, 1, 0.45, "w"],
  [90, 15, 1.3, 0.72, "w"],
  // 带外：稀疏
  [5, 30, 1.1, 0.5, "w"],
  [14, 52, 1, 0.4, "b"],
  [23, 16, 1.3, 0.6, "g"],
  [31, 93, 1, 0.42, "w"],
  [39, 24, 1.2, 0.55, "b"],
  [47, 87, 1, 0.4, "w"],
  [56, 12, 1.3, 0.6, "w"],
  [64, 78, 1, 0.42, "b"],
  [72, 62, 1.1, 0.5, "g"],
  [79, 88, 1, 0.4, "w"],
  [88, 58, 1.2, 0.55, "b"],
  [95, 34, 1, 0.45, "w"],
  [3, 68, 1.1, 0.48, "b"],
]
  .map(([x, y, r, a, tint]) => {
    // 关键：给每颗星钉死 `position / size no-repeat`。不写 size 的话每一层都会
    // 铺满整个元素，为了一个 1.3px 的点把整屏渐变算一遍——34 颗星就是 34 遍，
    // 拖窗口改尺寸时会实打实地掉帧。钉上尺寸后光栅面积只有几个像素。
    const box = (r * 2).toFixed(1);
    return `radial-gradient(closest-side, rgba(${STAR_TINTS[tint]}, ${a}), transparent 100%) ${x}% ${y}% / ${box}px ${box}px no-repeat`;
  })
  .join(", ");

// 亮星的十字衍射星芒：核心 + 横竖两道细长椭圆 + 一圈软晕。
// 这是「像照片」和「像色块」的分界线，值得多这几层。
// closest-side 在非正方形的框里出椭圆，正好就是星芒那两道细长光。
const DEEP_SPACE_FLARES = [
  [19, 27, "232, 240, 255"],
  [58, 45, "255, 246, 226"],
  [84, 71, "212, 228, 255"],
]
  .map(([x, y, rgb]) =>
    [
      `radial-gradient(closest-side, rgba(${rgb}, 1), rgba(${rgb}, 0.4) 46%, transparent 100%) ${x}% ${y}% / 5px 5px no-repeat`,
      `radial-gradient(closest-side, rgba(${rgb}, 0.5), transparent 100%) ${x}% ${y}% / 38px 1.6px no-repeat`,
      `radial-gradient(closest-side, rgba(${rgb}, 0.45), transparent 100%) ${x}% ${y}% / 1.6px 38px no-repeat`,
      `radial-gradient(closest-side, rgba(${rgb}, 0.2), transparent 100%) ${x}% ${y}% / 18px 18px no-repeat`,
    ].join(", "),
  )
  .join(", ");

// 银河带本体：垂直于星带方向的一条软光带
const MILKY_WAY =
  "linear-gradient(125deg, transparent 29%, rgba(186, 202, 255, 0.05) 40%, rgba(228, 218, 255, 0.078) 48%, rgba(186, 202, 255, 0.045) 56%, transparent 67%)";

// 文字处理的默认档。主题只写和默认不同的那几项，其余走这里。
// glow 走 CSS text-shadow 压在 .xterm-rows 上——xterm 这里用的是 DOM renderer
// （没挂 canvas / webgl addon），所以 CSS 能直接作用到字上；换渲染器会失效。
// weight / cursorStyle 是 xterm 选项，两者都不改变字符测量结果，切主题不必 refit。
const DEFAULT_TYPOGRAPHY = {
  glow: null,
  weight: 400,
  cursorStyle: "block",
};

export function getThemeTypography(theme) {
  return { ...DEFAULT_TYPOGRAPHY, ...theme.typography };
}

export const TERMINAL_THEMES = [
  // —— glass ——
  {
    id: "xin-glass-dark",
    name: "Xin Glass",
    group: "glass",
    mode: "dark",
    accent: "#89b4fa",
    // 玻璃组屏幕是透明的，预览卡画一张应用环境光的缩影
    preview: stack(
      "radial-gradient(72% 62% at 18% -6%, rgba(126, 156, 255, 0.34), transparent 66%)",
      "radial-gradient(64% 56% at 98% 28%, rgba(56, 189, 248, 0.2), transparent 62%)",
      "linear-gradient(160deg, #242843 0%, #191c2d 100%)",
    ),
    ui: {
      panelBg: "rgba(21, 23, 40, 0.84)",
      panelBorder: "rgba(255, 255, 255, 0.12)",
      toolbarBg: "rgba(255, 255, 255, 0.04)",
      divider: "rgba(255, 255, 255, 0.08)",
      screenBg: "transparent",
      screenOverlay: null,
      glow: "rgba(89, 108, 200, 0.3)",
      text: "#e6ebff",
      textMuted: "rgba(230, 235, 255, 0.6)",
      soft: "rgba(230, 235, 255, 0.1)",
      scrollbarThumb: "rgba(205, 214, 244, 0.32)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#e4e9ff",
      cursor: "#89b4fa",
      cursorAccent: "#151728",
      selectionBackground: "rgba(137, 180, 250, 0.3)",
      black: "#494d64",
      red: "#f38ba8",
      green: "#a6e3a1",
      yellow: "#f9e2af",
      blue: "#89b4fa",
      magenta: "#f5c2e7",
      cyan: "#7fdcf0",
      white: "#cdd6f4",
      brightBlack: "#6c7086",
      brightRed: "#f38ba8",
      brightGreen: "#a6e3a1",
      brightYellow: "#f9e2af",
      brightBlue: "#89b4fa",
      brightMagenta: "#f5c2e7",
      brightCyan: "#7fdcf0",
      brightWhite: "#ffffff",
    },
  },
  {
    id: "xin-glass-light",
    name: "Xin Glass Light",
    group: "glass",
    mode: "light",
    accent: "#155eef",
    preview: stack(
      "radial-gradient(72% 62% at 16% -8%, rgba(120, 160, 255, 0.3), transparent 64%)",
      "radial-gradient(66% 56% at 98% 24%, rgba(56, 189, 248, 0.22), transparent 60%)",
      "linear-gradient(160deg, #fbfdff 0%, #e7edf9 100%)",
    ),
    ui: {
      panelBg: "rgba(255, 255, 255, 0.55)",
      panelBorder: "rgba(255, 255, 255, 0.62)",
      toolbarBg: "rgba(255, 255, 255, 0.3)",
      divider: "rgba(16, 26, 44, 0.08)",
      screenBg: "transparent",
      screenOverlay: null,
      glow: "rgba(20, 34, 62, 0.14)",
      text: "#101a2c",
      textMuted: "#5d6c84",
      soft: "rgba(16, 26, 44, 0.07)",
      scrollbarThumb: "rgba(90, 105, 135, 0.4)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(255, 255, 255, 0)",
      foreground: "#2c3852",
      cursor: "#155eef",
      cursorAccent: "#ffffff",
      selectionBackground: "rgba(47, 109, 246, 0.2)",
      black: "#3d4352",
      red: "#d12f42",
      green: "#1a7f37",
      yellow: "#9a6700",
      blue: "#0969da",
      magenta: "#8250df",
      cyan: "#0f7d87",
      white: "#8792a8",
      brightBlack: "#57657f",
      brightRed: "#e5484d",
      brightGreen: "#2da44e",
      brightYellow: "#bf8700",
      brightBlue: "#218bff",
      brightMagenta: "#a475f9",
      brightCyan: "#3192aa",
      brightWhite: "#aab4c8",
    },
  },

  // —— scenes ——
  // 极光：斜向的软边帘幕，靠几条宽过渡带堆出来（不是又一团 radial 光晕）
  {
    id: "aurora",
    name: "Aurora",
    group: "scene",
    mode: "dark",
    accent: "#2dd4bf",
    ui: {
      panelBg: "rgba(7, 22, 30, 0.82)",
      panelBorder: "rgba(148, 233, 214, 0.16)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: stack(
        "radial-gradient(120% 60% at 50% -10%, rgba(45, 212, 191, 0.17), transparent 68%)",
        "linear-gradient(101deg, transparent 5%, rgba(94, 234, 212, 0.15) 14%, transparent 25%)",
        "linear-gradient(97deg, transparent 26%, rgba(129, 140, 248, 0.15) 36%, transparent 47%)",
        "linear-gradient(104deg, transparent 49%, rgba(52, 211, 153, 0.12) 60%, transparent 71%)",
        "linear-gradient(99deg, transparent 71%, rgba(56, 189, 248, 0.1) 82%, transparent 93%)",
        "radial-gradient(92% 44% at 50% 110%, rgba(16, 185, 129, 0.12), transparent 70%)",
        "linear-gradient(180deg, #04121b 0%, #071c27 52%, #030f18 100%)",
      ),
      screenOverlay: null,
      glow: "rgba(45, 212, 191, 0.22)",
      text: "#d7f0e9",
      textMuted: "rgba(215, 240, 233, 0.58)",
      soft: "rgba(120, 220, 200, 0.12)",
      scrollbarThumb: "rgba(150, 220, 205, 0.3)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#cfeee6",
      cursor: "#5eead4",
      cursorAccent: "#04121b",
      selectionBackground: "rgba(45, 212, 191, 0.3)",
      black: "#153140",
      red: "#ff7a93",
      green: "#3ddb7a",
      yellow: "#ffd48a",
      blue: "#6aa5ff",
      magenta: "#b39dfb",
      cyan: "#22d3ee",
      white: "#c7e9e0",
      brightBlack: "#2f5a6b",
      brightRed: "#ff9fb2",
      brightGreen: "#6ded9e",
      brightYellow: "#ffe6b3",
      brightBlue: "#9ac2ff",
      brightMagenta: "#cdbcfd",
      brightCyan: "#67e8f9",
      brightWhite: "#eafff9",
    },
  },
  // 深空：十字星芒亮星 + 34 颗色温不一的星 + 银河带 + 三层星云。
  // 全静态——终端是开一整天的工具，常驻合成动画在笔记本上是实打实的耗电。
  {
    id: "deep-space",
    name: "Deep Space",
    group: "scene",
    mode: "dark",
    accent: "#8fa7ff",
    typography: {
      glow: "0 0 7px rgba(143, 167, 255, 0.2)",
    },
    ui: {
      panelBg: "rgba(6, 8, 20, 0.84)",
      panelBorder: "rgba(143, 167, 255, 0.18)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: stack(
        DEEP_SPACE_FLARES,
        DEEP_SPACE_STARS,
        MILKY_WAY,
        "radial-gradient(58% 40% at 76% 8%, rgba(99, 102, 241, 0.3), transparent 62%)",
        "radial-gradient(40% 28% at 85% 17%, rgba(56, 189, 248, 0.16), transparent 60%)",
        "radial-gradient(52% 38% at 11% 77%, rgba(217, 70, 239, 0.18), transparent 62%)",
        "radial-gradient(34% 25% at 20% 85%, rgba(244, 114, 182, 0.12), transparent 58%)",
        "radial-gradient(120% 68% at 50% 128%, rgba(45, 212, 191, 0.07), transparent 66%)",
        // 底色用径向而不是线性：天然带一圈暗角，画面自己就收得住
        "radial-gradient(142% 112% at 50% 38%, #080b1e 0%, #04050f 56%, #010206 100%)",
      ),
      screenOverlay: null,
      glow: "rgba(99, 102, 241, 0.28)",
      text: "#dfe4ff",
      textMuted: "rgba(223, 228, 255, 0.58)",
      soft: "rgba(143, 167, 255, 0.12)",
      scrollbarThumb: "rgba(200, 208, 255, 0.3)",
      scrollbarEdge: "transparent",
    },
    // 星光配色：整体偏冷，黄色取「星等金」而不是纯黄，红取星云的粉红
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#dde3ff",
      cursor: "#a5b4fc",
      cursorAccent: "#04050f",
      selectionBackground: "rgba(143, 167, 255, 0.32)",
      black: "#1b2140",
      red: "#ff8ba4",
      green: "#7ee7a8",
      yellow: "#ffd98a",
      blue: "#8fa7ff",
      magenta: "#e39bff",
      cyan: "#6be3f5",
      white: "#c8d0f0",
      brightBlack: "#3d4674",
      brightRed: "#ffadbf",
      brightGreen: "#a3f3c4",
      brightYellow: "#ffe9b4",
      brightBlue: "#b3c2ff",
      brightMagenta: "#f0bcff",
      brightCyan: "#96f0fd",
      brightWhite: "#eef1ff",
    },
  },
  // Synthwave：真·透视网格——conic 打出向消失点收拢的竖线，天空层盖住地平线以上，
  // 横线用手写的递增间距做纵深，再压一颗落日。
  {
    id: "synthwave",
    name: "Synthwave",
    group: "scene",
    mode: "dark",
    accent: "#ff7edb",
    typography: {
      glow: "0 0 8px rgba(255, 126, 219, 0.28)",
    },
    ui: {
      panelBg: "rgba(20, 10, 36, 0.82)",
      panelBorder: "rgba(255, 126, 219, 0.18)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: stack(
        "linear-gradient(180deg, transparent 60.5%, rgba(255, 126, 219, 0.3) 62%, transparent 63.5%)",
        "radial-gradient(19% 13% at 50% 60%, rgba(255, 196, 120, 0.34), rgba(255, 95, 150, 0.2) 62%, transparent 66%)",
        "linear-gradient(180deg, #170b2a 0%, #1b0d33 54%, rgba(27, 13, 51, 0) 62%)",
        "linear-gradient(180deg, transparent 0 63%, rgba(255, 126, 219, 0.14) 63% 63.3%, transparent 63.3% 67%, rgba(255, 126, 219, 0.15) 67% 67.4%, transparent 67.4% 72.5%, rgba(255, 126, 219, 0.16) 72.5% 73%, transparent 73% 80%, rgba(255, 126, 219, 0.18) 80% 80.6%, transparent 80.6% 90%, rgba(255, 126, 219, 0.2) 90% 90.8%, transparent 90.8% 100%)",
        "repeating-conic-gradient(from 182deg at 50% 62%, rgba(255, 126, 219, 0.18) 0deg 0.4deg, transparent 0.4deg 6deg)",
        "radial-gradient(70% 40% at 50% 112%, rgba(168, 85, 247, 0.22), transparent 68%)",
        "linear-gradient(180deg, #150a26 0%, #1a0d30 60%, #0f0619 100%)",
      ),
      screenOverlay: null,
      glow: "rgba(236, 72, 153, 0.24)",
      text: "#f4ecff",
      textMuted: "rgba(244, 236, 255, 0.6)",
      soft: "rgba(255, 126, 219, 0.12)",
      scrollbarThumb: "rgba(244, 236, 255, 0.3)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#f2e9ff",
      cursor: "#ff7edb",
      cursorAccent: "#150a26",
      selectionBackground: "rgba(255, 126, 219, 0.3)",
      black: "#2a1a44",
      red: "#fe4450",
      green: "#72f1b8",
      yellow: "#fede5d",
      blue: "#58b2ff",
      magenta: "#ff7edb",
      cyan: "#36f9f6",
      white: "#e6dcff",
      brightBlack: "#514373",
      brightRed: "#ff6e77",
      brightGreen: "#a1ffd8",
      brightYellow: "#ffec8a",
      brightBlue: "#8ccaff",
      brightMagenta: "#ffa3e8",
      brightCyan: "#8bfbfa",
      brightWhite: "#ffffff",
    },
  },
  // 余烬：光源全部压到画面**下缘**（火线地平线），和极光的顶部帘幕正好相反。
  // 配色整体推暖：蓝退成烟青、绿退成橄榄，像被火光染过一样。
  {
    id: "ember",
    name: "Ember",
    group: "scene",
    mode: "dark",
    accent: "#fb923c",
    ui: {
      panelBg: "rgba(21, 12, 7, 0.84)",
      panelBorder: "rgba(251, 146, 60, 0.16)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: stack(
        "radial-gradient(86% 40% at 50% 114%, rgba(255, 146, 60, 0.34), rgba(239, 68, 68, 0.12) 46%, transparent 74%)",
        "radial-gradient(52% 28% at 16% 108%, rgba(255, 196, 88, 0.16), transparent 72%)",
        "radial-gradient(58% 32% at 88% 104%, rgba(244, 63, 94, 0.13), transparent 70%)",
        "linear-gradient(180deg, transparent 84%, rgba(255, 176, 92, 0.09) 92%, transparent 97%)",
        "radial-gradient(112% 58% at 50% -16%, rgba(120, 53, 15, 0.3), transparent 66%)",
        "linear-gradient(180deg, #140b06 0%, #1c1008 56%, #100704 100%)",
      ),
      screenOverlay: null,
      glow: "rgba(251, 146, 60, 0.22)",
      text: "#f5e6d8",
      textMuted: "rgba(245, 230, 216, 0.6)",
      soft: "rgba(251, 146, 60, 0.12)",
      scrollbarThumb: "rgba(245, 230, 216, 0.3)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#f7e4cf",
      cursor: "#fb923c",
      cursorAccent: "#140b06",
      selectionBackground: "rgba(251, 146, 60, 0.3)",
      black: "#3a2418",
      red: "#ff6b5a",
      green: "#c9c04a",
      yellow: "#ffb43f",
      blue: "#6fb0cf",
      magenta: "#e08a86",
      cyan: "#a9d4a0",
      white: "#ecd9c0",
      brightBlack: "#63472f",
      brightRed: "#ff8f80",
      brightGreen: "#dfd672",
      brightYellow: "#ffcd72",
      brightBlue: "#9acbe0",
      brightMagenta: "#f0aaa6",
      brightCyan: "#c5e5bf",
      brightWhite: "#fff2e0",
    },
  },
  // Daylight：唯一的浅色场景。白天开着强光看深色终端最费眼，这一档才是「明显不一样」
  {
    id: "daylight",
    name: "Daylight",
    group: "scene",
    mode: "light",
    accent: "#e06c2b",
    ui: {
      panelBg: "rgba(255, 255, 255, 0.6)",
      panelBorder: "rgba(255, 255, 255, 0.7)",
      toolbarBg: "rgba(255, 255, 255, 0.38)",
      divider: "rgba(16, 26, 44, 0.08)",
      screenBg: stack(
        "radial-gradient(70% 54% at 6% -12%, rgba(255, 214, 170, 0.55), transparent 62%)",
        "radial-gradient(64% 48% at 97% 0%, rgba(173, 206, 255, 0.5), transparent 60%)",
        "radial-gradient(92% 52% at 50% 118%, rgba(196, 216, 255, 0.42), transparent 66%)",
        "linear-gradient(180deg, #fdfcf9 0%, #f1f4fb 100%)",
      ),
      screenOverlay: null,
      glow: "rgba(20, 34, 62, 0.13)",
      text: "#1c2536",
      textMuted: "#5d6c84",
      soft: "rgba(16, 26, 44, 0.07)",
      scrollbarThumb: "rgba(90, 105, 135, 0.38)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(255, 255, 255, 0)",
      foreground: "#2b3648",
      cursor: "#e06c2b",
      cursorAccent: "#fdfcf9",
      selectionBackground: "rgba(224, 108, 43, 0.22)",
      black: "#3f4655",
      red: "#c93b3b",
      green: "#2f8a4f",
      yellow: "#a4700a",
      blue: "#2563eb",
      magenta: "#8b46c9",
      cyan: "#0f7d8c",
      white: "#8b94a6",
      brightBlack: "#5a6479",
      brightRed: "#e05252",
      brightGreen: "#3aa563",
      brightYellow: "#c08a1c",
      brightBlue: "#4b83f0",
      brightMagenta: "#a463dd",
      brightCyan: "#2b98a6",
      brightWhite: "#aab3c2",
    },
  },

  // —— hacker ——
  // Matrix：磷光 + 垂直「雨柱」导轨（背景层），扫描线和暗角压在文字之上。
  // 配色推成近单色磷光——只有 red 保留可辨的红，否则 ls 的报错、diff 的删除行
  // 会彻底看不出来，「好看」不该以看不懂输出为代价。
  {
    id: "matrix",
    name: "Matrix",
    group: "hacker",
    mode: "dark",
    accent: "#00ff88",
    typography: {
      glow: "0 0 6px rgba(0, 255, 136, 0.45)",
      weight: 500,
    },
    ui: {
      panelBg: "rgba(2, 16, 9, 0.86)",
      panelBorder: "rgba(0, 255, 136, 0.18)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.06)",
      screenBg: stack(
        "repeating-linear-gradient(90deg, rgba(0, 255, 136, 0.05) 0 1px, transparent 1px 13px)",
        "radial-gradient(88% 54% at 50% -12%, rgba(0, 255, 136, 0.15), transparent 66%)",
        "radial-gradient(72% 44% at 50% 114%, rgba(0, 255, 136, 0.08), transparent 68%)",
        "linear-gradient(180deg, #010b06 0%, #02160d 55%, #010805 100%)",
      ),
      screenOverlay: stack(SCANLINES_SOFT, VIGNETTE),
      glow: "rgba(0, 255, 136, 0.2)",
      text: "#c9ffe2",
      textMuted: "rgba(201, 255, 226, 0.55)",
      soft: "rgba(0, 255, 136, 0.1)",
      scrollbarThumb: "rgba(0, 255, 136, 0.28)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#8dffbe",
      cursor: "#00ff88",
      cursorAccent: "#01110a",
      selectionBackground: "rgba(0, 255, 136, 0.28)",
      black: "#0a2416",
      red: "#ff5f57",
      green: "#00ff88",
      yellow: "#b6ff4d",
      blue: "#2fb7f0",
      magenta: "#a9ffdb",
      cyan: "#7ff2e4",
      white: "#a8ffd0",
      brightBlack: "#1b5533",
      brightRed: "#ff8a83",
      brightGreen: "#5cffab",
      brightYellow: "#d2ff8a",
      brightBlue: "#6bd0f8",
      brightMagenta: "#c9ffe9",
      brightCyan: "#a8f8ee",
      brightWhite: "#e4fff0",
    },
  },
  // Amber CRT：最重的一档扫描线 + 强暗角 + 顶部弧面眩光，尽量还原显像管
  {
    id: "amber-crt",
    name: "Amber CRT",
    group: "hacker",
    mode: "dark",
    accent: "#ffb000",
    typography: {
      glow: "0 0 6px rgba(255, 176, 0, 0.42)",
      weight: 500,
    },
    ui: {
      panelBg: "rgba(18, 10, 2, 0.86)",
      panelBorder: "rgba(255, 176, 0, 0.18)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.06)",
      screenBg: stack(
        "radial-gradient(80% 50% at 50% 108%, rgba(255, 176, 0, 0.17), transparent 68%)",
        "radial-gradient(120% 68% at 50% -20%, rgba(255, 138, 0, 0.13), transparent 62%)",
        "linear-gradient(180deg, #0c0600 0%, #180d02 56%, #080400 100%)",
      ),
      screenOverlay: stack(CRT_GLARE, SCANLINES, VIGNETTE),
      glow: "rgba(255, 176, 0, 0.22)",
      text: "#ffd9a0",
      textMuted: "rgba(255, 217, 160, 0.55)",
      soft: "rgba(255, 176, 0, 0.12)",
      scrollbarThumb: "rgba(255, 176, 0, 0.3)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#ffc266",
      cursor: "#ffb000",
      cursorAccent: "#120a02",
      selectionBackground: "rgba(255, 176, 0, 0.28)",
      black: "#2e1f08",
      red: "#ff5533",
      green: "#c2d84a",
      yellow: "#ffc247",
      blue: "#7fb3d8",
      magenta: "#ff8f5e",
      cyan: "#8fd8c0",
      white: "#ffdda6",
      brightBlack: "#5c4218",
      brightRed: "#ff7a5c",
      brightGreen: "#dbec7f",
      brightYellow: "#ffd97e",
      brightBlue: "#a9cfe8",
      brightMagenta: "#ffaf85",
      brightCyan: "#b3e8d6",
      brightWhite: "#fff2d9",
    },
  },
  // 蓝图：双层工程网格（细网 24px / 主网 96px），整块画面的结构感来自线而不是光。
  // 光标用竖线而非方块——制图语境里方块光标太"终端"，竖线更像绘图笔。
  {
    id: "blueprint",
    name: "Blueprint",
    group: "hacker",
    mode: "dark",
    accent: "#38bdf8",
    typography: {
      cursorStyle: "bar",
    },
    ui: {
      panelBg: "rgba(6, 22, 40, 0.86)",
      panelBorder: "rgba(56, 189, 248, 0.2)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: stack(
        "repeating-linear-gradient(0deg, rgba(125, 211, 252, 0.15) 0 1px, transparent 1px 96px)",
        "repeating-linear-gradient(90deg, rgba(125, 211, 252, 0.15) 0 1px, transparent 1px 96px)",
        "repeating-linear-gradient(0deg, rgba(125, 211, 252, 0.07) 0 1px, transparent 1px 24px)",
        "repeating-linear-gradient(90deg, rgba(125, 211, 252, 0.07) 0 1px, transparent 1px 24px)",
        "radial-gradient(118% 76% at 50% -12%, rgba(56, 189, 248, 0.13), transparent 68%)",
        "linear-gradient(180deg, #071a30 0%, #0a2038 55%, #05121f 100%)",
      ),
      screenOverlay: VIGNETTE_SOFT,
      glow: "rgba(56, 189, 248, 0.22)",
      text: "#d3e8ff",
      textMuted: "rgba(211, 232, 255, 0.58)",
      soft: "rgba(56, 189, 248, 0.12)",
      scrollbarThumb: "rgba(150, 205, 255, 0.32)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#d6eaff",
      cursor: "#38bdf8",
      cursorAccent: "#071a30",
      selectionBackground: "rgba(56, 189, 248, 0.28)",
      black: "#14395c",
      red: "#ff7a93",
      green: "#6ee7a8",
      yellow: "#ffd479",
      blue: "#6aa8ff",
      magenta: "#c4b5fd",
      cyan: "#5fe6fa",
      white: "#c5dcf5",
      brightBlack: "#345a80",
      brightRed: "#ff9db2",
      brightGreen: "#9df0c4",
      brightYellow: "#ffe3a8",
      brightBlue: "#9cc6ff",
      brightMagenta: "#dbcffe",
      brightCyan: "#9df1fb",
      brightWhite: "#eef8ff",
    },
  },
  // Zero Day：近黑 + 胶片颗粒，红光只从底缘渗上来，安静但有压迫感
  {
    id: "zero-day",
    name: "Zero Day",
    group: "hacker",
    mode: "dark",
    accent: "#ff3352",
    typography: {
      glow: "0 0 6px rgba(255, 51, 82, 0.16)",
    },
    ui: {
      panelBg: "rgba(10, 10, 14, 0.86)",
      panelBorder: "rgba(255, 51, 82, 0.16)",
      toolbarBg: "rgba(255, 255, 255, 0.03)",
      divider: "rgba(255, 255, 255, 0.06)",
      screenBg: stack(
        GRAIN,
        "radial-gradient(86% 48% at 50% 114%, rgba(255, 51, 82, 0.17), transparent 66%)",
        "radial-gradient(70% 44% at 6% -10%, rgba(124, 58, 237, 0.13), transparent 62%)",
        "linear-gradient(180deg, #06060a 0%, #0c0a10 55%, #040406 100%)",
      ),
      screenOverlay: VIGNETTE_SOFT,
      glow: "rgba(255, 51, 82, 0.18)",
      text: "#e8e6ee",
      textMuted: "rgba(232, 230, 238, 0.55)",
      soft: "rgba(255, 51, 82, 0.1)",
      scrollbarThumb: "rgba(232, 230, 238, 0.26)",
      scrollbarEdge: "transparent",
    },
    xterm: {
      background: "rgba(0, 0, 0, 0)",
      foreground: "#dcd9e4",
      cursor: "#ff3352",
      cursorAccent: "#0a0a0e",
      selectionBackground: "rgba(255, 51, 82, 0.28)",
      black: "#1a1a22",
      red: "#ff3352",
      green: "#2fd475",
      yellow: "#f5b944",
      blue: "#6c7bff",
      magenta: "#c46bff",
      cyan: "#4dd8e6",
      white: "#c9c6d4",
      brightBlack: "#4a4858",
      brightRed: "#ff5c76",
      brightGreen: "#5ee996",
      brightYellow: "#ffd076",
      brightBlue: "#93a0ff",
      brightMagenta: "#d693ff",
      brightCyan: "#7ce6f2",
      brightWhite: "#f4f2f9",
    },
  },

  // —— classics ——
  // 这一组刻意保持素面实底：它们的价值是「原汁原味的配色」，不该加场景纹理
  {
    id: "catppuccin-mocha",
    name: "Catppuccin Mocha",
    group: "classic",
    mode: "dark",
    accent: "#89b4fa",
    ui: {
      panelBg: "rgba(30, 30, 46, 0.92)",
      panelBorder: "rgba(255, 255, 255, 0.09)",
      toolbarBg: "rgba(255, 255, 255, 0.035)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: "#1e1e2e",
      screenOverlay: null,
      glow: "rgba(17, 18, 38, 0.4)",
      text: "#cdd6f4",
      textMuted: "rgba(205, 214, 244, 0.62)",
      soft: "rgba(205, 214, 244, 0.1)",
      scrollbarThumb: "rgba(205, 214, 244, 0.32)",
      scrollbarEdge: "#1e1e2e",
    },
    xterm: {
      background: "#1e1e2e",
      foreground: "#cdd6f4",
      cursor: "#f38ba8",
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
  },
  {
    id: "tokyo-night",
    name: "Tokyo Night",
    group: "classic",
    mode: "dark",
    accent: "#7aa2f7",
    ui: {
      panelBg: "rgba(26, 27, 38, 0.92)",
      panelBorder: "rgba(255, 255, 255, 0.09)",
      toolbarBg: "rgba(255, 255, 255, 0.035)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: "#1a1b26",
      screenOverlay: null,
      glow: "rgba(15, 16, 30, 0.4)",
      text: "#c0caf5",
      textMuted: "rgba(192, 202, 245, 0.6)",
      soft: "rgba(192, 202, 245, 0.1)",
      scrollbarThumb: "rgba(192, 202, 245, 0.3)",
      scrollbarEdge: "#1a1b26",
    },
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
  },
  {
    id: "dracula",
    name: "Dracula",
    group: "classic",
    mode: "dark",
    accent: "#bd93f9",
    ui: {
      panelBg: "rgba(40, 42, 54, 0.92)",
      panelBorder: "rgba(255, 255, 255, 0.09)",
      toolbarBg: "rgba(255, 255, 255, 0.04)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: "#282a36",
      screenOverlay: null,
      glow: "rgba(30, 32, 48, 0.4)",
      text: "#f8f8f2",
      textMuted: "rgba(248, 248, 242, 0.6)",
      soft: "rgba(248, 248, 242, 0.1)",
      scrollbarThumb: "rgba(248, 248, 242, 0.3)",
      scrollbarEdge: "#282a36",
    },
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
  },
  {
    id: "rose-pine",
    name: "Rosé Pine",
    group: "classic",
    mode: "dark",
    accent: "#c4a7e7",
    ui: {
      panelBg: "rgba(25, 23, 36, 0.92)",
      panelBorder: "rgba(255, 255, 255, 0.09)",
      toolbarBg: "rgba(255, 255, 255, 0.035)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: "#191724",
      screenOverlay: null,
      glow: "rgba(18, 16, 28, 0.4)",
      text: "#e0def4",
      textMuted: "rgba(224, 222, 244, 0.62)",
      soft: "rgba(224, 222, 244, 0.1)",
      scrollbarThumb: "rgba(224, 222, 244, 0.3)",
      scrollbarEdge: "#191724",
    },
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
  },
  {
    id: "vesper",
    name: "Vesper",
    group: "classic",
    mode: "dark",
    accent: "#ffc799",
    ui: {
      panelBg: "rgba(16, 16, 16, 0.93)",
      panelBorder: "rgba(255, 255, 255, 0.09)",
      toolbarBg: "rgba(255, 255, 255, 0.04)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: "#101010",
      screenOverlay: null,
      glow: "rgba(10, 10, 10, 0.4)",
      text: "#dbdbdb",
      textMuted: "rgba(219, 219, 219, 0.6)",
      soft: "rgba(255, 199, 153, 0.1)",
      scrollbarThumb: "rgba(219, 219, 219, 0.28)",
      scrollbarEdge: "#101010",
    },
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
  },
  {
    id: "poimandres",
    name: "Poimandres",
    group: "classic",
    mode: "dark",
    accent: "#5de4c7",
    ui: {
      panelBg: "rgba(27, 30, 40, 0.92)",
      panelBorder: "rgba(255, 255, 255, 0.09)",
      toolbarBg: "rgba(255, 255, 255, 0.04)",
      divider: "rgba(255, 255, 255, 0.07)",
      screenBg: "#1b1e28",
      screenOverlay: null,
      glow: "rgba(19, 21, 30, 0.4)",
      text: "#a6accd",
      textMuted: "rgba(166, 172, 205, 0.62)",
      soft: "rgba(166, 172, 205, 0.1)",
      scrollbarThumb: "rgba(166, 172, 205, 0.3)",
      scrollbarEdge: "#1b1e28",
    },
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
  },
];
