const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

// 首页「最后连接」相对时间：入参为 epoch 毫秒字符串（与后端 now_stamp 一致）
export function formatRelativeTime(epochMs, t) {
  const timestamp = Number(epochMs);
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    return t("home.time.never");
  }

  const elapsed = Date.now() - timestamp;
  if (elapsed < MINUTE_MS) return t("home.time.justNow");
  if (elapsed < HOUR_MS) return t("home.time.minutesAgo", { count: Math.floor(elapsed / MINUTE_MS) });
  if (elapsed < DAY_MS) return t("home.time.hoursAgo", { count: Math.floor(elapsed / HOUR_MS) });

  return t("home.time.daysAgo", { count: Math.floor(elapsed / DAY_MS) });
}
