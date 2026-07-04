import "./assets/css/main.css";

import { createApp } from "vue";
import { createPinia } from "pinia";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import { i18n } from "./i18n";
import ui from "@nuxt/ui/vue-plugin";

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
