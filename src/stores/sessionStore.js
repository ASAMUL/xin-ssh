import { defineStore } from "pinia";
import {
  closeTerminalSession,
  resizeTerminal,
  sendTerminalInput,
  startTerminalSession,
} from "../services/workspaceApi";

const INITIAL_TERMINAL_COLUMNS = 120;
const INITIAL_TERMINAL_ROWS = 32;

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
        this.activeSessionId = existingSession.id;
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
        remotePath: profile.defaultRemotePath || "/",
        status: "authRequired",
        error: null,
        createdAt: Date.now(),
        terminalOutput: "",
        terminalColumns: INITIAL_TERMINAL_COLUMNS,
        terminalRows: INITIAL_TERMINAL_ROWS,
      };

      this.sessions.push(session);
      this.activeSessionId = session.id;
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
      const session = this.activeSession;
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
        session.status = "authRequired";
        session.error = String(error);
      }
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
    findSession(sessionId) {
      return this.sessions.find((session) => session.id === sessionId) ?? null;
    },
  },
});
