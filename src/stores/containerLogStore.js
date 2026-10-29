import { defineStore } from "pinia";
import { useSessionStore } from "./sessionStore";

// 回看行数的上限与后端 MAX_LOG_TAIL_LINES 一致
export const CONTAINER_LOG_TAIL_OPTIONS = [200, 1000, 5000, 10000];
// 0 = 不限时间，只按行数回看
export const CONTAINER_LOG_SINCE_OPTIONS = [
  { value: 0, labelKey: "containerLogs.sinceAll" },
  { value: 10 * 60, labelKey: "containerLogs.since10m" },
  { value: 3600, labelKey: "containerLogs.since1h" },
  { value: 6 * 3600, labelKey: "containerLogs.since6h" },
  { value: 24 * 3600, labelKey: "containerLogs.since24h" },
  { value: 7 * 24 * 3600, labelKey: "containerLogs.since7d" },
];

// 只记「看哪台机器的哪个容器」和回看选项；日志流跟着窗口里的 xterm 走，关窗即停，不进 store。
// 回看选项留在这里而不是窗口里：换个容器看时，通常还想用刚才那套条件
export const useContainerLogStore = defineStore("containerLog", {
  state: () => ({
    isOpen: false,
    sessionId: null,
    container: null,
    tail: 1000,
    sinceSeconds: 0,
    timestamps: false,
  }),
  getters: {
    session(state) {
      return useSessionStore().findSession(state.sessionId);
    },
  },
  actions: {
    open(session, container) {
      if (!session || !container?.id) return;
      this.sessionId = session.id;
      this.container = container;
      this.isOpen = true;
    },
    close() {
      this.isOpen = false;
    },
  },
});
