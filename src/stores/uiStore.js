import { defineStore } from "pinia";

export const useUiStore = defineStore("ui", {
  state: () => ({
    isConnectionFormOpen: false,
    editingProfileId: null,
    isSettingsOpen: false,
    error: null,
  }),
  actions: {
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
    setError(error) {
      this.error = error ? String(error) : null;
    },
    clearError() {
      this.error = null;
    },
  },
});
