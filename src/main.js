import "@fontsource-variable/inter";
import "@fontsource-variable/noto-sans-sc";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "@fontsource/fira-code/400.css";
import "@fontsource/fira-code/700.css";
import "./assets/css/main.css";

import { createApp } from "vue";
import { createPinia } from "pinia";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import { i18n } from "./i18n";
import ui from "@nuxt/ui/vue-plugin";

// 运行在 Tauri 里时窗口是透明的（Mica/Acrylic 玻璃），CSS 据此切换到半透明底色；
// 浏览器里跑 dev 时保持不透明底色。
if (window.__TAURI_INTERNALS__) {
  document.documentElement.classList.add("native-glass");
}

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
app.mount("#app");
