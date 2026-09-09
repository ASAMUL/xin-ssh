import { defineStore } from "pinia";
import { toAppError } from "../utils/appError";

export const useUiStore = defineStore("ui", {
  state: () => ({
    isConnectionFormOpen: false,
    editingProfileId: null,
    isSettingsOpen: false,
    isTunnelPanelOpen: false,
    error: null,
    // 首页导航：home | all | favorites | recents；groupFilter 为分组 id 或 null
    homeNav: "home",
    homeGroupFilter: null,
  }),
  actions: {
    setHomeNav(nav) {
      this.homeNav = nav;
      this.homeGroupFilter = null;
    },
    setHomeGroupFilter(groupId) {
      this.homeNav = "all";
      this.homeGroupFilter = groupId;
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
