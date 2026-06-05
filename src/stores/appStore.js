import { defineStore } from "pinia";
import { getAppStatus, initWorkspace, loadWorkspace } from "../services/workspaceApi";
import { useProfileStore } from "./profileStore";
import { useSettingsStore } from "./settingsStore";

export const useAppStore = defineStore("app", {
  state: () => ({
    isLoading: true,
    initialized: false,
    workspaceDir: null,
    error: null,
  }),
  actions: {
    async bootstrap() {
      this.isLoading = true;
      this.error = null;
      try {
        const status = await getAppStatus();
        if (!status.initialized) {
          const snapshot = await initWorkspace(null);
          this.consumeSnapshot(snapshot);
          return;
        }

        const snapshot = await loadWorkspace();
        if (snapshot) {
          this.consumeSnapshot(snapshot);
        }
      } catch (error) {
        this.error = String(error);
      } finally {
        this.isLoading = false;
      }
    },
    consumeSnapshot(snapshot) {
      this.initialized = true;
      this.workspaceDir = snapshot.workspaceDir;
      useSettingsStore().hydrate(snapshot.settings);
      useProfileStore().hydrate(snapshot);
    },
  },
});
