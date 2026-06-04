import { defineStore } from "pinia";
import { saveSettings } from "../services/workspaceApi";
import { i18n } from "../i18n";

const MONITOR_REFRESH_SECONDS_OPTIONS = [2, 5, 10, 30];
const DEFAULT_MONITOR_REFRESH_SECONDS = 5;

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    settings: null,
  }),
  getters: {
    locale: (state) => state.settings?.locale ?? "zh-CN",
    monitorRefreshSeconds: (state) =>
      normalizeMonitorRefreshSeconds(state.settings?.monitorRefreshSeconds),
  },
  actions: {
    hydrate(settings) {
      this.settings = {
        monitorRefreshSeconds: DEFAULT_MONITOR_REFRESH_SECONDS,
        ...settings,
      };
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
    async setMonitorRefreshSeconds(seconds) {
      if (!this.settings) return null;

      const monitorRefreshSeconds = normalizeMonitorRefreshSeconds(seconds);
      const nextSettings = {
        ...this.settings,
        monitorRefreshSeconds,
      };
      const snapshot = await saveSettings(nextSettings);
      this.hydrate(snapshot.settings);
      return snapshot;
    },
  },
});

function normalizeMonitorRefreshSeconds(value) {
  const seconds = Number(value);
  if (MONITOR_REFRESH_SECONDS_OPTIONS.includes(seconds)) {
    return seconds;
  }

  return DEFAULT_MONITOR_REFRESH_SECONDS;
}
