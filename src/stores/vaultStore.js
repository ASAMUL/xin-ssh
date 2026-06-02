import { defineStore } from "pinia";
import { lockStronghold, saveCredential, unlockStronghold } from "../services/vaultService";
import { archiveCredentialStore, getDeviceVaultPassword } from "../services/workspaceApi";

export const useVaultStore = defineStore("vault", {
  state: () => ({
    vaultPath: null,
    isUnlocked: false,
    isBusy: false,
    lastBackupPath: null,
  }),
  actions: {
    hydrate(vaultPath) {
      const vaultPathChanged = this.vaultPath !== vaultPath;
      this.vaultPath = vaultPath;

      if (vaultPathChanged) {
        this.isUnlocked = false;
        this.lastBackupPath = null;
      }
    },
    async unlock(unlockPassword, onProgress) {
      if (!this.vaultPath) {
        throw new Error("Vault path is not configured");
      }

      this.isBusy = true;
      try {
        await unlockStronghold(this.vaultPath, unlockPassword, onProgress);
        this.isUnlocked = true;
      } finally {
        this.isBusy = false;
      }
    },
    async ensureUnlocked(onProgress) {
      if (this.isUnlocked) return;

      onProgress?.("readDeviceVaultKey");
      const deviceVaultPassword = await getDeviceVaultPassword();
      try {
        await this.unlock(deviceVaultPassword, onProgress);
      } catch (error) {
        await lockStronghold(onProgress);
        this.isUnlocked = false;
        onProgress?.("archiveCredentialStore");
        this.lastBackupPath = await archiveCredentialStore();

        try {
          await this.unlock(deviceVaultPassword, onProgress);
        } catch (retryError) {
          throw new Error(
            `Credential store could not be opened after rebuilding: ${String(retryError)}`,
          );
        }
      }
    },
    async lock() {
      await lockStronghold();
      this.isUnlocked = false;
    },
    async saveCredential(credentialRef, secret, onProgress) {
      await saveCredential(credentialRef, secret, onProgress);
    },
  },
});
