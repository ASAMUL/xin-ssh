import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import ui from '@nuxt/ui/vite'
import { DEVICON_NAMES } from "./src/config/osIcons.js";

const host = process.env.TAURI_DEV_HOST;
const require = createRequire(import.meta.url);

// devicon 全集 5MB，整包 addCollection 会让首包翻几倍；这里只切出系统图标目录里
// 用到的那几十个，作为 virtual:os-icons 交给 main.js 注册（Tauri 离线，不能走 Iconify API）。
function osIconCollection() {
  const virtualId = "virtual:os-icons";
  const resolvedId = `\0${virtualId}`;

  return {
    name: "xin-os-icon-collection",
    resolveId: (id) => (id === virtualId ? resolvedId : null),
    load(id) {
      if (id !== resolvedId) return null;
      const collection = JSON.parse(readFileSync(require.resolve("@iconify-json/devicon/icons.json"), "utf8"));
      const icons = Object.fromEntries(DEVICON_NAMES.map((name) => {
        const icon = collection.icons[name];
        if (!icon) throw new Error(`devicon 中没有图标 "${name}"，请检查 src/config/osIcons.js`);
        return [name, icon];
      }));
      const { prefix, width, height } = collection;
      return `export default ${JSON.stringify({ prefix, width, height, icons })};`;
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue(),
  ui({
    ui: {
      colors: {
        // 墨樱主题：sakura 色阶定义在 main.css 的 @theme。中性灰用 zinc（接近 Apple 的 systemGray，略偏冷），
        // 不用带紫调的 mauve——Liquid Glass 的颜色应该来自玻璃背后的内容，灰阶自己带色会和桌面壁纸混成脏紫
        primary: "sakura",
        success: "green",
        neutral: "zinc",
      },
      // 所有浮在内容上的弹出层统一换成液态玻璃材质（类定义在 assets/css/liquid-glass.css，
      // 未分层规则会压过这些 slot 默认的 bg-default / ring / shadow 工具类）
      dropdownMenu: { slots: { content: "liquid-menu" } },
      contextMenu: { slots: { content: "liquid-menu" } },
      popover: { slots: { content: "liquid-menu" } },
      select: { slots: { content: "liquid-menu" } },
      selectMenu: { slots: { content: "liquid-menu" } },
      tooltip: { slots: { content: "liquid-tooltip" } },
      toast: { slots: { root: "liquid-menu" } },
      modal: { slots: { content: "liquid-sheet", overlay: "liquid-scrim" } },
    },
  }),
  osIconCollection(),
  ],

  // main.js 用 addCollection 往 @iconify/vue 里注册整套离线图标，Nuxt UI 的 UIcon 又从它自己解析到的那份取图标。
  // 两边版本一错开（升 Nuxt UI 时就发生过），pnpm 会装出两份、图标仓库各自独立：dev 下预构建按包名合成一份看不出来，
  // 正式包里 UIcon 找不到图标就去请求 Iconify API，又被 CSP 拦下，图标整片空白。这里强制全项目只用根目录那一份
  resolve: {
    dedupe: ["@iconify/vue"],
  },

  // 不再用 manualChunks 手动拆包：按包名硬拆会让 ui / vendor 两个 chunk 互相 import，
  // 入口先加载 vendor 时 ui 抢在 vendor 初始化前执行，触发 TDZ 报错整页白屏（0.1.1 就是这么挂的）。
  // Rollup 自动分包能保证执行顺序；Tauri 从本地磁盘加载，单个大 chunk 没有网络成本，拆了也是启动时全量预加载。
  build: {
    chunkSizeWarningLimit: 5000,
    target: 'es2021',
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
        protocol: "ws",
        host,
        port: 1421,
      }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
