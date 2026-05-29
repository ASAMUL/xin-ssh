import { defineStore } from "pinia";

export const useUiStore = defineStore("ui", {
  state: () => ({
    isConnectionFormOpen: false,
    isSettingsOpen: false,
    error: null,
  }),
  actions: {
    openConnectionForm() {
      this.isConnectionFormOpen = true;
    },
    closeConnectionForm() {
      this.isConnectionFormOpen = false;
    },
    openSettings() {
      this.isSettingsOpen = true;
    },
    closeSettings() {
      this.isSettingsOpen = false;
    },
    setError(error) {
      this.error = error ? String(error) : null;
    },
    clearError() {
      this.error = null;
    },
  },
});
