import { openUrl } from "@tauri-apps/plugin-opener";
import { useToast } from "@nuxt/ui/composables";
import { useI18n } from "vue-i18n";
import { resolveRemoteHome, statRemotePath } from "../services/workspaceApi";
import { useEditorStore } from "../stores/editorStore";
import { useFileExplorerStore } from "../stores/fileExplorerStore";
import { useSessionStore } from "../stores/sessionStore";
import { useSettingsStore } from "../stores/settingsStore";
import { useUiStore } from "../stores/uiStore";
import { localizeError } from "../utils/appError";
import { normalizeRemotePath } from "../utils/remotePath";
import { TERMINAL_CWD_STATUS } from "../utils/terminalCwd";

// 这些端口上一般不是网页：转发好就行，不替用户打开浏览器
const NON_HTTP_PORTS = new Set([
  21, 22, 23, 25, 53, 110, 143, 389, 445, 465, 587, 636, 993, 995,
  1433, 1521, 2181, 3306, 3389, 5432, 5672, 5900, 6379, 9092, 11211, 27017,
]);

// 终端里可点的东西点了之后做什么：路径 → 文件就进编辑器并跳到那一行、目录就让文件面板跳过去；
// 端口 → 在这条 SSH 连接上转发到本机，像网页就顺手在浏览器打开。全程只读 SFTP 和端口转发，不往终端里敲命令。
export function useTerminalLinks() {
  const { t } = useI18n();
  const toast = useToast();
  const sessionStore = useSessionStore();
  const editorStore = useEditorStore();
  const fileExplorerStore = useFileExplorerStore();
  const settingsStore = useSettingsStore();
  const uiStore = useUiStore();

  // promptDir：这段输出上方那个提示符里写的目录（完整路径的提示符才有）。
  // 相对路径要按「输出产生时」所在的目录解析，翻到上面的旧输出点开也对；没有就退回终端当前目录
  async function openPath(session, link, { promptDir = null } = {}) {
    if (!canReachTerminalHost(session)) return;
    try {
      const path = await resolveLinkPath(session, link.path, promptDir);
      if (!path) {
        notifyFailure(t("terminal.links.unknownCwd"));
        return;
      }
      const entry = await statRemotePath(session.id, path);
      if (entry.isDirectory) {
        showFilePane();
        fileExplorerStore.loadDirectory(session, entry.path);
        return;
      }
      editorStore.openFile(session, entry, { line: link.line, column: link.column });
    } catch (error) {
      notifyFailure(t("terminal.links.openFailed", { path: link.path }), error);
    }
  }

  // scheme / path：从 URL 点进来时（https://localhost:8443/admin），转发后原样拼回去
  async function openPort(session, remotePort, { scheme = "http", path = "/" } = {}) {
    if (!canReachTerminalHost(session)) return;
    try {
      const forward = await sessionStore.forwardPort(session.id, remotePort);
      const opensBrowser = !NON_HTTP_PORTS.has(remotePort);
      if (opensBrowser) await openUrl(`${scheme}://localhost:${forward.localPort}${path}`);
      toast.add({
        title: t("terminal.links.forwarded", { remote: remotePort, local: forward.localPort }),
        description: opensBrowser ? undefined : t("terminal.links.forwardedHint", { local: forward.localPort }),
        icon: "i-lucide-arrow-right-left",
        color: "success",
      });
    } catch (error) {
      notifyFailure(t("terminal.links.forwardFailed", { port: remotePort }), error);
    }
  }

  // 终端里 ssh 到了另一台机器：打印出来的路径和端口都在那台上，这条连接够不着
  function canReachTerminalHost(session) {
    if (session?.status !== "connected") return false;
    if (session.terminalCwd?.status !== TERMINAL_CWD_STATUS.elsewhere) return true;
    notifyFailure(t("terminal.links.elsewhere"));
    return false;
  }

  async function resolveLinkPath(session, rawPath, promptDir) {
    if (rawPath.startsWith("/")) return normalizeRemotePath(rawPath);
    if (rawPath.startsWith("~")) return expandHome(session, rawPath);

    const following = session.terminalCwd?.status === TERMINAL_CWD_STATUS.following;
    const base = promptDir ?? (following ? session.terminalCwd.path : null);
    if (!base) return null;
    const absoluteBase = base.startsWith("~") ? await expandHome(session, base) : base;
    return normalizeRemotePath(`${absoluteBase}/${rawPath}`);
  }

  async function expandHome(session, path) {
    const home = await resolveRemoteHome(session.id);
    return normalizeRemotePath(`${home}/${path.slice(1)}`);
  }

  function showFilePane() {
    if (uiStore.isWorkspaceImmersive) uiStore.toggleWorkspaceImmersive(false);
    if (!settingsStore.showFileExplorer) settingsStore.toggleFileExplorer();
  }

  function notifyFailure(title, error) {
    toast.add({
      title,
      description: error ? localizeError(error) : undefined,
      icon: "i-lucide-circle-alert",
      color: "error",
    });
  }

  return { openPath, openPort };
}
