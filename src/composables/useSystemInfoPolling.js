import { onBeforeUnmount, onMounted, watch } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// 每轮完成后再计时，切会话或隐藏时废弃旧调度，避免慢采样叠加和旧回调重启定时器。
export function useSystemInfoPolling(sessionStore, settingsStore) {
  const appWindow = isTauri() ? getCurrentWindow() : null;
  let timer = null;
  let generation = 0;
  let mounted = false;
  let visibilityErrorReported = false;

  function canPoll() {
    const session = sessionStore.activeSession;
    return mounted && document.visibilityState !== "hidden"
      && session?.status === "connected" && !session.isClosing;
  }

  async function windowIsVisible() {
    if (!appWindow) return true;
    try {
      const [visible, minimized] = await Promise.all([appWindow.isVisible(), appWindow.isMinimized()]);
      visibilityErrorReported = false;
      return visible && !minimized;
    } catch (error) {
      if (!visibilityErrorReported) console.warn("[Xin SSH] window visibility unavailable", error);
      visibilityErrorReported = true;
      return false;
    }
  }

  async function poll(currentGeneration) {
    const session = sessionStore.activeSession;
    if (currentGeneration !== generation || !canPoll()) return;
    try {
      const visible = await windowIsVisible();
      if (visible && currentGeneration === generation && canPoll()
        && sessionStore.activeSessionId === session.id) {
        await sessionStore.refreshSystemInfo(session.id);
      }
    } finally {
      // 某些 WebView 不随原生窗口最小化发送 visibilitychange，保留轻量本地检查以恢复采样。
      if (currentGeneration === generation && canPoll()) {
        const seconds = Number(settingsStore.monitorRefreshSeconds) || 5;
        timer = window.setTimeout(() => poll(currentGeneration), Math.max(seconds, 2) * 1000);
      }
    }
  }

  function stop() {
    generation += 1;
    window.clearTimeout(timer);
    timer = null;
  }

  function refresh() {
    stop();
    if (canPoll()) void poll(generation);
  }

  watch(() => [
    sessionStore.activeSessionId,
    sessionStore.activeSession?.currentAttemptId,
    sessionStore.activeSession?.status,
    sessionStore.activeSession?.isClosing,
    settingsStore.monitorRefreshSeconds,
  ], refresh);

  onMounted(() => {
    mounted = true;
    document.addEventListener("visibilitychange", refresh);
    window.addEventListener("focus", refresh);
    refresh();
  });

  onBeforeUnmount(() => {
    mounted = false;
    stop();
    document.removeEventListener("visibilitychange", refresh);
    window.removeEventListener("focus", refresh);
  });

  return { refresh };
}
