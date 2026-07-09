// 远程路径工具：store 与组件共用一份实现，行为与 Rust 端 sftp/path.rs 对齐
export const DEFAULT_REMOTE_PATH = "/";

export function normalizeRemotePath(path = DEFAULT_REMOTE_PATH) {
  const cleanPath = String(path || DEFAULT_REMOTE_PATH).trim().replaceAll("\\", "/");
  const parts = [];

  for (const part of cleanPath.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      parts.pop();
      continue;
    }
    parts.push(part);
  }

  return parts.length ? `/${parts.join("/")}` : DEFAULT_REMOTE_PATH;
}

export function parentRemotePath(path) {
  const currentPath = normalizeRemotePath(path);
  if (currentPath === DEFAULT_REMOTE_PATH) return currentPath;

  const parts = currentPath.split("/").filter(Boolean);
  parts.pop();

  return parts.length ? `/${parts.join("/")}` : DEFAULT_REMOTE_PATH;
}

export function joinRemotePath(base, child) {
  const cleanBase = normalizeRemotePath(base);
  const cleanChild = String(child || "").replace(/[\\/]/g, "_");

  return cleanBase === DEFAULT_REMOTE_PATH ? `/${cleanChild}` : `${cleanBase}/${cleanChild}`;
}
