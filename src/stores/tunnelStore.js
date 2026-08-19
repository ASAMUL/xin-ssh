import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import {
  deleteTunnelConfig,
  listTunnelConfigs,
  saveTunnelConfig,
  startTunnel,
  stopTunnel,
} from "../services/workspaceApi";
import { useCredentialStore } from "./credentialStore";
import { useProfileStore } from "./profileStore";
import { useSettingsStore } from "./settingsStore";

const TUNNEL_EVENT_NAME = "tunnel-event";

export const useTunnelStore = defineStore("tunnels", {
  state: () => ({
    configs: [],
    // tunnelId → { status: stopped | starting | running | error, error }
    runtime: {},
    unlistenTunnel: null,
    loaded: false,
  }),
  getters: {
    activeConfigs: (state) =>
      state.configs
        .filter((tunnel) => !tunnel.deletedAt)
        .sort((left, right) => left.sortOrder - right.sortOrder),
    runtimeOf: (state) => (tunnelId) =>
      state.runtime[tunnelId] ?? { status: "stopped", error: null },
  },
  actions: {
    async startListening() {
      if (this.unlistenTunnel) return;

      try {
        this.unlistenTunnel = await listen(TUNNEL_EVENT_NAME, (event) => {
          this.handleTunnelEvent(event.payload);
        });
      } catch (error) {
        console.warn("[Xin Terminal] tunnel event listener unavailable", error);
      }
    },
    handleTunnelEvent(event) {
      if (!event?.tunnelId) return;

      if (event.kind === "started") {
        this.runtime[event.tunnelId] = { status: "running", error: null };
      } else if (event.kind === "stopped") {
        this.runtime[event.tunnelId] = { status: "stopped", error: null };
      } else if (event.kind === "error") {
        this.runtime[event.tunnelId] = {
          status: "error",
          error: event.error ?? "Tunnel error",
        };
      }
    },
    async loadConfigs() {
      try {
        this.configs = await listTunnelConfigs();
        this.loaded = true;
      } catch (error) {
        console.warn("[Xin Terminal] tunnel configs unavailable", error);
      }
    },
    async saveConfig(tunnel) {
      this.configs = await saveTunnelConfig(tunnel);
    },
    async removeConfig(tunnelId) {
      await this.stop(tunnelId);
      this.configs = await deleteTunnelConfig(tunnelId);
      delete this.runtime[tunnelId];
    },
    // credentials: { password, passphrase } —— 面板在无保存凭据时收集
    async start(tunnel, credentials = {}) {
      const profileStore = useProfileStore();
      const profile = profileStore.activeProfiles.find(
        (item) => item.id === tunnel.profileId,
      );
      if (!profile) {
        this.runtime[tunnel.id] = { status: "error", error: "Connection profile missing" };
        return false;
      }

      this.runtime[tunnel.id] = { status: "starting", error: null };

      try {
        const resolved = await resolveCredentials(profile, credentials);

        await startTunnel({
          tunnelId: tunnel.id,
          host: profile.host,
          port: profile.port,
          username: profile.username,
          authMethod: profile.authMethod,
          password: resolved.password || null,
          privateKeyPath: profile.privateKeyPath || null,
          passphrase: resolved.passphrase || null,
          localPort: Number(tunnel.localPort),
          remoteHost: tunnel.remoteHost || "127.0.0.1",
          remotePort: Number(tunnel.remotePort),
          keepaliveSeconds: useSettingsStore().sshKeepaliveSeconds,
        });
        return true;
      } catch (error) {
        this.runtime[tunnel.id] = { status: "error", error: String(error) };
        return false;
      }
    },
    async stop(tunnelId) {
      const runtime = this.runtime[tunnelId];
      if (!runtime || ["stopped", "error"].includes(runtime.status)) return;

      try {
        await stopTunnel(tunnelId);
      } catch (error) {
        this.runtime[tunnelId] = { status: "error", error: String(error) };
      }
    },
  },
});

async function resolveCredentials(profile, credentials) {
  const provided = {
    password: credentials.password || null,
    passphrase: credentials.passphrase || null,
  };
  if (!profile.credentialRef) return provided;

  const credentialStore = useCredentialStore();
  if (profile.authMethod === "password" && !provided.password) {
    provided.password = await credentialStore.readCredential(profile.credentialRef, "password");
  }
  if (profile.authMethod === "privateKey" && !provided.passphrase) {
    provided.passphrase = await credentialStore.readCredential(
      profile.credentialRef,
      "passphrase",
    );
  }

  return provided;
}
