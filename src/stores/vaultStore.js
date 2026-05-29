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
      this.vaultPath = vaultPath;
      this.isUnlocked = false;
      this.lastBackupPath = null;
    },
    async unlock(unlockPassword) {
      if (!this.vaultPath) {
        throw new Error("Vault path is not configured");
      }

      this.isBusy = true;
      try {
        await unlockStronghold(this.vaultPath, unlockPassword);
        this.isUnlocked = true;
      } finally {
        this.isBusy = false;
      }
    },
    async ensureUnlocked() {
      if (this.isUnlocked) return;

      const deviceVaultPassword = await getDeviceVaultPassword();
      try {
        await this.unlock(deviceVaultPassword);
      } catch (error) {
        await lockStronghold();
        this.isUnlocked = false;
        this.lastBackupPath = await archiveCredentialStore();

        try {
          await this.unlock(deviceVaultPassword);
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
    async saveCredential(credentialRef, secret) {
      await saveCredential(credentialRef, secret);
    },
  },
});
