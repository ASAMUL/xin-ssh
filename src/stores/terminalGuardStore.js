import { defineStore } from "pinia";

// 生产机保护的确认框：谁要确认就 ask 一下拿到 Promise，TerminalGuardDialog 负责显示并 answer。
// 同一时间只有一个确认框；新请求进来时，旧的那个当作取消——
// 宁可让用户再按一次回车，也不能让一个没人看见的确认框替他放行。
export const useTerminalGuardStore = defineStore("terminalGuard", {
  state: () => ({
    // { kind: "command" | "paste", sessionName, command?, danger?, lines?, resolve }
    request: null,
  }),
  actions: {
    ask(request) {
      this.request?.resolve(false);
      return new Promise((resolve) => {
        this.request = { ...request, resolve };
      });
    },
    answer(confirmed) {
      const request = this.request;
      this.request = null;
      request?.resolve(Boolean(confirmed));
    },
  },
});
