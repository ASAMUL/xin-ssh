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

  // 把体积大的三方库拆出主 chunk，避免 ~1MB 单文件（xterm / UI 库 / 其余依赖各一份）
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes("node_modules")) return undefined;
          if (id.includes("@xterm")) return "xterm";
          if (id.includes("@nuxt/ui") || id.includes("reka-ui") || id.includes("tailwind")) {
            return "ui";
          }
          return "vendor";
        },
      },
    },
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
