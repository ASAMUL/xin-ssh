import './assets/css/main.css'

import { createApp } from "vue";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
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

app.use(router);
app.use(ui);
app.mount("#app");
