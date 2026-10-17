import { createStore, get, set } from "idb-keyval";
import { createSerialQueue } from "../utils/serialQueue";
import { toAppError } from "../utils/appError";
import { connectionIdentity } from "../utils/connectionIdentity";

const historyStore = createStore("xin-terminal-transfer-history", "history");
const HISTORY_KEY = "tasks";
const LEGACY_KEY = "xin-terminal/transfer-history";
const HISTORY_VERSION = 2;
const enqueueHistoryWrite = createSerialQueue();
const FINISHED_STATUSES = new Set(["completed", "failed", "canceled", "paused"]);
const ACTIVE_STATUSES = new Set(["queued", "planning", "running", "canceling", "pausing"]);

// 旧格式没有连接标识，也无法证明 20 条路径是否被截断，只迁移展示数据，不猜测续传目标。
export async function loadTransferHistory() {
  let stored = await get(HISTORY_KEY, historyStore);
  if (stored === undefined) {
    const legacy = localStorage.getItem(LEGACY_KEY);
    const tasks = legacy ? JSON.parse(legacy) : [];
    if (!Array.isArray(tasks)) throw new Error("Invalid legacy transfer history");
    stored = { version: HISTORY_VERSION, tasks: tasks.map((task) => ({ ...task, resumeMetadataVersion: 0 })) };
    await set(HISTORY_KEY, stored, historyStore);
    // IndexedDB 提交成功才移除旧记录，迁移失败时仍可重试。
    localStorage.removeItem(LEGACY_KEY);
  }
  if (stored?.version !== HISTORY_VERSION || !Array.isArray(stored.tasks)) {
    throw new Error("Unsupported transfer history format");
  }
  return stored.tasks
    .filter((task) => task && typeof task.id === "string"
      && ["upload", "download"].includes(task.direction)
      && (FINISHED_STATUSES.has(task.status) || ACTIVE_STATUSES.has(task.status)))
    .map((task) => ({
      ...task,
      // 重启只恢复可手动续传的记录，绝不自动执行磁盘或远端写入。中断不是用户取消，
      // 记成失败才能续传；旧版本把中断记成了 canceled，一并改过来。
      status: ACTIVE_STATUSES.has(task.status) || isLegacyInterruption(task) ? "failed" : task.status,
      error: ACTIVE_STATUSES.has(task.status)
        ? { code: "transfer.interrupted", message: "Transfer interrupted by application exit" }
        : task.error,
      backendTaskId: null,
      speedBytesPerSecond: 0,
      currentPath: null,
      conflictPrompted: true,
    }));
}

function isLegacyInterruption(task) {
  return task.status === "canceled" && task.error?.code === "transfer.interrupted";
}

// 只缩减运行态字段，完整保存原始选择；请求和响应按顺序落盘，避免晚完成的旧写入覆盖新历史。
export function saveTransferHistory(tasks) {
  const snapshot = { version: HISTORY_VERSION, tasks: tasks.map(serializeTask) };
  return enqueueHistoryWrite(() => set(HISTORY_KEY, snapshot, historyStore));
}

function serializeTask(task) {
  const error = task.error ? toAppError(task.error) : null;
  return {
    id: task.id,
    sessionId: task.sessionId ?? null,
    sessionName: task.sessionName ?? null,
    profileId: task.profileId ?? null,
    workspaceDir: task.workspaceDir ?? null,
    connection: task.connection ? connectionIdentity(task.connection) : null,
    resumeMetadataVersion: task.resumeMetadataVersion ?? 0,
    sourceCount: task.sourceCount ?? null,
    direction: task.direction,
    status: task.status,
    label: task.label,
    rootLabel: task.rootLabel ?? task.label,
    bytesTransferred: task.bytesTransferred ?? 0,
    bytesTotal: task.bytesTotal ?? 0,
    filesDone: task.filesDone ?? 0,
    filesTotal: task.filesTotal ?? 0,
    error: error ? { ...error, params: { ...error.params } } : null,
    createdAt: task.createdAt,
    updatedAt: task.updatedAt,
    conflictPolicy: task.conflictPolicy ?? null,
    localDir: task.localDir ?? null,
    remoteDir: task.remoteDir ?? null,
    localPaths: Array.isArray(task.localPaths) ? [...task.localPaths] : [],
    remoteEntries: Array.isArray(task.remoteEntries) ? task.remoteEntries.map((entry) => ({
      path: entry.path,
      name: entry.name,
      isDirectory: entry.isDirectory,
      size: entry.size ?? null,
    })) : [],
    partialFiles: Array.isArray(task.partialFiles) ? task.partialFiles.map((file) => ({ ...file })) : [],
  };
}
