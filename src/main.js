import "@fontsource-variable/inter";
import "@fontsource-variable/noto-sans-sc";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "@fontsource/fira-code/400.css";
import "@fontsource/fira-code/700.css";
import "@fontsource/victor-mono/400.css";
import "@fontsource/victor-mono/400-italic.css";
import "@fontsource/victor-mono/700.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/700.css";
import "@fontsource/geist-mono/400.css";
import "@fontsource/geist-mono/700.css";
import "@fontsource/space-mono/400.css";
import "@fontsource/space-mono/700.css";
import "./assets/css/main.css";

import { addCollection } from "@iconify/vue";
import lucideIcons from "@iconify-json/lucide/icons.json";
import osIcons from "virtual:os-icons";
import { createApp } from "vue";
import { createPinia } from "pinia";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import { i18n } from "./i18n";
import { applyAppearanceMode } from "./utils/appearanceMode";
import {
  autoAttachLiquidGlass,
  trackLiquidGlassPointer,
  vLiquidGlass,
  vLiquidStretch,
} from "./directives/liquidGlass";
import ui from "@nuxt/ui/vue-plugin";

addCollection(lucideIcons);
// 连接的系统图标（devicon 彩色子集），由 vite.config.js 按 config/osIcons.js 裁剪生成
addCollection(osIcons);

// 运行在 Tauri 里时窗口是透明的（Mica/Acrylic 玻璃），CSS 据此切换到半透明底色；
// 浏览器里跑 dev 时保持不透明底色。
if (window.__TAURI_INTERNALS__) {
  document.documentElement.classList.add("native-glass");
}

// 先按系统偏好上色再挂载：settings.json 是异步读的，不抢在挂载前定调的话，
// 深色用户每次启动都会先闪一下白。拿到存档后 settingsStore.hydrate 会再校准一次。
applyAppearanceMode("system");

// Nuxt UI 的 Vue 插件依赖 vue-router（ULink 等组件需要注入的 router）。
// 应用本身不做页面路由，这里只挂一个空路由满足该依赖，请勿移除。
const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      component: {
        render: () => null,
      },
    },
  ],
});

const app = createApp(App);

app.use(createPinia());
app.use(router);
app.use(i18n);
app.use(ui);
// 液态玻璃的高光带与折射层，配合 .liquid-glass 材质类使用（见 assets/css/liquid-glass.css）
app.directive("liquid-glass", vLiquidGlass);
// 分段切换里的选中水滴换位时拉伸回弹
app.directive("liquid-stretch", vLiquidStretch);
app.mount("#app");

trackLiquidGlassPointer();

// 弹出层才是真正展示玻璃的地方：它们浮在卡片、终端、文件列表上，边缘的弯折最明显。
// 菜单要读字，滤镜里的模糊比控件胶囊重；对话框面积大、离内容远，折射和模糊都再加一档。
autoAttachLiquidGlass([
  { selector: ".liquid-menu", options: { scale: 22, blur: 8 } },
  // 远程编辑器：正文是不透明的代码区，整块折射看不见；打字、光标闪烁、背后终端刷新都会让整块滤镜重算，
  // 只留高光线和 CSS 模糊。规则按顺序取第一条匹配的，所以写在通用的 .liquid-sheet 前面
  { selector: ".liquid-sheet.remote-editor-modal", options: { refraction: false } },
  { selector: ".liquid-sheet", options: { scale: 32, blur: 12 } },
]);
