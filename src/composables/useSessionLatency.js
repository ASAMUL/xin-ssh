import { onBeforeUnmount, onMounted, watch } from "vue";

const LATENCY_INTERVAL_MS = 10_000;

// 状态栏的延迟读数：只测当前会话，窗口隐藏时停，切会话 / 重连后立刻测一次，
// 每轮结束后再计时，网络卡住时不会叠出一串等待中的请求。
export function useSessionLatency(sessionStore) {
  let timer = null;
  let generation = 0;

  function canMeasure() {
    const session = sessionStore.activeSession;
    return document.visibilityState !== "hidden" && session?.status === "connected" && !session.isClosing;
  }

  async function measure(currentGeneration) {
    if (currentGeneration !== generation || !canMeasure()) return;
    try {
      await sessionStore.measureLatency(sessionStore.activeSessionId);
    } finally {
      if (currentGeneration === generation && canMeasure()) {
        timer = window.setTimeout(() => measure(currentGeneration), LATENCY_INTERVAL_MS);
      }
    }
  }

  function restart() {
    generation += 1;
    window.clearTimeout(timer);
    timer = null;
    if (canMeasure()) void measure(generation);
  }

  watch(() => [
    sessionStore.activeSessionId,
    sessionStore.activeSession?.status,
    sessionStore.activeSession?.currentAttemptId,
  ], restart);

  onMounted(() => {
    document.addEventListener("visibilitychange", restart);
    restart();
  });

  onBeforeUnmount(() => {
    generation += 1;
    window.clearTimeout(timer);
    document.removeEventListener("visibilitychange", restart);
  });
}
