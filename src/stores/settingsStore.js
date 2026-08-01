import { defineStore } from "pinia";
import { saveSettings } from "../services/workspaceApi";
import { i18n } from "../i18n";
import { DEFAULT_TERMINAL_THEME_ID, TERMINAL_FONTS, TERMINAL_FONT_SIZES } from "../config/terminalThemes";

const MONITOR_REFRESH_SECONDS_OPTIONS = [2, 5, 10, 30];
const DEFAULT_MONITOR_REFRESH_SECONDS = 5;
const DEFAULT_TERMINAL_FONT_SIZE = 14;
const DEFAULT_MONITOR_PANE_SIZE = 22;
const DEFAULT_FILE_PANE_SIZE = 42;
const DEFAULT_FILE_TREE_SIZE = 22;

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
    showMonitorSidebar: (state) => state.settings?.showMonitorSidebar ?? true,
    showFileExplorer: (state) => state.settings?.showFileExplorer ?? true,
    showFileTree: (state) => state.settings?.showFileTree ?? true,
    showHiddenFiles: (state) => state.settings?.showHiddenFiles ?? true,
    monitorPaneSize: (state) =>
      normalizePaneSize(state.settings?.monitorPaneSize, DEFAULT_MONITOR_PANE_SIZE),
    filePaneSize: (state) =>
      normalizePaneSize(state.settings?.filePaneSize, DEFAULT_FILE_PANE_SIZE),
    fileTreeSize: (state) =>
      normalizePaneSize(state.settings?.fileTreeSize, DEFAULT_FILE_TREE_SIZE),
    homeViewMode: (state) =>
      state.settings?.homeViewMode === "list" ? "list" : "grid",
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
    // 布局/外观类偏好：先改内存立即生效，持久化失败不回滚（下次启动回落到旧值）
    applySettingsPatch(partial) {
      if (this.settings) {
        Object.assign(this.settings, partial);
      }

      return this.patchSettings(partial).catch((error) => {
        console.warn("[Xin Terminal] settings patch failed", error);
        return null;
      });
    },
    toggleMonitorSidebar() {
      return this.applySettingsPatch({ showMonitorSidebar: !this.showMonitorSidebar });
    },
    toggleFileExplorer() {
      return this.applySettingsPatch({ showFileExplorer: !this.showFileExplorer });
    },
    toggleFileTree() {
      return this.applySettingsPatch({ showFileTree: !this.showFileTree });
    },
    toggleHiddenFiles() {
      return this.applySettingsPatch({ showHiddenFiles: !this.showHiddenFiles });
    },
    setHomeViewMode(homeViewMode) {
      return this.applySettingsPatch({
        homeViewMode: homeViewMode === "list" ? "list" : "grid",
      });
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

function normalizePaneSize(value, fallback) {
  const size = Number(value);
  if (!Number.isFinite(size) || size <= 0 || size >= 100) {
    return fallback;
  }

  return size;
}
