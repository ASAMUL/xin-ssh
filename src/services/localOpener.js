import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";

// 传输记录相关的本地路径打开能力，统一封装 tauri-plugin-opener

function joinLocalPath(dir, name) {
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.replace(/[\\/]+$/, "") + sep + name;
}

// 下载任务对应的确切本地路径；仅单条目任务能推断，多条目返回 null
export function taskLocalFilePath(task) {
  if (task?.direction !== "download" || !task.localDir) return null;

  const entries = task.remoteEntries || [];
  if (entries.length !== 1 || !entries[0]?.name) return null;

  return joinLocalPath(task.localDir, entries[0].name);
}

export function canOpenTaskLocally(task) {
  return task?.direction === "download" && task.status === "completed" && Boolean(task.localDir);
}

// 打开下载产物本身；冲突自动改名等场景推断的文件名可能不存在，退化为打开目录
export async function openTaskFile(task) {
  if (!canOpenTaskLocally(task)) return;

  const filePath = taskLocalFilePath(task);
  if (filePath) {
    try {
      await openPath(filePath);
      return;
    } catch {
      // fall through
    }
  }

  await openPath(task.localDir);
}

// 在系统文件管理器中定位下载产物；失败时退化为直接打开下载目录
export async function revealTaskInFolder(task) {
  if (!canOpenTaskLocally(task)) return;

  const filePath = taskLocalFilePath(task);
  if (filePath) {
    try {
      await revealItemInDir(filePath);
      return;
    } catch {
      // fall through
    }
  }

  await openPath(task.localDir);
}
