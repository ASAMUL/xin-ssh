import { useProfileStore } from "../stores/profileStore";
import { useTerminalGuardStore } from "../stores/terminalGuardStore";
import { findDangerousCommand, pastedLines } from "../utils/dangerousCommands";

// 生产机保护：只对标记成「生产」的连接生效，全在客户端拦，不碰服务器。
// 两个入口都返回 Promise<boolean>：true = 放行（不是生产机、没命中、或者用户点了确认）。
// 生产标记每次现查连接配置，而不是复制到会话上：开着的会话里改了标记也立刻生效。
export function useTerminalGuard() {
  const profileStore = useProfileStore();
  const guardStore = useTerminalGuardStore();

  function isProduction(session) {
    return Boolean(session && profileStore.isProductionProfile(session.profileId));
  }

  // 回车执行之前：命中危险规则才弹确认
  async function confirmCommand(session, command) {
    if (!isProduction(session)) return true;
    const danger = findDangerousCommand(command);
    if (!danger) return true;
    return guardStore.ask({ kind: "command", sessionName: session.name, command: command.trim(), danger });
  }

  // 粘贴（以及片段一次送进好几行）之前：带换行就弹确认，预览会被执行的每一行
  async function confirmPaste(session, text) {
    if (!isProduction(session)) return true;
    const lines = pastedLines(text);
    if (!lines) return true;
    return guardStore.ask({ kind: "paste", sessionName: session.name, lines });
  }

  return { isProduction, confirmCommand, confirmPaste };
}
