import { defineStore } from "pinia";
import { saveSettings } from "../services/workspaceApi";
import { normalizeTerminalBackground } from "../services/backgroundApi";
import { applyAppearanceMode, normalizeAppearanceMode } from "../utils/appearanceMode";
import { applyGlassStyle, normalizeGlassStyle } from "../utils/glassStyle";
import { i18n } from "../i18n";
import { createSerialQueue } from "../utils/serialQueue";
import { useUiStore } from "./uiStore";
import {
  TERMINAL_FONTS,
  TERMINAL_FONT_SIZES,
  normalizeTerminalAppearance,
  normalizeTerminalMaterial,
  normalizeTerminalThemeId,
} from "../config/terminalThemes";

const MONITOR_REFRESH_SECONDS_OPTIONS = [2, 5, 10, 30];
const DEFAULT_MONITOR_REFRESH_SECONDS = 5;
// 0 = 禁用 keepalive
export const SSH_KEEPALIVE_SECONDS_OPTIONS = [0, 15, 30, 60, 120];
const DEFAULT_SSH_KEEPALIVE_SECONDS = 30;
const DEFAULT_TERMINAL_FONT_SIZE = 14;
const DEFAULT_MONITOR_PANE_SIZE = 22;
const DEFAULT_FILE_PANE_SIZE = 42;
const DEFAULT_FILE_TREE_SIZE = 22;
const enqueueSettingsWrite = createSerialQueue();

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    settings: null,
    workspaceDir: null,
  }),
  getters: {
    locale: (state) => state.settings?.locale ?? "zh-CN",
    monitorRefreshSeconds: (state) =>
      normalizeMonitorRefreshSeconds(state.settings?.monitorRefreshSeconds),
    sshKeepaliveSeconds: (state) =>
      normalizeSshKeepaliveSeconds(state.settings?.sshKeepaliveSeconds),
    // 老存档里可能还是已下线的主题 id，读的时候折算成现在的家族
    terminalTheme: (state) => normalizeTerminalThemeId(state.settings?.terminalTheme),
    terminalAppearance: (state) => normalizeTerminalAppearance(state.settings?.terminalAppearance),
    terminalMaterial: (state) => normalizeTerminalMaterial(state.settings?.terminalMaterial),
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
    terminalBackground: (state) => normalizeTerminalBackground(state.settings?.terminalBackground),
    appearanceMode: (state) => normalizeAppearanceMode(state.settings?.appearanceMode),
    glassStyle: (state) => normalizeGlassStyle(state.settings?.glassStyle),
    // 应用壁纸和终端背景图同一个数据形状（file / opacity / blur / fit），校验也复用
    appWallpaper: (state) => normalizeTerminalBackground(state.settings?.appWallpaper),
    sessionRestore: (state) => normalizeSessionRestore(state.settings?.sessionRestore),
  },
  actions: {
    hydrate(settings, workspaceDir = this.workspaceDir) {
      this.workspaceDir = workspaceDir;
      this.settings = {
        monitorRefreshSeconds: DEFAULT_MONITOR_REFRESH_SECONDS,
        ...settings,
      };
      if (settings?.locale) {
        i18n.global.locale.value = settings.locale;
      }
      // 明暗模式挂在 <html> 上，不属于 Vue 的响应式范围，得在这里显式落一次；
      // 启动时 main.js 已按系统偏好画过，这里是拿到用户存的档后校准。
      applyAppearanceMode(this.appearanceMode);
      applyGlassStyle(this.glassStyle);
    },
    previewSettings(partial) {
      if (!this.settings) {
        this.hydrate(partial);
        return;
      }
      Object.assign(this.settings, partial);
      if (partial.locale) i18n.global.locale.value = partial.locale;
      // 滑块预览很频繁，只有模式变化才需要同步原生窗口主题。
      if (Object.hasOwn(partial, "appearanceMode")) applyAppearanceMode(this.appearanceMode);
      if (Object.hasOwn(partial, "glassStyle")) applyGlassStyle(this.glassStyle);
    },
    // 内存立即采用最新输入；响应只确认落盘，不能拿旧响应覆盖之后的预览或修改。
    async patchSettings(partial, workspaceDir = this.workspaceDir) {
      if (workspaceDir !== this.workspaceDir) {
        throw { code: "workspace.changed", message: "Workspace changed before saving" };
      }
      const patch = { ...partial };
      if (Object.hasOwn(patch, "terminalBackground")) {
        patch.terminalBackground = normalizeTerminalBackground(patch.terminalBackground);
      }
      if (Object.hasOwn(patch, "appWallpaper")) {
        patch.appWallpaper = normalizeTerminalBackground(patch.appWallpaper);
      }
      this.previewSettings(patch);
      if (!workspaceDir) return null;

      return enqueueSettingsWrite(() => saveSettings(patch, workspaceDir));
    },
    // 布局/外观类偏好：先改内存立即生效，持久化失败不回滚（下次启动回落到旧值）
    applySettingsPatch(partial) {
      return this.patchSettings(partial).catch((error) => {
        console.warn("[Xin SSH] settings patch failed", error);
        useUiStore().setError(error);
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
    setSshKeepaliveSeconds(seconds) {
      return this.patchSettings({
        sshKeepaliveSeconds: normalizeSshKeepaliveSeconds(seconds),
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
    // 全局终端背景图；单个连接可在外观面板里覆盖成自己的一张
    setTerminalBackground(terminalBackground, workspaceDir = this.workspaceDir) {
      return this.patchSettings({ terminalBackground }, workspaceDir);
    },
    // 先落 DOM 再持久化：切换要立刻可见，存盘失败也不该把界面卡在旧模式
    setAppearanceMode(appearanceMode) {
      const mode = normalizeAppearanceMode(appearanceMode);
      return this.applySettingsPatch({ appearanceMode: mode });
    },
    setGlassStyle(glassStyle) {
      return this.applySettingsPatch({ glassStyle: normalizeGlassStyle(glassStyle) });
    },
    // null = 移除壁纸，窗口回到近白 / 近黑的画布
    setAppWallpaper(appWallpaper) {
      return this.patchSettings({ appWallpaper });
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

function normalizeSshKeepaliveSeconds(value) {
  const seconds = Number(value);
  if (SSH_KEEPALIVE_SECONDS_OPTIONS.includes(seconds)) {
    return seconds;
  }

  return DEFAULT_SSH_KEEPALIVE_SECONDS;
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

function normalizeSessionRestore(value) {
  if (!value || !Array.isArray(value.sessions)) {
    return { sessions: [], activeIndex: null, activeProfileId: null, activeLabelIndex: null };
  }
  const sessions = value.sessions
    .filter((entry) => entry && typeof entry.profileId === "string" && entry.profileId.trim())
    .map((entry) => ({
      profileId: entry.profileId,
      remotePath: typeof entry.remotePath === "string" ? entry.remotePath : null,
      labelIndex: Number.isInteger(entry.labelIndex) && entry.labelIndex > 0 ? entry.labelIndex : null,
    }));
  const activeIndex = Number.isInteger(value.activeIndex) && value.activeIndex >= 0
    && value.activeIndex < sessions.length ? value.activeIndex : null;
  return {
    sessions,
    activeIndex,
    activeProfileId: typeof value.activeProfileId === "string" ? value.activeProfileId : null,
    activeLabelIndex: Number.isInteger(value.activeLabelIndex) && value.activeLabelIndex > 0
      ? value.activeLabelIndex : null,
  };
}
