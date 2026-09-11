import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { i18n } from "../i18n";
import { toAppError } from "../utils/appError";
import { sameConnection } from "../utils/connectionIdentity";
import { loadTransferHistory, saveTransferHistory } from "../services/transferHistory";
import { useSessionStore } from "./sessionStore";
import { useProfileStore } from "./profileStore";
import { useSettingsStore } from "./settingsStore";
import { useUiStore } from "./uiStore";
import {
  cancelTransfer,
  startDownloadTransfer,
  startUploadTransfer,
} from "../services/workspaceApi";

const TRANSFER_EVENT_NAME = "transfer-event";
const FINISHED_STATUSES = new Set(["completed", "failed", "canceled"]);
const ACTIVE_STATUSES = new Set(["queued", "planning", "running", "canceling"]);
const speedSamples = new Map();
// 已结束任务保留的条数上限：面板兼作跨会话的传输历史，放宽到 100 条
const MAX_FINISHED_TASKS = 100;
let historyLoading = null;
let listeningPromise = null;

export const useTransferStore = defineStore("transfers", {
  state: () => ({
    tasks: [],
    historyLoaded: false,
    historyIsLoading: false,
    historyError: null,
    retryingTasks: {},
    panelOpen: false,
    // 下载完成信号：面板关闭时由 App 层消费并弹出快捷提示
    downloadReadySignal: null,
    unlistenTransfer: null,
  }),
  getters: {
    hasTasks: (state) => state.tasks.length > 0,
    hasFinishedTasks: (state) => state.tasks.some((task) => FINISHED_STATUSES.has(task.status)),
    summary: (state) => {
      const activeTasks = state.tasks.filter((task) => !FINISHED_STATUSES.has(task.status));
      const sourceTasks = activeTasks.length ? activeTasks : state.tasks.slice(0, 2);
      const bytesTransferred = sourceTasks.reduce((total, task) => total + task.bytesTransferred, 0);
      const bytesTotal = sourceTasks.reduce((total, task) => total + task.bytesTotal, 0);
      const progress = bytesTotal > 0 ? Math.round((bytesTransferred / bytesTotal) * 100) : 0;

      return {
        progress: Math.min(progress, 100),
        activeCount: activeTasks.length,
        totalCount: state.tasks.length,
      };
    },
  },
  actions: {
    // 加载失败时保留磁盘原件；新任务可留在内存，重试成功后再合并写回。
    async loadHistory() {
      if (this.historyLoaded) return true;
      if (historyLoading) return historyLoading;
      this.historyIsLoading = true;
      historyLoading = loadTransferHistory().then((history) => {
        const currentIds = new Set(this.tasks.map((task) => task.id));
        this.tasks.push(...history.filter((task) => !currentIds.has(task.id)));
        this.pruneFinishedTasks();
        this.historyLoaded = true;
        this.historyError = null;
        this.persistHistory();
        return true;
      }).catch((error) => {
        console.warn("[Xin SSH] load transfer history failed", error);
        this.historyError = { code: "transfer.historyLoadFailed", message: String(error) };
        return false;
      }).finally(() => {
        this.historyIsLoading = false;
        historyLoading = null;
      });
      return historyLoading;
    },
    async startListening() {
      if (this.unlistenTransfer) return;
      if (!listeningPromise) {
        listeningPromise = listen(TRANSFER_EVENT_NAME, (event) => {
          this.handleTransferEvent(event.payload);
        }).then((unlisten) => { this.unlistenTransfer = unlisten; })
          .finally(() => { listeningPromise = null; });
      }
      return listeningPromise;
    },
    async prepareTransfer(session, options) {
      // 历史读取是异步的，等待时同一标签可能重连到修改后的地址，不能沿用旧目录发起传输。
      const expectedConnection = session?.connectionTarget ? { ...session.connectionTarget } : null;
      await this.loadHistory();
      await this.startListening();
      const current = useSessionStore().findSession(session?.id);
      if (!current || current.status !== "connected" || !current.connectionTarget) {
        throw { code: "session.notRunning", message: "Terminal session is not connected" };
      }
      if (!sameConnection(expectedConnection, current.connectionTarget)) {
        throw { code: "transfer.connectionChanged", message: "Connection changed before starting the transfer" };
      }
      if (options.taskId) {
        const original = this.findTask(options.taskId);
        const error = this.resumeBlockReason(original, options.conflictPolicy, true);
        if (error) throw error;
        if (current.profileId !== original.profileId || current.workspaceDir !== original.workspaceDir
          || !sameConnection(current.connectionTarget, original.connection)) {
          throw { code: "transfer.connectionChanged", message: "Connection changed before resuming" };
        }
      }
      return current;
    },
    stopListening() {
      this.unlistenTransfer?.();
      this.unlistenTransfer = null;
    },
    togglePanel() {
      this.panelOpen = !this.panelOpen;
    },
    closePanel() {
      this.panelOpen = false;
    },
    consumeDownloadReadySignal() {
      const signal = this.downloadReadySignal;
      this.downloadReadySignal = null;
      return signal;
    },
    async uploadPaths(session, localPaths, remoteDir, options = {}) {
      localPaths = [...localPaths];
      session = await this.prepareTransfer(session, options);
      const taskId = options.taskId || crypto.randomUUID();
      const backendTaskId = crypto.randomUUID();
      const label = transferLabel(localPaths, "transfers.uploadFallback", "transfers.localItems");
      this.panelOpen = true;
      this.upsertTask({
        id: taskId,
        backendTaskId,
        sessionId: session.id,
        sessionName: session.name || session.host || null,
        profileId: session.profileId,
        workspaceDir: session.workspaceDir,
        connection: { ...session.connectionTarget },
        resumeMetadataVersion: 1,
        sourceCount: localPaths.length,
        direction: "upload",
        status: "queued",
        label,
        rootLabel: label,
        bytesTransferred: 0,
        bytesTotal: 0,
        filesDone: 0,
        filesTotal: 0,
        speedBytesPerSecond: 0,
        localPaths,
        remoteDir,
        conflictPolicy: options.conflictPolicy ?? null,
        conflictPrompted: Boolean(options.conflictPolicy),
        error: null,
        currentPath: null,
      });

      try {
        await startUploadTransfer({
          taskId: backendTaskId,
          sessionId: session.id,
          localPaths,
          remoteDir,
          conflictPolicy: options.conflictPolicy ?? null,
        });
        const current = this.findTask(taskId);
        if (current?.backendTaskId === backendTaskId && current.status === "canceling") {
          await this.requestTaskCancellation(taskId, backendTaskId);
        }
      } catch (error) {
        this.failTask(taskId, backendTaskId, error);
        throw error;
      }

      return taskId;
    },
    async downloadEntries(session, entries, localDir, options = {}) {
      entries = entries.map((entry) => ({
        path: entry.path, name: entry.name, isDirectory: Boolean(entry.isDirectory), size: entry.size ?? null,
      }));
      session = await this.prepareTransfer(session, options);
      const taskId = options.taskId || crypto.randomUUID();
      const backendTaskId = crypto.randomUUID();
      const label = transferLabel(
        entries.map((entry) => entry.name),
        "transfers.downloadFallback",
        "transfers.remoteItems",
      );
      this.panelOpen = true;
      this.upsertTask({
        id: taskId,
        backendTaskId,
        sessionId: session.id,
        sessionName: session.name || session.host || null,
        profileId: session.profileId,
        workspaceDir: session.workspaceDir,
        connection: { ...session.connectionTarget },
        resumeMetadataVersion: 1,
        sourceCount: entries.length,
        direction: "download",
        status: "queued",
        label,
        rootLabel: label,
        bytesTransferred: 0,
        bytesTotal: 0,
        filesDone: 0,
        filesTotal: 0,
        speedBytesPerSecond: 0,
        remoteEntries: entries,
        localDir,
        conflictPolicy: options.conflictPolicy ?? null,
        conflictPrompted: Boolean(options.conflictPolicy),
        error: null,
        currentPath: null,
      });

      try {
        await startDownloadTransfer({
          taskId: backendTaskId,
          sessionId: session.id,
          localDir,
          remoteEntries: entries.map((entry) => ({
            path: entry.path,
            name: entry.name,
            isDirectory: entry.isDirectory,
            size: entry.size,
          })),
          conflictPolicy: options.conflictPolicy ?? null,
        });
        const current = this.findTask(taskId);
        if (current?.backendTaskId === backendTaskId && current.status === "canceling") {
          await this.requestTaskCancellation(taskId, backendTaskId);
        }
      } catch (error) {
        this.failTask(taskId, backendTaskId, error);
        throw error;
      }

      return taskId;
    },
    // 同一记录每次重试都使用新后端 ID，旧事件和旧任务清理不会影响这次传输。
    async retryTaskWithConflictPolicy(taskId, conflictPolicy) {
      const task = this.findTask(taskId);
      if (!task || !conflictPolicy || conflictPolicy === "cancel") return null;
      const error = this.resumeBlockReason(task, conflictPolicy);
      if (error) throw error;
      this.retryingTasks[taskId] = true;
      try {
        const session = this.findResumeSession(task);
        const options = { taskId: task.id, conflictPolicy };
        if (task.direction === "upload") {
          return await this.uploadPaths(session, task.localPaths, task.remoteDir, options);
        }
        return await this.downloadEntries(session, task.remoteEntries, task.localDir, options);
      } finally {
        delete this.retryingTasks[taskId];
      }
    },
    // 断点续传：resume 策略重发原任务，两端半成品保留，缺多少补多少
    resumeTask(taskId) {
      return this.retryTaskWithConflictPolicy(taskId, "resume");
    },
    async cancelTask(taskId) {
      const task = this.findTask(taskId);
      if (!task || FINISHED_STATUSES.has(task.status) || task.status === "canceling") return;

      this.upsertTask({
        ...task,
        status: "canceling",
        error: null,
        updatedAt: Date.now(),
      });

      await this.requestTaskCancellation(taskId, task.backendTaskId, task.status);
    },
    async requestTaskCancellation(taskId, backendTaskId, fallbackStatus = "running") {
      try {
        await cancelTransfer(backendTaskId);
      } catch (error) {
        const current = this.findTask(taskId);
        if (current?.backendTaskId === backendTaskId && current.status === "canceling") {
          // 取消请求失败不代表传输结束，必须继续接收真实进度并允许再次取消。
          this.upsertTask({ ...current, status: fallbackStatus, error: toAppError(error) });
          useUiStore().setError(error);
        }
      }
    },
    markTaskCanceled(taskId) {
      const task = this.findTask(taskId);
      if (!this.isResumeCandidate(task) || this.retryingTasks[taskId]) return;

      this.upsertTask({
        ...task,
        status: "canceled",
        error: null,
        updatedAt: Date.now(),
      });
    },
    markConflictPrompted(taskId) {
      const task = this.findTask(taskId);
      if (!task) return;

      this.upsertTask({
        ...task,
        conflictPrompted: true,
      });
    },
    handleTransferEvent(event) {
      const now = Date.now();
      const task = this.tasks.find((task) => task.backendTaskId === event.taskId
        && task.sessionId === event.sessionId && task.direction === event.direction);
      if (!task || FINISHED_STATUSES.has(task.status)) return;
      const rootLabel = task?.rootLabel || task?.label || event.label || event.direction;
      const label = event.kind === "progress" ? (event.label || rootLabel) : rootLabel;
      const speedBytesPerSecond = transferSpeed(event, task);

      this.upsertTask({
        id: task.id,
        sessionId: event.sessionId,
        direction: event.direction,
        status: task.status === "canceling" && !FINISHED_STATUSES.has(eventStatus(event.kind))
          ? "canceling" : eventStatus(event.kind),
        label,
        rootLabel,
        currentPath: event.currentPath,
        bytesTransferred: event.bytesTransferred ?? task?.bytesTransferred ?? 0,
        bytesTotal: event.bytesTotal ?? task?.bytesTotal ?? 0,
        filesDone: event.filesDone ?? task?.filesDone ?? 0,
        filesTotal: event.filesTotal ?? task?.filesTotal ?? 0,
        error: event.error ?? null,
        speedBytesPerSecond,
        createdAt: task?.createdAt ?? now,
        updatedAt: now,
      });

      // 下载刚完成且面板处于关闭状态时，交给 App 层弹出“打开文件夹”快捷提示
      const justCompleted = event.kind === "completed" && task?.status !== "completed";
      if (justCompleted && event.direction === "download" && !this.panelOpen) {
        this.downloadReadySignal = {
          taskId: task.id,
          at: now,
        };
      }
    },
    upsertTask(nextTask) {
      const now = Date.now();
      const taskIndex = this.tasks.findIndex((task) => task.id === nextTask.id);
      const normalizedTask = {
        createdAt: now,
        currentPath: null,
        error: null,
        ...(taskIndex >= 0 ? this.tasks[taskIndex] : {}),
        ...nextTask,
        updatedAt: nextTask.updatedAt ?? now,
      };

      if (taskIndex >= 0) {
        if (nextTask.status === "queued") {
          this.tasks.splice(taskIndex, 1);
          this.tasks.unshift(normalizedTask);
        } else {
          this.tasks[taskIndex] = normalizedTask;
        }
      } else {
        this.tasks.unshift(normalizedTask);
      }

      this.pruneFinishedTasks();

      if (FINISHED_STATUSES.has(normalizedTask.status) || nextTask.status === "queued") {
        this.persistHistory();
      }
    },
    pruneFinishedTasks() {
      const finishedCount = this.tasks.reduce(
        (count, task) => count + (FINISHED_STATUSES.has(task.status) ? 1 : 0),
        0,
      );
      if (finishedCount <= MAX_FINISHED_TASKS) return;

      // 长任务和旧记录的重试也可能刚刚结束，按最近更新时间淘汰，不能按最初入队位置删除。
      const recentFinishedIds = new Set(this.tasks
        .filter((task) => FINISHED_STATUSES.has(task.status))
        .sort((left, right) => (right.updatedAt ?? right.createdAt ?? 0) - (left.updatedAt ?? left.createdAt ?? 0))
        .slice(0, MAX_FINISHED_TASKS)
        .map((task) => task.id));
      this.tasks = this.tasks.filter((task) => !FINISHED_STATUSES.has(task.status)
        || this.retryingTasks[task.id] || recentFinishedIds.has(task.id));
    },
    failTask(taskId, backendTaskId, error) {
      const task = this.findTask(taskId);
      // 启动请求的拒绝可能晚于终态事件或下一次重试，只允许它结束自己的当前尝试。
      if (!task || task.backendTaskId !== backendTaskId || FINISHED_STATUSES.has(task.status)) return;
      speedSamples.delete(backendTaskId);

      this.upsertTask({
        ...task,
        status: "failed",
        // 保留结构化错误对象：面板据 code 判断是否是冲突、是否可续传
        error: toAppError(error),
        updatedAt: Date.now(),
      });
    },
    removeTask(taskId) {
      const task = this.findTask(taskId);
      if (!task || !FINISHED_STATUSES.has(task.status) || this.retryingTasks[taskId]) return;

      this.tasks = this.tasks.filter((item) => item.id !== taskId);
      this.persistHistory();
    },
    clearFinished() {
      this.tasks = this.tasks.filter((task) => !FINISHED_STATUSES.has(task.status) || this.retryingTasks[task.id]);
      this.persistHistory();
    },
    async persistHistory() {
      if (!this.historyLoaded) return false;
      try {
        await saveTransferHistory(this.tasks);
        this.historyError = null;
        return true;
      } catch (error) {
        console.warn("[Xin SSH] persist transfer history failed", error);
        this.historyError = { code: "transfer.historySaveFailed", message: String(error) };
        return false;
      }
    },
    retryHistoryStorage() {
      return this.historyLoaded ? this.persistHistory() : this.loadHistory();
    },
    canCancelTask(task) {
      return ACTIVE_STATUSES.has(task?.status);
    },
    isResumeCandidate(task) {
      return Boolean(task && ["failed", "canceled"].includes(task.status));
    },
    canResumeTask(task) {
      return this.isResumeCandidate(task) && !this.resumeBlockReason(task);
    },
    // profileId 只标识配置；同时核对地址与工作区，避免配置改指另一台主机后误续传。
    resumeBlockReason(task, conflictPolicy = "resume", allowRetrying = false) {
      const fail = (code) => ({ code, message: code, params: { name: task?.sessionName || task?.connection?.host || "" } });
      if (!this.isResumeCandidate(task) || (!allowRetrying && this.retryingTasks[task.id])) return fail("transfer.retryInProgress");
      if (task.resumeMetadataVersion !== 1 || !task.profileId || !task.connection) return fail("transfer.historyLegacy");
      if (!hasCompleteSources(task)) return fail("transfer.historyIncomplete");
      if (conflictPolicy === "resume" && task.conflictPolicy === "rename") return fail("transfer.resumeRenamed");
      if (task.workspaceDir !== useSettingsStore().workspaceDir) return fail("workspace.changed");
      const profile = useProfileStore().activeProfiles.find((profile) => profile.id === task.profileId);
      if (!profile) return fail("profile.notFound");
      if (!sameConnection(task.connection, profile)) return fail("transfer.connectionChanged");
      if (!this.findResumeSession(task)) return fail("transfer.connectionRequired");
      return null;
    },
    findResumeSession(task) {
      const sessions = useSessionStore();
      const candidates = sessions.sessions.filter((session) => session.status === "connected"
        && session.profileId === task.profileId && session.workspaceDir === task.workspaceDir
        && sameConnection(session.connectionTarget, task.connection));
      return candidates.find((session) => session.id === task.sessionId)
        ?? candidates.find((session) => session.id === sessions.activeSessionId)
        ?? candidates[0] ?? null;
    },
    findTask(taskId) {
      return this.tasks.find((task) => task.id === taskId) ?? null;
    },
  },
});

function eventStatus(kind) {
  if (kind === "planning") return "planning";
  if (kind === "completed") return "completed";
  if (kind === "failed") return "failed";
  if (kind === "canceled") return "canceled";

  return "running";
}

// 以单调时钟和时间加权 EMA 平滑速度；started 重置基线，已续传字节和扫描耗时不计入网速。
function transferSpeed(event, task) {
  const at = performance.now();
  const bytes = event.bytesTransferred ?? task.bytesTransferred ?? 0;
  const previous = speedSamples.get(event.taskId);
  if (event.kind === "started") {
    speedSamples.set(event.taskId, { at, bytes });
    return 0;
  }
  if (event.kind !== "progress") {
    speedSamples.delete(event.taskId);
    return 0;
  }
  if (!previous || bytes < previous.bytes) {
    speedSamples.set(event.taskId, { at, bytes });
    return 0;
  }
  const seconds = (at - previous.at) / 1000;
  if (seconds <= 0) return task.speedBytesPerSecond ?? 0;
  speedSamples.set(event.taskId, { at, bytes });
  const instantaneous = (bytes - previous.bytes) / seconds;
  const weight = 1 - Math.exp(-seconds / 1.5);
  const current = task.speedBytesPerSecond ?? 0;
  return current + weight * (instantaneous - current);
}

function transferLabel(items, fallbackKey, multiKey) {
  const { t } = i18n.global;

  if (items.length === 1) {
    const fallback = t(fallbackKey);
    const value = String(items[0] || fallback);
    return value.split(/[\\/]/).filter(Boolean).at(-1) || fallback;
  }

  return t(multiKey, { count: items.length });
}

function hasCompleteSources(task) {
  if (!Number.isInteger(task.sourceCount) || task.sourceCount <= 0) return false;
  const hasPath = (value) => typeof value === "string" && value.trim().length > 0;
  if (task.direction === "upload") {
    return hasPath(task.remoteDir) && Array.isArray(task.localPaths)
      && task.localPaths.length === task.sourceCount && task.localPaths.every(hasPath);
  }
  return task.direction === "download" && hasPath(task.localDir) && Array.isArray(task.remoteEntries)
    && task.remoteEntries.length === task.sourceCount
    && task.remoteEntries.every((entry) => entry && hasPath(entry.path)
      && hasPath(entry.name) && typeof entry.isDirectory === "boolean");
}
