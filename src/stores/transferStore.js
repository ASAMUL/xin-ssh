import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { i18n } from "../i18n";
import { startDownloadTransfer, startUploadTransfer } from "../services/workspaceApi";

const TRANSFER_EVENT_NAME = "transfer-event";
const FINISHED_STATUSES = new Set(["completed", "failed"]);

export const useTransferStore = defineStore("transfers", {
  state: () => ({
    tasks: [],
    unlistenTransfer: null,
  }),
  getters: {
    visibleTasks: (state) => state.tasks.slice(0, 4),
    hasVisibleTasks: (state) => state.tasks.length > 0,
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
    async uploadPaths(session, localPaths, remoteDir) {
      const taskId = crypto.randomUUID();
      const label = transferLabel(localPaths, "transfers.uploadFallback", "transfers.localItems");
      this.upsertTask({
        id: taskId,
        sessionId: session.id,
        direction: "upload",
        status: "queued",
        label,
        rootLabel: label,
        bytesTransferred: 0,
        bytesTotal: 0,
        filesDone: 0,
        filesTotal: 0,
        speedBytesPerSecond: 0,
      });

      try {
        await startUploadTransfer({
          taskId,
          sessionId: session.id,
          localPaths,
          remoteDir,
        });
      } catch (error) {
        this.failTask(taskId, String(error));
      }

      return taskId;
    },
    async downloadEntries(session, entries, localDir) {
      const taskId = crypto.randomUUID();
      const label = transferLabel(
        entries.map((entry) => entry.name),
        "transfers.downloadFallback",
        "transfers.remoteItems",
      );
      this.upsertTask({
        id: taskId,
        sessionId: session.id,
        direction: "download",
        status: "queued",
        label,
        rootLabel: label,
        bytesTransferred: 0,
        bytesTotal: 0,
        filesDone: 0,
        filesTotal: 0,
        speedBytesPerSecond: 0,
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
        });
      } catch (error) {
        this.failTask(taskId, String(error));
      }

      return taskId;
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
    clearFinished() {
      this.tasks = this.tasks.filter((task) => !FINISHED_STATUSES.has(task.status));
    },
    findTask(taskId) {
      return this.tasks.find((task) => task.id === taskId) ?? null;
    },
  },
});

function eventStatus(kind) {
  if (kind === "completed") return "completed";
  if (kind === "failed") return "failed";

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
