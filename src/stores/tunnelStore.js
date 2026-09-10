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
import { i18n } from "../i18n";
import { localizeError } from "../utils/appError";
import { createSerialQueue } from "../utils/serialQueue";
import { sameConnection } from "../utils/connectionIdentity";
import { normalizeAgentConfig, sameAgentConfig } from "../utils/sshAgent";

const TUNNEL_EVENT_NAME = "tunnel-event";
const enqueueTunnelConfigChange = createSerialQueue();
const startRequests = new Map();
const stopRequests = new Map();
const ACTIVE_STATUSES = new Set(["starting", "running", "stopping"]);
let listeningPromise = null;

export const useTunnelStore = defineStore("tunnels", {
  state: () => ({
    configs: [],
    // attemptId 隔离每次启停，旧事件不能改变重启后的状态。
    runtime: {},
    busyConfigs: {},
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
      if (!listeningPromise) {
        listeningPromise = listen(TUNNEL_EVENT_NAME, (event) => {
          this.handleTunnelEvent(event.payload);
        }).then((unlisten) => { this.unlistenTunnel = unlisten; })
          .finally(() => { listeningPromise = null; });
      }
      return listeningPromise;
    },
    handleTunnelEvent(event) {
      const current = this.runtime[event?.tunnelId];
      if (!current || current.attemptId !== event.attemptId || !ACTIVE_STATUSES.has(current.status)) return;

      if (event.kind === "started") {
        current.hasStarted = true;
        if (current.status !== "stopping") {
          current.status = "running";
          current.error = null;
        }
      } else if (event.kind === "stopped") {
        current.status = "stopped";
        current.error = null;
      } else if (event.kind === "error") {
        current.status = "error";
        current.error = localizeError(event.error) || i18n.global.t("tunnel.genericError");
      }
    },
    async loadConfigs() {
      try {
        await enqueueTunnelConfigChange(async () => {
          this.configs = await listTunnelConfigs();
          this.loaded = true;
        });
      } catch (error) {
        console.warn("[Xin Terminal] tunnel configs unavailable", error);
      }
    },
    async saveConfig(tunnel) {
      if (!this.canEditConfig(tunnel.id)) {
        throw { code: "tunnel.configBusy", message: "Stop the tunnel before editing" };
      }
      const next = { ...tunnel };
      this.busyConfigs[tunnel.id] = true;
      try {
        await enqueueTunnelConfigChange(async () => {
          this.configs = await saveTunnelConfig(next);
        });
      } finally {
        delete this.busyConfigs[tunnel.id];
      }
    },
    async removeConfig(tunnelId) {
      if (this.busyConfigs[tunnelId]) return;
      this.busyConfigs[tunnelId] = true;
      try {
        // 停止失败必须保留配置，不能把仍占用端口的运行实例从面板中删掉。
        await this.stop(tunnelId);
        await enqueueTunnelConfigChange(async () => {
          this.configs = await deleteTunnelConfig(tunnelId);
          delete this.runtime[tunnelId];
        });
      } finally {
        delete this.busyConfigs[tunnelId];
      }
    },
    canEditConfig(tunnelId) {
      return !this.busyConfigs[tunnelId] && !ACTIVE_STATUSES.has(this.runtime[tunnelId]?.status);
    },
    // 先记录意图再读取凭据，等待期间停止或配置变更都不能启动旧请求。
    async start(tunnel, credentials = {}) {
      if (!this.canEditConfig(tunnel.id)) return false;
      const config = this.activeConfigs.find((item) => item.id === tunnel.id);
      if (!config) return false;
      tunnel = { ...config };
      const profileStore = useProfileStore();
      const sourceProfile = profileStore.activeProfiles.find(
        (item) => item.id === tunnel.profileId,
      );
      if (!sourceProfile) {
        this.runtime[tunnel.id] = {
          status: "error",
          error: i18n.global.t("tunnel.profileMissing"),
        };
        return false;
      }

      const profile = { ...sourceProfile, agent: normalizeAgentConfig(sourceProfile.agent) };
      const workspaceDir = useSettingsStore().workspaceDir;
      const attemptId = crypto.randomUUID();
      const provided = { ...credentials };
      this.runtime[tunnel.id] = {
        status: "starting", error: null, attemptId, hasStarted: false, submitted: false,
      };

      try {
        await this.startListening();
        if (!this.isCurrentAttempt(tunnel.id, attemptId, "starting")) return false;
        const resolved = await resolveCredentials(profile, provided);
        if (!this.isCurrentAttempt(tunnel.id, attemptId, "starting")) return false;
        if (workspaceDir !== useSettingsStore().workspaceDir) {
          throw { code: "workspace.changed", message: "Workspace changed before tunnel startup" };
        }
        const currentProfile = profileStore.activeProfiles.find((item) => item.id === tunnel.profileId);
        if (!sameConnection(profile, currentProfile) || profile.authMethod !== currentProfile.authMethod
          || profile.privateKeyPath !== currentProfile.privateKeyPath
          || !sameAgentConfig(profile.agent, currentProfile.agent)
          || profile.credentialRef !== currentProfile.credentialRef) {
          throw { code: "tunnel.connectionChanged", message: "Connection changed before tunnel startup" };
        }
        this.runtime[tunnel.id].submitted = true;
        const submitted = startTunnel({
          tunnelId: tunnel.id,
          attemptId,
          host: profile.host,
          port: profile.port,
          username: profile.username,
          authMethod: profile.authMethod,
          agent: profile.authMethod === "agent" ? profile.agent : null,
          password: resolved.password || null,
          privateKeyPath: profile.privateKeyPath || null,
          passphrase: resolved.passphrase || null,
          localPort: Number(tunnel.localPort),
          remoteHost: tunnel.remoteHost || "127.0.0.1",
          remotePort: Number(tunnel.remotePort),
          keepaliveSeconds: useSettingsStore().sshKeepaliveSeconds,
        });
        startRequests.set(attemptId, submitted);
        await submitted;
        return this.isCurrentAttempt(tunnel.id, attemptId)
          && ["starting", "running"].includes(this.runtime[tunnel.id].status);
      } catch (error) {
        if (this.isCurrentAttempt(tunnel.id, attemptId, "starting")) {
          this.runtime[tunnel.id].status = "error";
          this.runtime[tunnel.id].error = localizeError(error);
        }
        return false;
      } finally {
        startRequests.delete(attemptId);
      }
    },
    isCurrentAttempt(tunnelId, attemptId, status) {
      const current = this.runtime[tunnelId];
      return current?.attemptId === attemptId && (!status || current.status === status);
    },
    // 停止等待启动登记确认；没有提交的凭据准备直接取消，避免 stop 抢在 start 前到达后端。
    stop(tunnelId) {
      const current = this.runtime[tunnelId];
      if (!current) return Promise.resolve();
      const { attemptId } = current;
      if (stopRequests.has(attemptId)) return stopRequests.get(attemptId);
      if (!ACTIVE_STATUSES.has(current.status)) return Promise.resolve();
      const submitted = startRequests.get(attemptId);
      const preparing = !current.submitted;
      current.status = "stopping";
      current.error = null;
      const stopped = this.stopAttempt(tunnelId, attemptId, submitted, preparing)
        .finally(() => { stopRequests.delete(attemptId); });
      stopRequests.set(attemptId, stopped);
      return stopped;
    },
    async stopAttempt(tunnelId, attemptId, submitted, preparing) {
      try {
        let registered = !preparing;
        if (submitted) {
          try { await submitted; } catch { registered = false; }
        }
        if (registered) await stopTunnel(tunnelId, attemptId);
        if (this.isCurrentAttempt(tunnelId, attemptId, "stopping")) {
          this.runtime[tunnelId].status = "stopped";
        }
      } catch (error) {
        if (this.isCurrentAttempt(tunnelId, attemptId, "stopping")) {
          const current = this.runtime[tunnelId];
          // 请求失败不证明隧道已经停止，保留活动状态以便重试，也继续参与关窗保护。
          current.status = current.hasStarted ? "running" : "starting";
          current.error = localizeError(error);
        }
        throw error;
      }
    },
  },
});

async function resolveCredentials(profile, credentials) {
  if (profile.authMethod === "agent") return {};
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
