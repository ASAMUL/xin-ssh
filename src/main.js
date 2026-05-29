import "./assets/css/main.css";

import { createApp } from "vue";
import { createPinia } from "pinia";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import { i18n } from "./i18n";
import ui from "@nuxt/ui/vue-plugin";

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
