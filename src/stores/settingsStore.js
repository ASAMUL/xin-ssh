import { defineStore } from "pinia";
import { saveSettings } from "../services/workspaceApi";
import { i18n } from "../i18n";
import { DEFAULT_TERMINAL_THEME_ID, TERMINAL_FONTS, TERMINAL_FONT_SIZES } from "../config/terminalThemes";

const MONITOR_REFRESH_SECONDS_OPTIONS = [2, 5, 10, 30];
const DEFAULT_MONITOR_REFRESH_SECONDS = 5;
const DEFAULT_TERMINAL_FONT_SIZE = 14;

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    settings: null,
  }),
  getters: {
    locale: (state) => state.settings?.locale ?? "zh-CN",
    monitorRefreshSeconds: (state) =>
      normalizeMonitorRefreshSeconds(state.settings?.monitorRefreshSeconds),
    terminalTheme: (state) => state.settings?.terminalTheme ?? DEFAULT_TERMINAL_THEME_ID,
    terminalFontFamily: (state) =>
      normalizeTerminalFontFamily(state.settings?.terminalFontFamily),
    terminalFontSize: (state) => normalizeTerminalFontSize(state.settings?.terminalFontSize),
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
    async patchSettings(partial) {
      if (!this.settings) {
        // no backend snapshot yet (e.g. browser dev): keep for this session only
        this.hydrate(partial);
        return null;
      }

      const nextSettings = {
        ...this.settings,
        ...partial,
      };
      const snapshot = await saveSettings(nextSettings);
      this.hydrate(snapshot.settings);
      return snapshot;
    },
    setLocale(locale) {
      return this.patchSettings({ locale });
    },
    setMonitorRefreshSeconds(seconds) {
      return this.patchSettings({
        monitorRefreshSeconds: normalizeMonitorRefreshSeconds(seconds),
      });
    },
    setTerminalTheme(terminalTheme) {
      return this.patchSettings({ terminalTheme });
    },
    setTerminalFontFamily(terminalFontFamily) {
      return this.patchSettings({
        terminalFontFamily: normalizeTerminalFontFamily(terminalFontFamily),
      });
    },
    setTerminalFontSize(terminalFontSize) {
      return this.patchSettings({
        terminalFontSize: normalizeTerminalFontSize(terminalFontSize),
      });
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

function normalizeTerminalFontFamily(value) {
  const font = TERMINAL_FONTS.find((item) => item.name === value);
  return (font ?? TERMINAL_FONTS[0]).name;
}

function normalizeTerminalFontSize(value) {
  const size = Number(value);
  if (TERMINAL_FONT_SIZES.includes(size)) {
    return size;
  }

  return DEFAULT_TERMINAL_FONT_SIZE;
}
