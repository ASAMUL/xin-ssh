// 远程路径工具：store 与组件共用一份实现，行为与 Rust 端 sftp/path.rs 对齐
export const DEFAULT_REMOTE_PATH = "/";

export function normalizeRemotePath(path = DEFAULT_REMOTE_PATH) {
  const cleanPath = String(path || DEFAULT_REMOTE_PATH);
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
  const cleanChild = String(child || "").replaceAll("/", "_");

  return cleanBase === DEFAULT_REMOTE_PATH ? `/${cleanChild}` : `${cleanBase}/${cleanChild}`;
}

// POSIX shell 单引号包裹，路径里的单引号转义为 '\''（与 Rust 端 util::shell_quote 对齐）
export function quoteRemotePathForShell(path) {
  return `'${String(path).replaceAll("'", `'\\''`)}'`;
}

// 拖进终端的几个路径拼成一段命令行参数：普通路径原样，带空格、引号这类字符的才加引号——
// 插进命令行的东西要一眼能读；末尾留一个空格，接着敲下一个参数
export function formatPathsForShell(paths) {
  const words = paths.map((path) => (/^[\w@%+=:,./~-]+$/.test(path) ? path : quoteRemotePathForShell(path)));
  return words.length ? `${words.join(" ")} ` : "";
}
