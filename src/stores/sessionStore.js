import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import {
  closeTerminalSession,
  getRemoteSystemInfo,
  resizeTerminal,
  sendTerminalInput,
  startTerminalSession,
  trustHostKey,
} from "../services/workspaceApi";
import { i18n } from "../i18n";
import { connectionIdentity, sameConnection } from "../utils/connectionIdentity";
import { normalizeAgentConfig } from "../utils/sshAgent";
import { usesSavedCredentials } from "../utils/authentication";
import { useAuthenticationStore } from "./authenticationStore";
import {
  clearSessionCredentials,
  confirmSessionCredentials,
  discardPendingCredentials,
  readSessionCredentials,
  sameCredentialScope,
  sessionCredentialScope,
  stageSessionCredentials,
} from "../services/sessionCredentials";
import {
  CONNECTION_LOST_CODES,
  ERROR_CODES,
  hasErrorCode,
  localizeError,
  toAppError,
} from "../utils/appError";
import { useCredentialStore } from "./credentialStore";
import { useProfileStore } from "./profileStore";
import { useSettingsStore } from "./settingsStore";
import { useUiStore } from "./uiStore";
import { useFileExplorerStore } from "./fileExplorerStore";

const TERMINAL_EVENT_NAME = "terminal-event";
const INITIAL_TERMINAL_COLUMNS = 120;
const INITIAL_TERMINAL_ROWS = 32;
const NETWORK_HISTORY_LIMIT = 30;
const MAX_AUTO_RECONNECT_ATTEMPTS = 1;
const STABLE_CONNECTION_RESET_MS = 30_000;
const TERMINAL_CLOSE_REASON_CLIENT = "client";
// 挂载前缓冲按字符数设上限；超限裁剪到低水位，避免频繁触发。
const TERMINAL_BUFFER_MAX_CHARS = 2_000_000;
const TERMINAL_BUFFER_TRIM_TO_CHARS = 1_500_000;

// 仅缓存组件挂载前的输出；挂载后的完整终端状态由每个会话自己的 xterm 持有。
const terminalBuffers = new Map();
// 隐藏会话也持续解析输出，切标签时不再重放被截断的 ANSI 字符流。
const terminalSinks = new Map();
// 关闭必须等已提交的启动请求完成登记，避免 IPC 乱序留下无标签的 SSH 连接。
const terminalStartRequests = new Map();
const terminalCloseRequests = new Map();
let terminalListening = null;

export const useSessionStore = defineStore("sessions", {
  state: () => ({
    sessions: [],
    activeSessionId: null,
    unlistenTerminal: null,
    hasRestoredSessions: false,
    isRestoringSessions: false,
  }),
  getters: {
    activeSession: (state) =>
      state.sessions.find((session) => session.id === state.activeSessionId) ?? null,
    // profileId → 聚合状态（同一连接多个终端时取“最好”的一个），给首页/快速连接的状态点用
    statusByProfile: (state) => {
      const priority = { connected: 4, connecting: 3, error: 2, authRequired: 1, closed: 1 };
      const statuses = {};
      for (const session of state.sessions) {
        const current = statuses[session.profileId];
        if (!current || (priority[session.status] ?? 0) > (priority[current] ?? 0)) {
          statuses[session.profileId] = session.status;
        }
      }
      return statuses;
    },
  },
  actions: {
    async startListening() {
      if (this.unlistenTerminal) return;
      // App 挂载与快速连接可能同时请求监听，只安装一次，并把失败交给调用方展示。
      if (!terminalListening) {
        terminalListening = listen(TERMINAL_EVENT_NAME, (event) => {
          this.handleTerminalEvent(event.payload);
        }).then((unlisten) => { this.unlistenTerminal = unlisten; });
      }
      const pending = terminalListening;
      try {
        await pending;
      } finally {
        if (terminalListening === pending) terminalListening = null;
      }
    },
    // 同步接管挂载前的数据，旧组件延迟卸载时不能摘掉同一会话的新接收器。
    attachTerminalSink(sessionId, revision, write) {
      if (this.findSession(sessionId)?.terminalRevision !== revision) return () => {};
      const sink = { revision, write };
      terminalSinks.set(sessionId, sink);
      const pending = readTerminalBuffer(sessionId);
      if (pending) write(pending);
      clearTerminalBuffer(sessionId);
      return () => {
        if (terminalSinks.get(sessionId) === sink) terminalSinks.delete(sessionId);
      };
    },
    // forceNew: true 跳过复用总是新开终端；恢复时关闭 autoStart，避免没有可复用秘密的连接弹出认证窗口。
    openProfileSession(profile, {
      forceNew = false,
      autoStart = true,
      remotePath = null,
      labelIndex = null,
    } = {}) {
      if (!forceNew) {
        const existingSession = this.sessions.find((session) => session.profileId === profile.id
          && session.workspaceDir === useSettingsStore().workspaceDir && !session.isClosing);
        if (existingSession) {
          syncSessionFromProfile(existingSession, profile);
          this.activeSessionId = existingSession.id;
          if (autoStart && shouldAutoConnectSession(existingSession)) {
            this.startSessionAutomatically(existingSession.id);
          }
          this.persistSessionLayout();
          return existingSession;
        }
      }

      const preferredLabelIndex = Number.isInteger(labelIndex) && labelIndex > 0 ? labelIndex : null;
      const usedLabels = new Set(this.sessions
        .filter((session) => session.profileId === profile.id)
        .map((session) => session.labelIndex ?? 1));
      const resolvedLabelIndex = preferredLabelIndex && !usedLabels.has(preferredLabelIndex)
        ? preferredLabelIndex : nextSessionLabelIndex(this.sessions, profile.id);
      const session = {
        id: crypto.randomUUID(),
        profileId: profile.id,
        workspaceDir: useSettingsStore().workspaceDir,
        // 同一连接第 2+ 个终端显示 "name (2)"，编号取最小可用值
        labelIndex: resolvedLabelIndex,
        name: sessionDisplayName(profile.name, resolvedLabelIndex),
        host: profile.host,
        port: profile.port,
        username: profile.username,
        authMethod: profile.authMethod,
        privateKeyPath: profile.privateKeyPath,
        agent: normalizeAgentConfig(profile.agent),
        credentialRef: profile.credentialRef ?? null,
        remotePath: typeof remotePath === "string" && remotePath.trim()
          ? remotePath : (profile.defaultRemotePath || "/"),
        status: "authRequired",
        isClosing: false,
        error: null,
        hostKeyChange: null,
        autoStartAttempted: false,
        currentAttemptId: null,
        autoReconnectAttempts: 0,
        connectedAt: null,
        connectionTarget: null,
        lastDisconnectReason: null,
        createdAt: Date.now(),
        terminalRevision: 0,
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
      if (!this.isRestoringSessions && autoStart) this.persistSessionLayout();
      if (autoStart && shouldAutoConnectSession(session)) {
        this.startSessionAutomatically(session.id);
      }
      return session;
    },
    // 工作区只保存可重新解析的连接引用；恢复后生成新的运行时 sessionId，避免复用旧事件和文件状态。
    restorePersistedSessions() {
      if (this.hasRestoredSessions || !useSettingsStore().settings) return false;
      this.hasRestoredSessions = true;
      const restore = useSettingsStore().sessionRestore;
      const profiles = new Map(useProfileStore().activeProfiles.map((profile) => [profile.id, profile]));
      const restored = [];
      this.isRestoringSessions = true;
      try {
        for (const entry of restore.sessions) {
          const profile = profiles.get(entry.profileId);
          if (!profile) continue;
          restored.push(this.openProfileSession(profile, {
            forceNew: true,
            autoStart: false,
            remotePath: entry.remotePath,
            labelIndex: entry.labelIndex,
          }));
        }
      } finally {
        this.isRestoringSessions = false;
      }
      const active = restore.activeProfileId
        ? restored.find((session) => session.profileId === restore.activeProfileId
          && (restore.activeLabelIndex == null || session.labelIndex === restore.activeLabelIndex))
        : restore.activeIndex == null ? null : restored[restore.activeIndex];
      this.activeSessionId = active?.id ?? restored[0]?.id ?? null;
      for (const session of restored) {
        if (canAutoStartRestoredSession(session)) this.startSessionAutomatically(session.id);
      }
      this.persistSessionLayout();
      return restored.length > 0;
    },
    selectSession(sessionId) {
      if (this.sessions.some((session) => session.id === sessionId && !session.isClosing)) {
        this.activeSessionId = sessionId;
        this.persistSessionLayout();
      }
    },
    // 首页不占数字编号；从首页向后/向前分别进入第一个/最后一个可用会话。
    selectAdjacentSession(offset) {
      const sessions = this.sessions.filter((session) => !session.isClosing);
      if (!sessions.length || ![-1, 1].includes(offset)) return;
      const index = sessions.findIndex((session) => session.id === this.activeSessionId);
      const nextIndex = index < 0
        ? (offset > 0 ? 0 : sessions.length - 1)
        : (index + offset + sessions.length) % sessions.length;
      this.selectSession(sessions[nextIndex].id);
    },
    selectSessionByNumber(number) {
      if (!Number.isInteger(number) || number < 1 || number > 9) return;
      const sessions = this.sessions.filter((session) => !session.isClosing);
      const session = number === 9 ? sessions.at(-1) : sessions[number - 1];
      if (session) this.selectSession(session.id);
    },
    // 只移动已有对象，保持 sessionId、revision 及其 xterm 实例不变；拒绝过期的拖动目标。
    moveSession(sessionId, beforeId = null) {
      if (sessionId === beforeId || this.sessions.some((session) => session.isClosing)) return;
      const index = this.sessions.findIndex((session) => session.id === sessionId);
      if (index < 0 || (beforeId !== null && !this.findSession(beforeId))) return;
      const reordered = [...this.sessions];
      const [session] = reordered.splice(index, 1);
      const target = beforeId === null ? reordered.length
        : reordered.findIndex((session) => session.id === beforeId);
      reordered.splice(target, 0, session);
      this.sessions = reordered;
      this.persistSessionLayout();
    },
    showConnectionHub() {
      this.activeSessionId = null;
      this.persistSessionLayout();
    },
    // 同一标签的并发关闭共用结果，删除配置必须等后端关闭成功才能继续。
    async closeSession(sessionId, { requireDisconnect = false } = {}) {
      const session = this.findSession(sessionId);
      if (!session) return;
      let closing = terminalCloseRequests.get(sessionId);
      if (!closing) {
        session.isClosing = true;
        useAuthenticationStore().cancelOwner(`terminal:${sessionId}`);
        closing = (async () => {
          await terminalStartRequests.get(sessionId)?.catch(() => {});
          await closeTerminalSession(sessionId);
          const closingIndex = this.sessions.findIndex((session) => session.id === sessionId);
          clearSessionCredentials(sessionId);
          clearTerminalBuffer(sessionId);
          terminalSinks.delete(sessionId);
          useFileExplorerStore().removeSessionState(sessionId);
          if (closingIndex < 0) return;
          this.sessions.splice(closingIndex, 1);
          if (this.activeSessionId === sessionId) {
            const nextSession = this.sessions.slice(closingIndex).find((session) => !session.isClosing)
              ?? this.sessions.slice(0, closingIndex).reverse().find((session) => !session.isClosing);
            this.activeSessionId = nextSession?.id ?? null;
          }
          this.persistSessionLayout();
        })();
        terminalCloseRequests.set(sessionId, closing);
      }
      try {
        await closing;
      } catch (error) {
        session.isClosing = false;
        discardPendingCredentials(sessionId, session.currentAttemptId);
        session.status = "error";
        session.error = localizeError(error);
        if (requireDisconnect) throw error;
        useUiStore().setError(localizeError(error));
      } finally {
        if (terminalCloseRequests.get(sessionId) === closing) terminalCloseRequests.delete(sessionId);
      }
    },
    // 删除连接配置时，它已打开的终端一并关掉，不留下指向已删配置的孤儿 tab
    async closeSessionsForProfile(profileId) {
      const sessionIds = this.sessions
        .filter((session) => session.profileId === profileId)
        .map((session) => session.id);

      for (const sessionId of sessionIds) {
        await this.closeSession(sessionId, { requireDisconnect: true });
      }
      return sessionIds.length;
    },
    sessionCountForProfile(profileId) {
      return this.sessions.filter((session) => session.profileId === profileId).length;
    },
    clearActiveTerminal() {
      const session = this.activeSession;
      if (!session) return;

      resetTerminalBuffer(session);
    },
    requestCredentials(sessionId = this.activeSessionId) {
      const session = this.findSession(sessionId);
      if (!session || session.isClosing || ["connecting", "connected"].includes(session.status)) return;

      session.status = "authRequired";
      session.error = null;
    },
    async startActiveTerminal(credentials = {}) {
      return this.startTerminalForSession(this.activeSessionId, credentials, {
        resetReconnectAttempts: true,
      });
    },
    // 输入、已保存凭据和重连共用尝试管理，所有异步边界都核对原会话与目标。
    async startTerminalForSession(sessionId, credentials = {}, options = {}) {
      const session = this.findSession(sessionId);
      if (!session || session.isClosing || session.status === "connecting"
        || (session.status === "connected" && !options.force)) return false;

      const attemptId = createTerminalAttemptId(session.id);
      const credentialSource = options.credentialSource ?? "provided";
      discardPendingCredentials(sessionId, session.currentAttemptId);
      session.status = "connecting";
      session.error = null;
      session.hostKeyChange = null;
      session.currentAttemptId = attemptId;
      useFileExplorerStore().invalidateSessionState(sessionId, attemptId);
      session.connectedAt = null;
      session.lastDisconnectReason = null;
      session.systemInfo = null;
      session.systemInfoLoading = false;
      session.systemInfoError = null;
      session.systemInfoUpdatedAt = null;
      session.networkHistory = [];
      if (options.resetReconnectAttempts !== false) {
        session.autoReconnectAttempts = 0;
      }
      let startRequest;
      let authentication;
      try {
        const profile = currentSessionProfile(session);
        const previousScope = sessionCredentialScope(session);
        syncSessionFromProfile(session, profile);
        const scope = sessionCredentialScope(session);
        if (!sameCredentialScope(previousScope, scope)) throw sessionConnectionChanged();
        const credentialRef = session.credentialRef;
        const provided = { password: credentials.password, passphrase: credentials.passphrase };
        await this.startListening();
        if (session.isClosing || session.status !== "connecting"
          || this.findSession(sessionId)?.currentAttemptId !== attemptId) return false;
        const resolved = await resolveSessionCredentials(session, provided, credentialSource);
        const latestSession = this.findSession(sessionId);
        if (!latestSession || latestSession.isClosing || latestSession.status !== "connecting"
          || latestSession.currentAttemptId !== attemptId) return false;
        const latestProfile = currentSessionProfile(latestSession);
        const latestScope = sessionCredentialScope({
          ...latestProfile, profileId: latestProfile.id, workspaceDir: latestSession.workspaceDir,
        });
        if (!sameCredentialScope(scope, latestScope)
          || !sameCredentialScope(scope, sessionCredentialScope(latestSession))
          || credentialRef !== (latestProfile.credentialRef ?? null)) {
          syncSessionFromProfile(latestSession, latestProfile);
          throw sessionConnectionChanged();
        }
        if (!resolved) {
          latestSession.status = "authRequired";
          return false;
        }
        resetTerminalBuffer(session);
        // 配置可以在终端存活时被修改，传输必须记录本次真正发给后端的连接地址。
        session.connectionTarget = connectionIdentity(scope);
        stageSessionCredentials(sessionId, attemptId, scope, resolved);
        authentication = useAuthenticationStore().createFlow(`terminal:${sessionId}`, {
          ...session.connectionTarget, name: session.name, purpose: "terminal",
        });
        startRequest = startTerminalSession({
          sessionId: session.id,
          attemptId,
          ...session.connectionTarget,
          authMethod: session.authMethod,
          agent: session.authMethod === "agent" ? normalizeAgentConfig(session.agent) : null,
          password: resolved.password || null,
          privateKeyPath: session.privateKeyPath || null,
          passphrase: resolved.passphrase || null,
          defaultRemotePath: session.remotePath,
          columns: session.terminalColumns,
          rows: session.terminalRows,
          keepaliveSeconds: useSettingsStore().sshKeepaliveSeconds,
        }, authentication.channel);
        terminalStartRequests.set(sessionId, startRequest);
        await startRequest;
        return true;
      } catch (error) {
        authentication?.close();
        discardPendingCredentials(sessionId, attemptId);
        const latestSession = this.findSession(session.id);
        if (latestSession && !latestSession.isClosing && latestSession.currentAttemptId === attemptId
          && latestSession.status === "connecting") {
          const requiresCredentials = hasErrorCode(error, [ERROR_CODES.authFailed, ERROR_CODES.authKeyFile,
            ERROR_CODES.sessionConnectionChanged]);
          if (requiresCredentials) clearSessionCredentials(sessionId);
          latestSession.status = requiresCredentials ? "authRequired" : (options.fallbackStatus ?? "authRequired");
          latestSession.error = localizeError(error);
          latestSession.lastDisconnectReason = "error";
        }
        return false;
      } finally {
        if (terminalStartRequests.get(sessionId) === startRequest) terminalStartRequests.delete(sessionId);
      }
    },
    startSessionAutomatically(sessionId = this.activeSessionId, options = {}) {
      const session = this.findSession(sessionId);
      if (!session || (!session.credentialRef && usesSavedCredentials(session.authMethod))) return false;
      return this.startTerminalForSession(sessionId, {}, { ...options, credentialSource: "saved" });
    },
    reconnectActiveTerminal() {
      return this.reconnectSession(this.activeSessionId);
    },
    reconnectSession(sessionId = this.activeSessionId) {
      return this.startTerminalForSession(sessionId, {}, {
        credentialSource: "reconnect", force: true, resetReconnectAttempts: true,
      });
    },
    async sendActiveTerminalInput(data) {
      return this.sendSessionTerminalInput(this.activeSessionId, data);
    },
    async sendSessionTerminalInput(sessionId, data) {
      const session = this.findSession(sessionId);
      if (!session || session.isClosing || session.status !== "connected" || !data) return;

      await sendTerminalInput({
        sessionId: session.id,
        data,
      });
    },
    async resizeActiveTerminal(columns, rows) {
      return this.resizeSessionTerminal(this.activeSessionId, columns, rows);
    },
    async resizeSessionTerminal(sessionId, columns, rows) {
      const session = this.findSession(sessionId);
      if (!session) return;

      session.terminalColumns = columns;
      session.terminalRows = rows;
      if (session.isClosing || session.status !== "connected") return;

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
      this.persistSessionLayout();
    },
    // 将关闭或顺序变更后的完整状态一次写入，后端按字段合并，不会覆盖其它工作区偏好。
    persistSessionLayout() {
      if (this.isRestoringSessions || !useSettingsStore().workspaceDir) return;
      const sessions = this.sessions.filter((session) => !session.isClosing);
      const activeSession = sessions.find((session) => session.id === this.activeSessionId);
      return useSettingsStore().applySettingsPatch({
        sessionRestore: {
          sessions: sessions.map((session) => ({
            profileId: session.profileId,
            remotePath: session.remotePath || null,
            labelIndex: session.labelIndex ?? 1,
          })),
          activeIndex: activeSession ? sessions.indexOf(activeSession) : null,
          activeProfileId: activeSession?.profileId ?? null,
          activeLabelIndex: activeSession?.labelIndex ?? null,
        },
      });
    },
    handleTerminalEvent(event) {
      const session = this.findSession(event.sessionId);
      if (!session || session.isClosing) return;
      if (isStaleTerminalEvent(session, event)) return;
      if (["connected", "closed", "error"].includes(event.kind)) {
        useAuthenticationStore().cancelOwner(`terminal:${session.id}`);
      }

      if (event.kind === "output") {
        const data = event.data ?? "";
        if (!data) return;

        const sink = terminalSinks.get(session.id);
        if (sink?.revision === session.terminalRevision) sink.write(data);
        else appendTerminalChunk(session.id, data);
        return;
      }

      if (event.kind === "connected") {
        confirmSessionCredentials(session.id, event.attemptId);
        session.status = "connected";
        session.error = null;
        session.hostKeyChange = null;
        session.connectedAt = Date.now();
        session.lastDisconnectReason = null;
        // 首页「最近连接」打点，失败不影响会话
        if (session.workspaceDir === useSettingsStore().workspaceDir) {
          useProfileStore().markConnected(session.profileId);
        }
        return;
      }

      if (event.kind === "connecting") {
        session.status = "connecting";
        session.error = null;
        session.lastDisconnectReason = null;
        return;
      }

      if (event.kind === "closed") {
        discardPendingCredentials(session.id, event.attemptId);
        const reason = event.reason ?? "remote";
        session.status = "closed";
        session.lastDisconnectReason = reason;
        this.maybeAutoReconnect(session, reason);
        return;
      }

      if (event.kind === "error") {
        const wasConnected = session.status === "connected";
        const authenticationFailed = hasErrorCode(event.error, [ERROR_CODES.authFailed, ERROR_CODES.authKeyFile]);
        discardPendingCredentials(session.id, event.attemptId);
        if (authenticationFailed) clearSessionCredentials(session.id);
        session.status = authenticationFailed ? "authRequired" : "error";
        session.hostKeyChange = parseHostKeyChange(event.error);
        session.error = session.hostKeyChange
          ? i18n.global.t("terminal.hostKeyChangedTitle")
          : localizeError(event.error) || i18n.global.t("terminal.genericError");
        session.lastDisconnectReason = event.reason ?? "error";
        if (wasConnected && !authenticationFailed && !session.hostKeyChange) {
          this.maybeAutoReconnect(session, session.lastDisconnectReason);
        }
      }
    },
    async trustHostKeyAndReconnect(sessionId = this.activeSessionId) {
      const session = this.findSession(sessionId);
      const hostKeyChange = session?.hostKeyChange;
      if (!session || session.isClosing || !hostKeyChange) return false;
      const attemptId = session.currentAttemptId;
      const isCurrentRequest = () => !session.isClosing
        && this.findSession(sessionId)?.currentAttemptId === attemptId
        && session.hostKeyChange === hostKeyChange;

      try {
        // 指纹来自已提交的连接目标，编辑配置后不能把旧指纹信任给另一台主机。
        const profile = currentSessionProfile(session);
        if (!sameConnection(session.connectionTarget, profile)
          || !sameConnection(session.connectionTarget, session)) throw sessionConnectionChanged();
        await trustHostKey({
          host: session.connectionTarget.host,
          port: session.connectionTarget.port,
          fingerprint: hostKeyChange.actualFingerprint,
          algorithm: null,
        });
      } catch (error) {
        if (isCurrentRequest()) session.error = localizeError(error);
        return false;
      }

      if (!isCurrentRequest()) return false;
      session.hostKeyChange = null;
      session.error = null;
      return this.reconnectSession(session.id);
    },
    handleConnectionOperationError(sessionId, error) {
      if (!hasErrorCode(error, CONNECTION_LOST_CODES)) return false;

      const session = this.findSession(sessionId);
      if (!session || session.isClosing || session.status !== "connected") return false;

      session.status = "closed";
      session.error = localizeError(error);
      session.lastDisconnectReason = "remote";
      return this.maybeAutoReconnect(session, "remote");
    },
    maybeAutoReconnect(session, reason = "remote") {
      if (!session || session.isClosing || session.hostKeyChange || reason === TERMINAL_CLOSE_REASON_CLIENT
        || !["closed", "error"].includes(session.status)
        || (usesSavedCredentials(session.authMethod) && !readSessionCredentials(session) && !session.credentialRef)) {
        return false;
      }

      if (session.connectedAt && Date.now() - session.connectedAt >= STABLE_CONNECTION_RESET_MS) {
        session.autoReconnectAttempts = 0;
      }

      if (session.autoReconnectAttempts >= MAX_AUTO_RECONNECT_ATTEMPTS) {
        return false;
      }

      session.autoReconnectAttempts += 1;
      this.startTerminalForSession(session.id, {}, {
        credentialSource: "reconnect",
        force: true,
        fallbackStatus: "error",
        resetReconnectAttempts: false,
      });
      return true;
    },
    async refreshSystemInfo(sessionId = this.activeSessionId) {
      const session = this.findSession(sessionId);
      if (!session || session.isClosing || session.status !== "connected" || session.systemInfoLoading) return null;
      const attemptId = session.currentAttemptId;

      ensureSystemInfoState(session);
      session.systemInfoLoading = true;
      session.systemInfoError = null;

      try {
        const snapshot = await getRemoteSystemInfo({ sessionId: session.id });
        const latestSession = this.findSession(session.id);
        if (!latestSession || latestSession.isClosing || latestSession.currentAttemptId !== attemptId) return null;

        latestSession.systemInfo = normalizeSystemInfoSnapshot(snapshot);
        latestSession.systemInfoUpdatedAt = Date.now();
        appendNetworkSample(latestSession, latestSession.systemInfo);
        return latestSession.systemInfo;
      } catch (error) {
        const latestSession = this.findSession(session.id);
        if (latestSession && !latestSession.isClosing && latestSession.currentAttemptId === attemptId) {
          latestSession.systemInfoError = localizeError(error);
          this.handleConnectionOperationError(latestSession.id, error);
        }
        return null;
      } finally {
        const latestSession = this.findSession(session.id);
        if (latestSession && latestSession.currentAttemptId === attemptId) {
          latestSession.systemInfoLoading = false;
        }
      }
    },
    findSession(sessionId) {
      return this.sessions.find((session) => session.id === sessionId) ?? null;
    },
  },
});

function terminalBufferFor(sessionId) {
  let buffer = terminalBuffers.get(sessionId);
  if (!buffer) {
    buffer = { chunks: [], totalLength: 0 };
    terminalBuffers.set(sessionId, buffer);
  }

  return buffer;
}

function appendTerminalChunk(sessionId, data) {
  const buffer = terminalBufferFor(sessionId);
  buffer.chunks.push(data);
  buffer.totalLength += data.length;

  if (buffer.totalLength <= TERMINAL_BUFFER_MAX_CHARS) return;

  while (
    buffer.chunks.length > 1 &&
    buffer.totalLength - buffer.chunks[0].length >= TERMINAL_BUFFER_TRIM_TO_CHARS
  ) {
    buffer.totalLength -= buffer.chunks[0].length;
    buffer.chunks.shift();
  }

  // 裁剪后对齐到下一行行首，避免回放时首行落在残缺的转义序列中间
  const firstChunk = buffer.chunks[0];
  const newlineIndex = firstChunk.indexOf("\n");
  if (newlineIndex >= 0 && newlineIndex + 1 < firstChunk.length) {
    const alignedChunk = firstChunk.slice(newlineIndex + 1);
    buffer.totalLength -= firstChunk.length - alignedChunk.length;
    buffer.chunks[0] = alignedChunk;
  }
}

function readTerminalBuffer(sessionId) {
  const buffer = terminalBuffers.get(sessionId);
  if (!buffer || !buffer.chunks.length) return "";

  return buffer.chunks.join("");
}

function clearTerminalBuffer(sessionId) {
  terminalBuffers.delete(sessionId);
}

function resetTerminalBuffer(session) {
  clearTerminalBuffer(session.id);
  terminalSinks.delete(session.id);
  session.terminalRevision += 1;
}

// 主机密钥变更：后端用 hostKey.changed + params.expected/actual 上报，
// 不再靠 "HOST_KEY_CHANGED|a|b" 拼字符串。
function parseHostKeyChange(error) {
  const { code, params } = toAppError(error);
  if (code !== ERROR_CODES.hostKeyChanged) return null;

  const { expected, actual } = params;
  if (!expected || !actual) return null;

  return { expectedFingerprint: expected, actualFingerprint: actual };
}

function syncSessionFromProfile(session, profile) {
  // 连接配置改名后按会话自身的编号重算显示名，多终端后缀不丢
  session.name = sessionDisplayName(profile.name, session.labelIndex ?? 1);
  session.host = profile.host;
  session.port = profile.port;
  session.username = profile.username;
  session.authMethod = profile.authMethod;
  session.privateKeyPath = profile.privateKeyPath;
  session.agent = normalizeAgentConfig(profile.agent);
  session.credentialRef = profile.credentialRef ?? null;
  session.remotePath = session.remotePath || profile.defaultRemotePath || "/";
  // 主机、用户或认证方式改变后，旧连接成功过的秘密不能被发给新目标。
  readSessionCredentials(session);
}

function shouldAutoConnectSession(session) {
  return (
    Boolean(session && (session.credentialRef || !usesSavedCredentials(session.authMethod))) &&
    !session.isClosing &&
    !session.autoStartAttempted &&
    ["authRequired", "closed", "error"].includes(session.status)
  );
}

function canAutoStartRestoredSession(session) {
  return session?.authMethod === "agent"
    || (usesSavedCredentials(session?.authMethod) && Boolean(session?.credentialRef));
}

function sessionDisplayName(profileName, labelIndex) {
  return labelIndex > 1 ? `${profileName} (${labelIndex})` : profileName;
}

// 最小可用编号：1 号无后缀；关掉的编号可复用，保证同屏不重名
function nextSessionLabelIndex(sessions, profileId) {
  const used = new Set(
    sessions
      .filter((session) => session.profileId === profileId)
      .map((session) => session.labelIndex ?? 1),
  );
  let index = 1;
  while (used.has(index)) index += 1;
  return index;
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

function currentSessionProfile(session) {
  if (session.workspaceDir !== useSettingsStore().workspaceDir) {
    throw { code: "workspace.changed", message: "Workspace changed before connecting" };
  }
  const profile = useProfileStore().activeProfiles.find((profile) => profile.id === session.profileId);
  if (!profile) throw { code: "profile.notFound", message: "Connection profile not found" };
  return profile;
}

function sessionConnectionChanged() {
  return { code: ERROR_CODES.sessionConnectionChanged, message: "Connection changed while preparing authentication" };
}

// 仅重连优先复用本标签已验证的凭据；首次打开仍遵循用户是否保存凭据的选择。
async function resolveSessionCredentials(session, provided, source) {
  if (!usesSavedCredentials(session.authMethod)) {
    session.autoStartAttempted = true;
    return {};
  }
  if (source === "provided") return { password: provided.password, passphrase: provided.passphrase };
  if (source === "reconnect") {
    const cached = readSessionCredentials(session);
    if (cached) return cached;
  }
  if (!session.credentialRef) return null;
  session.autoStartAttempted = true;
  const credentials = await readSavedCredentials(useCredentialStore(), session);
  if (session.authMethod === "password" && !credentials.password) {
    throw new Error(i18n.global.t("terminal.savedCredentialMissing"));
  }
  return credentials;
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
