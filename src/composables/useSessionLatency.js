import { onBeforeUnmount, onMounted, watch } from "vue";

const LATENCY_INTERVAL_MS = 10_000;

// 延迟读数：只测一个会话（默认当前会话，给状态栏；首页传 getSession 测它展示的那个），
// 窗口隐藏时停，切会话 / 重连后立刻测一次，每轮结束后再计时，网络卡住时不会叠出一串等待中的请求。
export function useSessionLatency(sessionStore, getSession = () => sessionStore.activeSession) {
  let timer = null;
  let generation = 0;

  function canMeasure() {
    const session = getSession();
    return document.visibilityState !== "hidden" && session?.status === "connected" && !session.isClosing;
  }

  async function measure(currentGeneration) {
    if (currentGeneration !== generation || !canMeasure()) return;
    try {
      await sessionStore.measureLatency(getSession().id);
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

  watch(() => {
    const session = getSession();
    return [session?.id, session?.status, session?.currentAttemptId];
  }, restart);

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
