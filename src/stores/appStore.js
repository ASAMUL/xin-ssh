import { defineStore } from "pinia";
import { getAppStatus, initWorkspace, loadWorkspace } from "../services/workspaceApi";
import { localizeError } from "../utils/appError";
import { useProfileStore } from "./profileStore";
import { useSettingsStore } from "./settingsStore";
import { useSessionStore } from "./sessionStore";

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
          useSessionStore().restorePersistedSessions();
          return;
        }

        const snapshot = await loadWorkspace();
        if (snapshot) {
          this.consumeSnapshot(snapshot);
          useSessionStore().restorePersistedSessions();
        }
      } catch (error) {
        this.error = localizeError(error);
      } finally {
        this.isLoading = false;
      }
    },
    consumeSnapshot(snapshot) {
      this.initialized = true;
      this.workspaceDir = snapshot.workspaceDir;
      useSettingsStore().hydrate(snapshot.settings, snapshot.workspaceDir);
      useProfileStore().hydrate(snapshot);
    },
  },
});
