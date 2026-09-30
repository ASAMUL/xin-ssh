import { defineStore } from "pinia";
import { toAppError } from "../utils/appError";

export const useUiStore = defineStore("ui", {
  state: () => ({
    isConnectionFormOpen: false,
    editingProfileId: null,
    isSettingsOpen: false,
    isTunnelPanelOpen: false,
    isWindowCloseOpen: false,
    // 会话标签拖动中：标签条在工作区里，快捷键挂在常驻顶栏上，两边靠这个标志对齐
    isDraggingSessionTab: false,
    // 沉浸：工作区只剩终端（收起概览、文件区、标签条和状态栏）。会话级状态，不落盘——
    // 下次打开应用还是完整工作区，免得用户以为面板丢了
    isWorkspaceImmersive: false,
    error: null,
    // 首页筛选：all | favorites | recents，与分组筛选（分组 id 或 null）叠加生效
    homeNav: "all",
    homeGroupFilter: null,
  }),
  actions: {
    requestWindowClose() {
      this.isWindowCloseOpen = true;
    },
    cancelWindowClose() {
      this.isWindowCloseOpen = false;
    },
    setDraggingSessionTab(dragging) {
      this.isDraggingSessionTab = Boolean(dragging);
    },
    toggleWorkspaceImmersive(immersive = !this.isWorkspaceImmersive) {
      this.isWorkspaceImmersive = Boolean(immersive);
    },
    setHomeNav(nav) {
      this.homeNav = ["all", "favorites", "recents"].includes(nav) ? nav : "all";
    },
    setHomeGroupFilter(groupId) {
      this.homeGroupFilter = groupId || null;
    },
    openConnectionForm(profileId = null) {
      this.editingProfileId = profileId;
      this.isConnectionFormOpen = true;
    },
    closeConnectionForm() {
      this.isConnectionFormOpen = false;
      this.editingProfileId = null;
    },
    openSettings() {
      this.isSettingsOpen = true;
    },
    closeSettings() {
      this.isSettingsOpen = false;
    },
    openTunnelPanel() {
      this.isTunnelPanelOpen = true;
    },
    closeTunnelPanel() {
      this.isTunnelPanelOpen = false;
    },
    setError(error) {
      this.error = error ? toAppError(error) : null;
    },
    clearError() {
      this.error = null;
    },
  },
});
