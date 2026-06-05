import { defineStore } from "pinia";
import {
  readCredential,
  removeCredential,
  saveCredential,
} from "../services/workspaceApi";

export const useCredentialStore = defineStore("credentials", {
  actions: {
    saveCredential(credentialRef, secret) {
      return saveCredential({
        credentialRef,
        secret,
      });
    },
    readCredential(credentialRef, kind) {
      return readCredential({
        credentialRef,
        kind,
      });
    },
    removeCredential(credentialRef, kinds) {
      return removeCredential({
        credentialRef,
        kinds,
      });
    },
  },
});
