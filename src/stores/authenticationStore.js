import { defineStore } from "pinia";
import { Channel } from "@tauri-apps/api/core";
import { respondAuthentication } from "../services/authenticationApi";
import { errorCode, localizeError } from "../utils/appError";
import { useUiStore } from "./uiStore";

// 通道与生命周期闭包不进入响应式状态，store 只保留待展示的问题，答案由弹窗短暂持有。
const flows = new Map();

export const useAuthenticationStore = defineStore("authentication", {
  state: () => ({ challenges: [] }),
  getters: {
    activeChallenge: (state) => state.challenges[0] ?? null,
  },
  actions: {
    // 每个启动请求独享通道；关闭的请求即使迟到提问，也只能收到取消响应。
    createFlow(owner, target) {
      this.cancelOwner(owner);
      const flowId = crypto.randomUUID();
      const channel = new Channel();
      let closed = false;
      const finish = (cancel = false) => {
        closed = true;
        const pending = this.challenges.filter((challenge) => challenge.flowId === flowId);
        this.challenges = this.challenges.filter((challenge) => challenge.flowId !== flowId);
        if (flows.get(owner)?.id === flowId) flows.delete(owner);
        if (cancel) pending.forEach((challenge) => cancelChallenge(challenge.challengeId));
      };
      const flow = { id: flowId, channel, close: () => finish(true) };
      flows.set(owner, flow);
      channel.onmessage = (event) => {
        if (event.kind === "finished") {
          finish();
        } else if (event.kind === "dismissed") {
          this.removeChallenge(event.challengeId);
        } else if (event.kind === "challenge") {
          if (closed) {
            cancelChallenge(event.challengeId);
            return;
          }
          this.challenges.push({
            ...event, flowId, target: { ...target }, isSubmitting: false, error: null,
          });
        }
      };
      return flow;
    },
    cancelOwner(owner) {
      flows.get(owner)?.close();
    },
    removeChallenge(challengeId) {
      this.challenges = this.challenges.filter((challenge) => challenge.challengeId !== challengeId);
    },
    beginAnswer(challengeId) {
      const challenge = this.challenges.find((item) => item.challengeId === challengeId);
      if (!challenge || challenge.isSubmitting) return false;
      challenge.isSubmitting = true;
      challenge.error = null;
      return true;
    },
    finishAnswer(challengeId, error = null) {
      const challenge = this.challenges.find((item) => item.challengeId === challengeId);
      if (!challenge) return;
      if (!error || errorCode(error) === "auth.challengeExpired") {
        this.removeChallenge(challengeId);
      } else {
        challenge.error = localizeError(error);
        challenge.isSubmitting = false;
      }
    },
  },
});

// 答案不作为 Pinia action 参数，避免开发工具的 action 时间线记录密码或验证码。
export async function submitAuthenticationAnswers(challengeId, answers) {
  const store = useAuthenticationStore();
  if (!store.beginAnswer(challengeId)) return;
  try {
    await respondAuthentication(challengeId, answers);
    store.finishAnswer(challengeId);
  } catch (error) {
    store.finishAnswer(challengeId, error);
  }
}

function cancelChallenge(challengeId) {
  respondAuthentication(challengeId, null).catch((error) => {
    if (errorCode(error) !== "auth.challengeExpired") useUiStore().setError(error);
  });
}
