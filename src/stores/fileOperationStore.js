import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import {
  cancelFileOperation,
  startRemoteCompress,
  startRemoteDelete,
  startRemoteExtract,
} from "../services/workspaceApi";
import { useFileExplorerStore } from "./fileExplorerStore";
import { useSessionStore } from "./sessionStore";
import { ERROR_CODES, toAppError } from "../utils/appError";

const FINISHED_STATUSES = new Set(["completed", "canceled", "failed"]);
let listeningPromise = null;

export const useFileOperationStore = defineStore("fileOperations", {
  state: () => ({
    tasks: [],
    panelOpen: false,
    unlisten: null,
    // 压缩解压从右键菜单发起，没有弹窗可以挂结果；结束时由 App 层弹提示
    finishedSignal: null,
  }),
  getters: {
    activeCount: (state) => state.tasks.filter((task) => !FINISHED_STATUSES.has(task.status)).length,
    hasFinished: (state) => state.tasks.some((task) => FINISHED_STATUSES.has(task.status)),
  },
  actions: {
    // 必须先成功监听再启动任务，否则用户可能看不到真实进度和取消结果。
    async startListening() {
      if (this.unlisten) return;
      if (!listeningPromise) {
        listeningPromise = listen("file-operation-event", (event) => this.handleEvent(event.payload))
          .then((unlisten) => { this.unlisten = unlisten; })
          .finally(() => { listeningPromise = null; });
      }
      return listeningPromise;
    },
    startDelete(session, entries, recursive) {
      return this.launch(session, "delete", entries, (taskId) => startRemoteDelete({
        taskId,
        sessionId: session.id,
        entries: entries.map((entry) => ({
          path: entry.path,
          name: entry.name,
          isDirectory: entry.isDirectory,
          size: entry.size,
        })),
        recursive,
      }));
    },
    // 压缩包放在被压缩项所在的目录；archiveName 不带扩展名，重名由服务器加序号
    startCompress(session, directory, entries, format, archiveName) {
      return this.launch(session, "compress", entries, (taskId) => startRemoteCompress({
        taskId,
        sessionId: session.id,
        directory,
        names: entries.map((entry) => entry.name),
        format,
        archiveName,
      }));
    },
    startExtract(session, entry, intoFolder) {
      return this.launch(session, "extract", [entry], (taskId) => startRemoteExtract({
        taskId,
        sessionId: session.id,
        archivePath: entry.path,
        intoFolder,
      }));
    },
    // 任务先进入全局状态，确认弹窗关闭或切换标签后仍然能跟踪和取消。
    async launch(session, kind, entries, start) {
      if (!session || session.status !== "connected" || !entries.length) return;
      await this.startListening();
      const taskId = crypto.randomUUID();
      this.tasks.unshift({
        id: taskId,
        kind,
        sessionId: session.id,
        sessionName: session.name,
        paths: entries.map((entry) => entry.path),
        label: entries[0].name,
        entryCount: entries.length,
        status: "queued",
        completedEntries: 0,
        currentPath: null,
        resultPath: null,
        error: null,
        hasFinalEvent: false,
      });
      try {
        await start(taskId);
        return taskId;
      } catch (error) {
        const current = this.tasks.find((item) => item.id === taskId);
        if (current && !FINISHED_STATUSES.has(current.status)) {
          const appError = toAppError(error);
          current.status = appError.code === ERROR_CODES.fileOperationCanceled ? "canceled" : "failed";
          current.error = appError;
        }
        this.pruneFinished();
        throw error;
      }
    },
    // 后端终态只消费一次；启动确认与取消交错时，仍以终态事件补齐最后的计数。
    handleEvent(event) {
      const task = this.tasks.find((item) => item.id === event.taskId && item.sessionId === event.sessionId);
      if (!task || task.hasFinalEvent) return;
      const finished = FINISHED_STATUSES.has(event.kind);
      if (!finished && FINISHED_STATUSES.has(task.status)) return;
      if (finished) task.status = event.kind;
      else if (task.status !== "canceling") task.status = "running";
      task.completedEntries = event.completedEntries;
      task.currentPath = event.currentPath;
      task.resultPath = event.resultPath ?? task.resultPath;
      task.error = event.error ?? null;
      if (!finished) return;
      task.hasFinalEvent = true;
      // 取消时脚本已经删掉了没做完的压缩包 / 解压目录
      if (event.kind === "canceled") task.resultPath = null;

      // 失败/取消也可能已经改动了部分文件；刷新来源会话，不能误刷新用户刚切去的标签。
      const session = useSessionStore().findSession(task.sessionId);
      if (session?.status === "connected") {
        useFileExplorerStore().refreshAfterChange(session, task.paths);
      }
      if (task.kind !== "delete" && event.kind !== "canceled") {
        this.finishedSignal = { taskId: task.id };
      }
      this.pruneFinished();
    },
    consumeFinishedSignal() {
      const signal = this.finishedSignal;
      this.finishedSignal = null;
      return signal;
    },
    findTask(taskId) {
      return this.tasks.find((task) => task.id === taskId) ?? null;
    },
    async cancelTask(taskId) {
      const task = this.tasks.find((item) => item.id === taskId);
      if (!task || FINISHED_STATUSES.has(task.status) || task.status === "canceling") return;
      const previousStatus = task.status;
      task.status = "canceling";
      try {
        await cancelFileOperation(taskId);
      } catch (error) {
        if (!FINISHED_STATUSES.has(task.status)) {
          task.status = previousStatus;
          task.error = toAppError(error);
        }
      }
    },
    isActive(task) {
      return !FINISHED_STATUSES.has(task.status);
    },
    removeTask(taskId) {
      this.tasks = this.tasks.filter((task) => task.id !== taskId || !FINISHED_STATUSES.has(task.status));
    },
    clearFinished() {
      this.tasks = this.tasks.filter((task) => !FINISHED_STATUSES.has(task.status));
    },
    pruneFinished() {
      let kept = 0;
      this.tasks = this.tasks.filter((task) => !FINISHED_STATUSES.has(task.status) || ++kept <= 30);
    },
  },
});
