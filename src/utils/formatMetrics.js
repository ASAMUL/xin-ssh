// 监控数值的统一格式化：概览栏、系统详情抽屉、状态栏共用，保证同一个数在三处长得一样。

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];

export function clampPercent(value) {
  return Math.min(Math.max(Number(value) || 0, 0), 100);
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
