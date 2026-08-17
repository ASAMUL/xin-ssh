import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { i18n } from "../i18n";
import {
  cancelTransfer,
  startDownloadTransfer,
  startUploadTransfer,
} from "../services/workspaceApi";

const TRANSFER_EVENT_NAME = "transfer-event";
const FINISHED_STATUSES = new Set(["completed", "failed", "canceled"]);
const ACTIVE_STATUSES = new Set(["queued", "running", "canceling"]);
// 已结束任务保留的条数上限：面板兼作跨会话的传输历史，放宽到 100 条
const MAX_FINISHED_TASKS = 100;
const HISTORY_STORAGE_KEY = "xin-terminal/transfer-history";
// 每条记录最多持久化的路径条数，避免大批量传输把 localStorage 撑爆
const MAX_PERSISTED_PATHS = 20;

export const useTransferStore = defineStore("transfers", {
  state: () => ({
    tasks: loadPersistedHistory(),
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
    async startListening() {
      if (this.unlistenTransfer) return;

      try {
        this.unlistenTransfer = await listen(TRANSFER_EVENT_NAME, (event) => {
          this.handleTransferEvent(event.payload);
        });
      } catch (error) {
        console.warn("[Xin Terminal] transfer event listener unavailable", error);
      }
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
      const taskId = options.taskId || crypto.randomUUID();
      const label = transferLabel(localPaths, "transfers.uploadFallback", "transfers.localItems");
      this.panelOpen = true;
      this.upsertTask({
        id: taskId,
        sessionId: session.id,
        sessionName: session.name || session.host || null,
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
      });

      try {
        await startUploadTransfer({
          taskId,
          sessionId: session.id,
          localPaths,
          remoteDir,
          conflictPolicy: options.conflictPolicy ?? null,
        });
      } catch (error) {
        this.failTask(taskId, String(error));
        throw error;
      }

      return taskId;
    },
    async downloadEntries(session, entries, localDir, options = {}) {
      const taskId = options.taskId || crypto.randomUUID();
      const label = transferLabel(
        entries.map((entry) => entry.name),
        "transfers.downloadFallback",
        "transfers.remoteItems",
      );
      this.panelOpen = true;
      this.upsertTask({
        id: taskId,
        sessionId: session.id,
        sessionName: session.name || session.host || null,
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
      });

      try {
        await startDownloadTransfer({
          taskId,
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
      } catch (error) {
        this.failTask(taskId, String(error));
        throw error;
      }

      return taskId;
    },
    retryTaskWithConflictPolicy(taskId, conflictPolicy) {
      const task = this.findTask(taskId);
      if (!task || !conflictPolicy || conflictPolicy === "cancel") return null;

      const sessionRef = { id: task.sessionId, name: task.sessionName };

      if (task.direction === "upload") {
        return this.uploadPaths(sessionRef, task.localPaths || [], task.remoteDir, {
          taskId: task.id,
          conflictPolicy,
        });
      }

      return this.downloadEntries(sessionRef, task.remoteEntries || [], task.localDir, {
        taskId: task.id,
        conflictPolicy,
      });
    },
    async cancelTask(taskId) {
      const task = this.findTask(taskId);
      if (!task || FINISHED_STATUSES.has(task.status)) return;

      this.upsertTask({
        ...task,
        status: "canceling",
        error: null,
        updatedAt: Date.now(),
      });

      try {
        await cancelTransfer(taskId);
      } catch (error) {
        this.failTask(taskId, String(error));
      }
    },
    markTaskCanceled(taskId) {
      const task = this.findTask(taskId);
      if (!task) return;

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
      const task = this.findTask(event.taskId);
      const previousBytes = task?.bytesTransferred ?? 0;
      const previousUpdatedAt = task?.updatedAt ?? now;
      const elapsedSeconds = Math.max((now - previousUpdatedAt) / 1000, 0.001);
      const rootLabel = task?.rootLabel || task?.label || event.label || event.direction;
      const label = event.kind === "progress" ? (event.label || rootLabel) : rootLabel;
      const speedBytesPerSecond =
        event.kind === "progress"
          ? Math.max(0, (event.bytesTransferred - previousBytes) / elapsedSeconds)
          : (task?.speedBytesPerSecond ?? 0);

      this.upsertTask({
        id: event.taskId,
        sessionId: event.sessionId,
        direction: event.direction,
        status: eventStatus(event.kind),
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
          taskId: event.taskId,
          at: now,
        };
      }
    },
    upsertTask(nextTask) {
      const now = Date.now();
      const taskIndex = this.tasks.findIndex((task) => task.id === nextTask.id);
      const normalizedTask = {
        createdAt: now,
        updatedAt: now,
        currentPath: null,
        error: null,
        ...nextTask,
      };

      if (taskIndex >= 0) {
        this.tasks[taskIndex] = {
          ...this.tasks[taskIndex],
          ...normalizedTask,
        };
      } else {
        this.tasks.unshift(normalizedTask);
      }

      this.pruneFinishedTasks();

      if (FINISHED_STATUSES.has(normalizedTask.status)) {
        this.persistFinishedTasks();
      }
    },
    pruneFinishedTasks() {
      const finishedCount = this.tasks.reduce(
        (count, task) => count + (FINISHED_STATUSES.has(task.status) ? 1 : 0),
        0,
      );
      if (finishedCount <= MAX_FINISHED_TASKS) return;

      // 任务按新到旧排列，保留最新的已结束任务
      let kept = 0;
      this.tasks = this.tasks.filter((task) => {
        if (!FINISHED_STATUSES.has(task.status)) return true;

        kept += 1;
        return kept <= MAX_FINISHED_TASKS;
      });
    },
    failTask(taskId, error) {
      const task = this.findTask(taskId);
      if (!task) return;

      this.upsertTask({
        ...task,
        status: "failed",
        error,
        updatedAt: Date.now(),
      });
    },
    removeTask(taskId) {
      const task = this.findTask(taskId);
      if (!task || !FINISHED_STATUSES.has(task.status)) return;

      this.tasks = this.tasks.filter((item) => item.id !== taskId);
      this.persistFinishedTasks();
    },
    clearFinished() {
      this.tasks = this.tasks.filter((task) => !FINISHED_STATUSES.has(task.status));
      this.persistFinishedTasks();
    },
    persistFinishedTasks() {
      try {
        const finished = this.tasks
          .filter((task) => FINISHED_STATUSES.has(task.status))
          .slice(0, MAX_FINISHED_TASKS)
          .map(slimTaskForStorage);
        localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(finished));
      } catch (error) {
        console.warn("[Xin Terminal] persist transfer history failed", error);
      }
    },
    canCancelTask(task) {
      return ACTIVE_STATUSES.has(task?.status);
    },
    findTask(taskId) {
      return this.tasks.find((task) => task.id === taskId) ?? null;
    },
  },
});

function eventStatus(kind) {
  if (kind === "completed") return "completed";
  if (kind === "failed") return "failed";
  if (kind === "canceled") return "canceled";

  return "running";
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

// 只持久化展示与“打开/复制路径”所需的字段，运行态字段（速度等）不落盘
function slimTaskForStorage(task) {
  return {
    id: task.id,
    sessionId: task.sessionId ?? null,
    sessionName: task.sessionName ?? null,
    direction: task.direction,
    status: task.status,
    label: task.label,
    rootLabel: task.rootLabel ?? task.label,
    bytesTransferred: task.bytesTransferred ?? 0,
    bytesTotal: task.bytesTotal ?? 0,
    filesDone: task.filesDone ?? 0,
    filesTotal: task.filesTotal ?? 0,
    error: task.error ?? null,
    createdAt: task.createdAt ?? Date.now(),
    updatedAt: task.updatedAt ?? Date.now(),
    localDir: task.localDir ?? null,
    remoteDir: task.remoteDir ?? null,
    localPaths: Array.isArray(task.localPaths) ? task.localPaths.slice(0, MAX_PERSISTED_PATHS) : [],
    remoteEntries: Array.isArray(task.remoteEntries)
      ? task.remoteEntries.slice(0, MAX_PERSISTED_PATHS).map((entry) => ({
          path: entry.path,
          name: entry.name,
          isDirectory: Boolean(entry.isDirectory),
        }))
      : [],
  };
}

function loadPersistedHistory() {
  try {
    const raw = localStorage.getItem(HISTORY_STORAGE_KEY);
    if (!raw) return [];

    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];

    return parsed
      .filter((task) => task && task.id && FINISHED_STATUSES.has(task.status))
      .slice(0, MAX_FINISHED_TASKS)
      .map((task) => ({
        ...task,
        speedBytesPerSecond: 0,
        currentPath: null,
        conflictPolicy: null,
        conflictPrompted: true,
      }));
  } catch (error) {
    console.warn("[Xin Terminal] load transfer history failed", error);
    return [];
  }
}
