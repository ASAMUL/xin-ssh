import { defineStore } from "pinia";
import { saveSettings } from "../services/workspaceApi";
import { i18n } from "../i18n";

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    settings: null,
  }),
  getters: {
    locale: (state) => state.settings?.locale ?? "zh-CN",
  },
  actions: {
    hydrate(settings) {
      this.settings = settings;
      if (settings?.locale) {
        i18n.global.locale.value = settings.locale;
      }
    },
    async setLocale(locale) {
      if (!this.settings) return null;

      const nextSettings = {
        ...this.settings,
        locale,
      };
      const snapshot = await saveSettings(nextSettings);
      this.hydrate(snapshot.settings);
      return snapshot;
    },
  },
});
