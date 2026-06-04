import { defineStore } from "pinia";
import {
  closeTerminalSession,
  getRemoteSystemInfo,
  resizeTerminal,
  sendTerminalInput,
  startTerminalSession,
} from "../services/workspaceApi";
import { i18n } from "../i18n";
import { useVaultStore } from "./vaultStore";

const INITIAL_TERMINAL_COLUMNS = 120;
const INITIAL_TERMINAL_ROWS = 32;
const NETWORK_HISTORY_LIMIT = 28;

export const useSessionStore = defineStore("sessions", {
  state: () => ({
    sessions: [],
    activeSessionId: null,
  }),
  getters: {
    activeSession: (state) =>
      state.sessions.find((session) => session.id === state.activeSessionId) ?? null,
  },
  actions: {
    openProfileSession(profile) {
      const existingSession = this.sessions.find((session) => session.profileId === profile.id);
      if (existingSession) {
        syncSessionFromProfile(existingSession, profile);
        this.activeSessionId = existingSession.id;
        if (shouldAutoConnectWithSavedCredential(existingSession)) {
          this.startSessionWithSavedCredential(existingSession.id);
        }
        return existingSession;
      }

      const session = {
        id: crypto.randomUUID(),
        profileId: profile.id,
        name: profile.name,
        host: profile.host,
        port: profile.port,
        username: profile.username,
        authMethod: profile.authMethod,
        privateKeyPath: profile.privateKeyPath,
        credentialRef: profile.credentialRef ?? null,
        remotePath: profile.defaultRemotePath || "/",
        status: "authRequired",
        error: null,
        savedCredentialAutoStarted: false,
        createdAt: Date.now(),
        terminalOutput: "",
        terminalColumns: INITIAL_TERMINAL_COLUMNS,
        terminalRows: INITIAL_TERMINAL_ROWS,
        systemInfo: null,
        systemInfoError: null,
        systemInfoLoading: false,
        systemInfoUpdatedAt: null,
        networkHistory: [],
      };

      this.sessions.push(session);
      this.activeSessionId = session.id;
      if (shouldAutoConnectWithSavedCredential(session)) {
        this.startSessionWithSavedCredential(session.id);
      }
      return session;
    },
    selectSession(sessionId) {
      if (this.sessions.some((session) => session.id === sessionId)) {
        this.activeSessionId = sessionId;
      }
    },
    async closeSession(sessionId) {
      const closingIndex = this.sessions.findIndex((session) => session.id === sessionId);
      if (closingIndex < 0) return;

      await closeTerminalSession(sessionId).catch(() => {});
      this.sessions.splice(closingIndex, 1);
      if (this.activeSessionId !== sessionId) return;

      const nextSession = this.sessions[closingIndex] ?? this.sessions[closingIndex - 1] ?? null;
      this.activeSessionId = nextSession?.id ?? null;
    },
    clearActiveTerminal() {
      if (!this.activeSession) return;

      this.activeSession.terminalOutput = "";
    },
    requestCredentials(sessionId = this.activeSessionId) {
      const session = this.findSession(sessionId);
      if (!session) return;

      session.status = "authRequired";
      session.error = null;
    },
    async startActiveTerminal(credentials = {}) {
      await this.startTerminalForSession(this.activeSessionId, credentials);
    },
    async startTerminalForSession(sessionId, credentials = {}, options = {}) {
      const session = this.findSession(sessionId);
      if (!session || session.status === "connecting") return;

      session.status = "connecting";
      session.error = null;
      session.terminalOutput = "";

      try {
        await startTerminalSession({
          sessionId: session.id,
          host: session.host,
          port: session.port,
          username: session.username,
          authMethod: session.authMethod,
          password: credentials.password || null,
          privateKeyPath: session.privateKeyPath || null,
          passphrase: credentials.passphrase || null,
          defaultRemotePath: session.remotePath,
          columns: session.terminalColumns,
          rows: session.terminalRows,
        });
      } catch (error) {
        const latestSession = this.findSession(session.id);
        if (latestSession) {
          latestSession.status = options.fallbackStatus ?? "authRequired";
          latestSession.error = String(error);
        }
      }
    },
    async startSessionWithSavedCredential(sessionId = this.activeSessionId) {
      const session = this.findSession(sessionId);
      if (!session?.credentialRef || session.status === "connected" || session.status === "connecting") {
        return false;
      }

      session.status = "connecting";
      session.error = null;
      session.terminalOutput = "";
      session.savedCredentialAutoStarted = true;

      try {
        const vaultStore = useVaultStore();
        await vaultStore.ensureUnlocked();
        const credentials = await readSavedCredentials(vaultStore, session);

        if (session.authMethod === "password" && !credentials.password) {
          throw new Error(i18n.global.t("terminal.savedCredentialMissing"));
        }

        await startTerminalSession({
          sessionId: session.id,
          host: session.host,
          port: session.port,
          username: session.username,
          authMethod: session.authMethod,
          password: credentials.password || null,
          privateKeyPath: session.privateKeyPath || null,
          passphrase: credentials.passphrase || null,
          defaultRemotePath: session.remotePath,
          columns: session.terminalColumns,
          rows: session.terminalRows,
        });
        return true;
      } catch (error) {
        const latestSession = this.findSession(session.id);
        if (latestSession) {
          latestSession.status = "authRequired";
          latestSession.error = String(error);
        }
        return false;
      }
    },
    reconnectActiveTerminal() {
      const session = this.activeSession;
      if (!session) return;

      if (session.credentialRef) {
        this.startSessionWithSavedCredential(session.id);
        return;
      }

      this.requestCredentials(session.id);
    },
    async sendActiveTerminalInput(data) {
      const session = this.activeSession;
      if (!session || session.status !== "connected" || !data) return;

      await sendTerminalInput({
        sessionId: session.id,
        data,
      });
    },
    async resizeActiveTerminal(columns, rows) {
      const session = this.activeSession;
      if (!session) return;

      session.terminalColumns = columns;
      session.terminalRows = rows;
      if (session.status !== "connected") return;

      await resizeTerminal({
        sessionId: session.id,
        columns,
        rows,
      }).catch(() => {});
    },
    updateSessionRemotePath(sessionId, remotePath) {
      const session = this.findSession(sessionId);
      if (!session || !remotePath) return;

      session.remotePath = remotePath;
    },
    handleTerminalEvent(event) {
      const session = this.findSession(event.sessionId);
      if (!session) return;

      if (event.kind === "output") {
        session.terminalOutput += event.data ?? "";
        return;
      }

      if (event.kind === "connected") {
        session.status = "connected";
        session.error = null;
        return;
      }

      if (event.kind === "connecting") {
        session.status = "connecting";
        session.error = null;
        return;
      }

      if (event.kind === "closed") {
        session.status = "closed";
        return;
      }

      if (event.kind === "error") {
        session.status = "error";
        session.error = event.error ?? event.data ?? "Terminal error";
      }
    },
    async refreshSystemInfo(sessionId = this.activeSessionId) {
      const session = this.findSession(sessionId);
      if (!session || session.status !== "connected" || session.systemInfoLoading) return null;

      ensureSystemInfoState(session);
      session.systemInfoLoading = true;
      session.systemInfoError = null;

      try {
        const snapshot = await getRemoteSystemInfo({ sessionId: session.id });
        const latestSession = this.findSession(session.id);
        if (!latestSession) return null;

        latestSession.systemInfo = normalizeSystemInfoSnapshot(snapshot);
        latestSession.systemInfoUpdatedAt = Date.now();
        appendNetworkSample(latestSession, latestSession.systemInfo);
        return latestSession.systemInfo;
      } catch (error) {
        const latestSession = this.findSession(session.id);
        if (latestSession) {
          latestSession.systemInfoError = String(error);
        }
        return null;
      } finally {
        const latestSession = this.findSession(session.id);
        if (latestSession) {
          latestSession.systemInfoLoading = false;
        }
      }
    },
    findSession(sessionId) {
      return this.sessions.find((session) => session.id === sessionId) ?? null;
    },
  },
});

function syncSessionFromProfile(session, profile) {
  session.name = profile.name;
  session.host = profile.host;
  session.port = profile.port;
  session.username = profile.username;
  session.authMethod = profile.authMethod;
  session.privateKeyPath = profile.privateKeyPath;
  session.credentialRef = profile.credentialRef ?? null;
  session.remotePath = session.remotePath || profile.defaultRemotePath || "/";
}

function shouldAutoConnectWithSavedCredential(session) {
  return (
    Boolean(session?.credentialRef) &&
    !session.savedCredentialAutoStarted &&
    ["authRequired", "closed", "error"].includes(session.status)
  );
}

async function readSavedCredentials(vaultStore, session) {
  if (session.authMethod === "password") {
    return {
      password: await vaultStore.readCredential(session.credentialRef, "password"),
      passphrase: null,
    };
  }

  return {
    password: null,
    passphrase: await vaultStore.readCredential(session.credentialRef, "passphrase"),
  };
}

function normalizeSystemInfoSnapshot(snapshot) {
  return {
    ...snapshot,
    loadAverage: snapshot?.loadAverage || [],
    networks: snapshot?.networks || [],
    processes: snapshot?.processes || [],
    disks: snapshot?.disks || [],
  };
}

function appendNetworkSample(session, snapshot) {
  ensureSystemInfoState(session);
  const capturedAt = Number(snapshot?.capturedAt) || Date.now();
  const totalBytes = (snapshot?.networks || []).reduce(
    (total, network) => total + Number(network.rxBytes || 0) + Number(network.txBytes || 0),
    0,
  );
  const previousSample = session.networkHistory.at(-1);
  const secondsElapsed = previousSample
    ? Math.max((capturedAt - previousSample.capturedAt) / 1000, 0.001)
    : 0;
  const bytesDelta = previousSample ? Math.max(totalBytes - previousSample.totalBytes, 0) : 0;
  const bytesPerSecond = secondsElapsed > 0 ? bytesDelta / secondsElapsed : 0;

  session.networkHistory = [
    ...session.networkHistory,
    {
      id: `${capturedAt}-${session.networkHistory.length}`,
      capturedAt,
      totalBytes,
      bytesPerSecond,
    },
  ].slice(-NETWORK_HISTORY_LIMIT);
}

function ensureSystemInfoState(session) {
  session.systemInfo ??= null;
  session.systemInfoError ??= null;
  session.systemInfoLoading ??= false;
  session.systemInfoUpdatedAt ??= null;
  session.networkHistory ??= [];
}
