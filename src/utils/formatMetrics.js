// 监控数值的统一格式化：概览栏、系统详情抽屉、状态栏共用，保证同一个数在三处长得一样。

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];

export function clampPercent(value) {
  return Math.min(Math.max(Number(value) || 0, 0), 100);
}

// 超过 80% 转琥珀、90% 转红：占用高才需要注意，平时保持品牌色不喧宾夺主
export function usageLevel(percent) {
  const value = Number(percent);
  if (value >= 90) return "critical";
  if (value >= 80) return "high";
  return "normal";
}

// 已用 + 缓存 + 空闲正好等于总量（后端按 available 拆的），分段条能严丝合缝铺满；
// 拿不到缓存（/proc/meminfo 缺 MemFree）返回 null，调用方退回单段
export function splitMemory(memory) {
  const { totalBytes, usedBytes, cacheBytes } = memory ?? {};
  if (!totalBytes || usedBytes == null || cacheBytes == null) return null;
  return { used: usedBytes, cache: cacheBytes, free: Math.max(totalBytes - usedBytes - cacheBytes, 0) };
}

export function formatPercent(value) {
  if (value === null || value === undefined) return "--";

  return `${Number(value).toFixed(Number(value) % 1 === 0 ? 0 : 1)}%`;
}

export function formatBytes(value) {
  if (value === null || value === undefined) return "--";

  let size = Math.max(Number(value) || 0, 0);
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${BYTE_UNITS[unitIndex]}`;
}

// 内存 / 磁盘容量用 GiB 两位小数（「1.12 / 4.00 GiB」），读数跳动时宽度不抖
export function formatGibibytes(value) {
  if (value === null || value === undefined) return "--";

  return (Math.max(Number(value) || 0, 0) / 1024 ** 3).toFixed(2);
}

export function formatBytesPerSecond(value) {
  if (value === null || value === undefined) return "--";

  let size = Math.max(Number(value) || 0, 0);
  if (size < 1024) return `${size.toFixed(0)} B/s`;

  const units = ["KB", "MB", "GB", "TB"];
  let unitIndex = 0;
  size /= 1024;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size < 10 ? 1 : 0)} ${units[unitIndex]}/s`;
}

export function formatDateTime(value) {
  if (!value) return "--";

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "--";

  const pad = (part) => String(part).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} `
    + `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

// 只要时分秒：采样都是当天的事，日期是噪音
export function formatClock(value, locale) {
  if (!value) return "--";

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "--";

  return new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  }).format(date);
}

// 精确到分钟：详情抽屉里用
export function formatUptime(seconds, t) {
  if (!seconds) return "--";

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return t("monitor.uptimeDays", { days, hours, minutes });
  if (hours > 0) return t("monitor.uptimeHours", { hours, minutes });
  return t("monitor.uptimeMinutes", { minutes });
}

// 只取最大的两级（「4 天 20 小时」）：概览卡片空间小，分钟级的跳动也没有意义
export function formatUptimeShort(seconds, t) {
  if (!seconds) return "--";

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return t("monitor.uptimeShortDays", { days, hours });
  if (hours > 0) return t("monitor.uptimeShortHours", { hours, minutes });
  return t("monitor.uptimeMinutes", { minutes });
}

// ps 的 args 带完整路径和参数；列表里只要一眼认出是谁，取可执行文件名
export function processName(command = "") {
  const trimmed = command.trim();
  // 内核线程的 args 是方括号包着的名字（[kworker/R-kstrp]），里面的斜杠不是路径分隔
  if (trimmed.startsWith("[")) return trimmed.replace(/^\[|\]$/g, "") || "--";
  const executable = trimmed.split(/\s+/)[0] ?? "";
  // 登录 shell 的 argv[0] 带前导 -（-bash），sshd 这类改写过进程标题的写成「sshd: root@pts/0」，
  // 这些标记不属于程序名
  return executable.split("/").pop().replace(/^-|:$/g, "") || trimmed || "--";
}
