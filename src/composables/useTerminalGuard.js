import { useProfileStore } from "../stores/profileStore";
import { useTerminalGuardStore } from "../stores/terminalGuardStore";
import { findDangerousCommand, pastedLines } from "../utils/dangerousCommands";

// 生产机保护：只对标记成「生产」的连接生效，全在客户端拦，不碰服务器。
// confirm* 入口返回 Promise<boolean>：true = 放行（不是生产机、没命中、或者用户点了确认）。
// 生产标记每次现查连接配置，而不是复制到会话上：开着的会话里改了标记也立刻生效。
export function useTerminalGuard() {
  const profileStore = useProfileStore();
  const guardStore = useTerminalGuardStore();

  function isProduction(session) {
    return Boolean(session && profileStore.isProductionProfile(session.profileId));
  }

  // 同步判断要不要拦：返回 null 放行，否则是交给确认框的请求。
  // commands 是同一条命令的几种读法（终端里认不准提示符在哪结束），任何一种命中都拦——
  // 误报的代价只是多点一下确认。同步是因为终端得当场决定这个回车扣不扣下
  function inspectCommand(session, commands) {
    if (!isProduction(session)) return null;
    for (const command of commands) {
      const danger = findDangerousCommand(command);
      if (danger) return { kind: "command", sessionName: session.name, command: command.trim(), danger };
    }
    return null;
  }

  function confirm(request) {
    return guardStore.ask(request);
  }

  // 片段执行之前：命中危险规则才弹确认
  async function confirmCommand(session, command) {
    const request = inspectCommand(session, [command]);
    return request ? confirm(request) : true;
  }

  // 粘贴（以及片段一次送进好几行）之前：带换行就弹确认，预览会被执行的每一行
  async function confirmPaste(session, text) {
    if (!isProduction(session)) return true;
    const lines = pastedLines(text);
    if (!lines) return true;
    return guardStore.ask({ kind: "paste", sessionName: session.name, lines });
  }

  return { isProduction, inspectCommand, confirm, confirmCommand, confirmPaste };
}
