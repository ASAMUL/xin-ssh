import { defineStore } from "pinia";
import {
  closeTerminalSession,
  getRemoteSystemInfo,
  resizeTerminal,
  sendTerminalInput,
  startTerminalSession,
} from "../services/workspaceApi";
import { i18n } from "../i18n";
import { useCredentialStore } from "./credentialStore";

const INITIAL_TERMINAL_COLUMNS = 120;
const INITIAL_TERMINAL_ROWS = 32;
const NETWORK_HISTORY_LIMIT = 30;
const MAX_AUTO_RECONNECT_ATTEMPTS = 1;
const STABLE_CONNECTION_RESET_MS = 30_000;
const TERMINAL_CLOSE_REASON_CLIENT = "client";

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
        currentAttemptId: null,
        autoReconnectAttempts: 0,
        connectedAt: null,
        lastDisconnectReason: null,
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
    showConnectionHub() {
      this.activeSessionId = null;
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
      await this.startTerminalForSession(this.activeSessionId, credentials, {
        resetReconnectAttempts: true,
      });
    },
    async startTerminalForSession(sessionId, credentials = {}, options = {}) {
      const session = this.findSession(sessionId);
      if (!session || session.status === "connecting") return;

      const attemptId = createTerminalAttemptId(session.id);
      session.status = "connecting";
      session.error = null;
      session.terminalOutput = "";
      session.currentAttemptId = attemptId;
      session.connectedAt = null;
      session.lastDisconnectReason = null;
      if (options.resetReconnectAttempts !== false) {
        session.autoReconnectAttempts = 0;
      }

      try {
        await startTerminalSession({
          sessionId: session.id,
          attemptId,
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
        if (latestSession && latestSession.currentAttemptId === attemptId) {
          latestSession.status = options.fallbackStatus ?? "authRequired";
          latestSession.error = String(error);
          latestSession.lastDisconnectReason = "error";
        }
      }
    },
    async startSessionWithSavedCredential(sessionId = this.activeSessionId, options = {}) {
      const session = this.findSession(sessionId);
      if (
        !session?.credentialRef ||
        session.status === "connecting" ||
        (session.status === "connected" && !options.force)
      ) {
        return false;
      }

      const attemptId = createTerminalAttemptId(session.id);
      session.status = "connecting";
      session.error = null;
      session.terminalOutput = "";
      session.savedCredentialAutoStarted = true;
      session.currentAttemptId = attemptId;
      session.connectedAt = null;
      session.lastDisconnectReason = null;
      if (options.resetReconnectAttempts !== false) {
        session.autoReconnectAttempts = 0;
      }

      try {
        const credentialStore = useCredentialStore();
        const credentials = await readSavedCredentials(credentialStore, session);

        if (session.authMethod === "password" && !credentials.password) {
          throw new Error(i18n.global.t("terminal.savedCredentialMissing"));
        }

        await startTerminalSession({
          sessionId: session.id,
          attemptId,
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
        if (latestSession && latestSession.currentAttemptId === attemptId) {
          latestSession.status = options.fallbackStatus ?? "authRequired";
          latestSession.error = String(error);
          latestSession.lastDisconnectReason = "error";
        }
        return false;
      }
    },
    reconnectActiveTerminal() {
      return this.reconnectSession(this.activeSessionId);
    },
    reconnectSession(sessionId = this.activeSessionId) {
      const session = this.findSession(sessionId);
      if (!session) return;

      session.autoReconnectAttempts = 0;
      if (session.credentialRef) {
        return this.startSessionWithSavedCredential(session.id, {
          force: true,
          resetReconnectAttempts: true,
        });
      }

      this.requestCredentials(session.id);
      return false;
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
      if (isStaleTerminalEvent(session, event)) return;

      if (event.kind === "output") {
        session.terminalOutput += event.data ?? "";
        return;
      }

      if (event.kind === "connected") {
        session.status = "connected";
        session.error = null;
        session.connectedAt = Date.now();
        session.lastDisconnectReason = null;
        return;
      }

      if (event.kind === "connecting") {
        session.status = "connecting";
        session.error = null;
        session.lastDisconnectReason = null;
        return;
      }

      if (event.kind === "closed") {
        const reason = event.reason ?? "remote";
        session.status = "closed";
        session.lastDisconnectReason = reason;
        this.maybeAutoReconnect(session, reason);
        return;
      }

      if (event.kind === "error") {
        const wasConnected = session.status === "connected";
        session.status = "error";
        session.error = event.error ?? event.data ?? "Terminal error";
        session.lastDisconnectReason = event.reason ?? "error";
        if (wasConnected) {
          this.maybeAutoReconnect(session, session.lastDisconnectReason);
        }
      }
    },
    handleConnectionOperationError(sessionId, error) {
      const message = String(error);
      if (!isConnectionLostError(message)) return false;

      const session = this.findSession(sessionId);
      if (!session) return false;

      session.status = "closed";
      session.error = message;
      session.lastDisconnectReason = "remote";
      return this.maybeAutoReconnect(session, "remote");
    },
    maybeAutoReconnect(session, reason = "remote") {
      if (!session || reason === TERMINAL_CLOSE_REASON_CLIENT || !session.credentialRef) {
        return false;
      }

      if (session.connectedAt && Date.now() - session.connectedAt >= STABLE_CONNECTION_RESET_MS) {
        session.autoReconnectAttempts = 0;
      }

      if (session.autoReconnectAttempts >= MAX_AUTO_RECONNECT_ATTEMPTS) {
        return false;
      }

      session.autoReconnectAttempts += 1;
      this.startSessionWithSavedCredential(session.id, {
        force: true,
        fallbackStatus: "error",
        resetReconnectAttempts: false,
      });
      return true;
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
          const message = String(error);
          latestSession.systemInfoError = message;
          this.handleConnectionOperationError(latestSession.id, message);
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

function createTerminalAttemptId(sessionId) {
  return `${sessionId}-${Date.now()}-${crypto.randomUUID()}`;
}

function isStaleTerminalEvent(session, event) {
  return Boolean(
    event.attemptId &&
      session.currentAttemptId &&
      event.attemptId !== session.currentAttemptId,
  );
}

function isConnectionLostError(error) {
  const message = String(error || "").toLowerCase();
  return [
    "terminal session is not running",
    "terminal session is not accepting",
    "request was canceled",
    "connection closed",
    "connection reset",
    "connection aborted",
    "broken pipe",
    "lost connection",
    "disconnected",
    "disconnect",
    "eof",
  ].some((signature) => message.includes(signature));
}

async function readSavedCredentials(credentialStore, session) {
  if (session.authMethod === "password") {
    return {
      password: await credentialStore.readCredential(session.credentialRef, "password"),
      passphrase: null,
    };
  }

  return {
    password: null,
    passphrase: await credentialStore.readCredential(session.credentialRef, "passphrase"),
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
  const rxBytes = sumNetworkBytes(snapshot, "rxBytes");
  const txBytes = sumNetworkBytes(snapshot, "txBytes");
  const totalBytes = rxBytes + txBytes;
  const previousSample = session.networkHistory.at(-1);
  const secondsElapsed = previousSample
    ? Math.max((capturedAt - previousSample.capturedAt) / 1000, 0.001)
    : 0;
  const bytesDelta = previousSample ? Math.max(totalBytes - previousSample.totalBytes, 0) : 0;
  const rxBytesDelta = previousSample ? Math.max(rxBytes - Number(previousSample.rxBytes || 0), 0) : 0;
  const txBytesDelta = previousSample ? Math.max(txBytes - Number(previousSample.txBytes || 0), 0) : 0;
  const bytesPerSecond = secondsElapsed > 0 ? bytesDelta / secondsElapsed : 0;
  const rxBytesPerSecond = secondsElapsed > 0 ? rxBytesDelta / secondsElapsed : 0;
  const txBytesPerSecond = secondsElapsed > 0 ? txBytesDelta / secondsElapsed : 0;

  session.networkHistory = [
    ...session.networkHistory,
    {
      id: `${capturedAt}-${session.networkHistory.length}`,
      capturedAt,
      rxBytes,
      txBytes,
      totalBytes,
      bytesPerSecond,
      rxBytesPerSecond,
      txBytesPerSecond,
    },
  ].slice(-NETWORK_HISTORY_LIMIT);
}

function sumNetworkBytes(snapshot, field) {
  return (snapshot?.networks || []).reduce(
    (total, network) => total + Number(network[field] || 0),
    0,
  );
}

function ensureSystemInfoState(session) {
  session.systemInfo ??= null;
  session.systemInfoError ??= null;
  session.systemInfoLoading ??= false;
  session.systemInfoUpdatedAt ??= null;
  session.networkHistory ??= [];
}
