import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import { cancelFileOperation, startDiskUsage } from "../services/workspaceApi";
import { toAppError } from "../utils/appError";

const DISK_USAGE_EVENT = "disk-usage-event";
// 一层层往下钻再退回来不必重算，但缓存也不能随着浏览无限变大
const MAX_CACHED_PATHS = 24;
const SETTLED_STATUSES = new Set(["completed", "running"]);
let listeningPromise = null;

// 分析模式按会话开关：打开后进入哪个目录就算哪一层，结果按路径缓存
export const useDiskUsageStore = defineStore("diskUsage", {
  state: () => ({
    sessions: {},
    unlisten: null,
  }),
  getters: {
    isEnabled: (state) => (sessionId) => Boolean(state.sessions[sessionId]?.enabled),
    resultFor: (state) => (sessionId, path) => state.sessions[sessionId]?.results[path] ?? null,
  },
  actions: {
    // 必须先监听再启动，否则最早的几批结果会丢
    async startListening() {
      if (this.unlisten) return;
      if (!listeningPromise) {
        listeningPromise = listen(DISK_USAGE_EVENT, (event) => this.handleEvent(event.payload))
          .then((unlisten) => { this.unlisten = unlisten; })
          .finally(() => { listeningPromise = null; });
      }
      return listeningPromise;
    },
    async enable(session, path) {
      if (!session) return;
      this.sessions[session.id] ??= { enabled: false, results: {} };
      this.sessions[session.id].enabled = true;
      return this.analyze(session, path);
    },
    disable(sessionId) {
      this.cancelRunning(sessionId);
      delete this.sessions[sessionId];
    },
    // 分析模式下换了目录：算过的直接用；没算完就被打断的、失败的，回来时重算
    visit(session, path) {
      const state = this.sessions[session?.id];
      if (!state?.enabled || SETTLED_STATUSES.has(state.results[path]?.status)) return;
      return this.analyze(session, path);
    },
    // 同一会话同时只跑一个 du：在大目录上算到一半就点进子目录，前一个没必要再占着服务器
    async analyze(session, path) {
      const state = this.sessions[session?.id];
      if (!state || session.status !== "connected") return;
      await this.startListening();
      this.cancelRunning(session.id);

      const taskId = crypto.randomUUID();
      delete state.results[path];
      state.results[path] = {
        taskId,
        path,
        status: "running",
        sizes: {},
        scannedBytes: 0,
        totalBytes: null,
        unreadable: 0,
        error: null,
      };
      pruneResults(state);

      try {
        await startDiskUsage({ taskId, sessionId: session.id, path });
      } catch (error) {
        const analysis = state.results[path];
        if (analysis?.taskId === taskId && analysis.status === "running") {
          analysis.status = "failed";
          analysis.error = toAppError(error);
        }
      }
    },
    handleEvent(event) {
      const analysis = this.sessions[event.sessionId]?.results[event.path];
      if (!analysis || analysis.taskId !== event.taskId) return;
      if (analysis.status !== "running" && analysis.status !== "canceling") return;

      for (const entry of event.entries) {
        analysis.scannedBytes += entry.bytes - (analysis.sizes[entry.name] ?? 0);
        analysis.sizes[entry.name] = entry.bytes;
      }
      analysis.unreadable = event.unreadable;
      if (event.kind === "progress") return;

      analysis.status = event.kind;
      analysis.totalBytes = event.totalBytes ?? null;
      analysis.error = event.error ?? null;
    },
    cancel(sessionId, path) {
      const analysis = this.sessions[sessionId]?.results[path];
      if (analysis?.status !== "running") return;
      analysis.status = "canceling";
      // 取消失败多半是任务已经结束，终态事件会把状态改对
      cancelFileOperation(analysis.taskId).catch(() => {});
    },
    cancelRunning(sessionId) {
      for (const path of Object.keys(this.sessions[sessionId]?.results ?? {})) {
        this.cancel(sessionId, path);
      }
    },
  },
});

function pruneResults(state) {
  const paths = Object.keys(state.results);
  for (const path of paths.slice(0, Math.max(0, paths.length - MAX_CACHED_PATHS))) {
    if (state.results[path].status !== "running") delete state.results[path];
  }
}
