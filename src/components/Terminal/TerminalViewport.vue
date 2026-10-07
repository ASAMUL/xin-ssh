<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { openUrl } from "@tauri-apps/plugin-opener";
import TerminalSearchPanel from "./TerminalSearchPanel.vue";
import { buildSearchDecorations, buildXtermTheme } from "../../config/terminalThemes";
import { loadCommandHistory, recordCommand, suggestCommand } from "../../services/commandHistory";
import { resolveRemoteHome } from "../../services/workspaceApi";
import { useTerminalGuard } from "../../composables/useTerminalGuard";
import { useTerminalLinks } from "../../composables/useTerminalLinks";
import { createCommandMarks } from "../../utils/commandMarks";
import { createCommandSuggest } from "../../utils/commandSuggest";
import { createTerminalCwd } from "../../utils/terminalCwd";
import { historySearchCommand } from "../../utils/dangerousCommands";
import { registerTerminalLinks } from "../../utils/terminalLinkProvider";
import { parseShellPrompt, readTerminalRow } from "../../utils/shellPrompt";
import { forgetWebgl, touchWebgl } from "../../utils/webglPool";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSnippetStore } from "../../stores/snippetStore";
import { useUiStore } from "../../stores/uiStore";

const props = defineProps({
  sessionId: { type: String, required: true },
  revision: { type: Number, required: true },
  active: { type: Boolean, default: false },
  // 已按明暗解析好的那一版调色板（resolveThemeVariant 的结果）
  theme: { type: Object, required: true },
  material: { type: String, required: true },
  fontFamily: { type: String, required: true },
  fontSize: { type: Number, required: true },
});

const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const snippetStore = useSnippetStore();
const uiStore = useUiStore();
const terminalGuard = useTerminalGuard();
const terminalLinks = useTerminalLinks();
const { t } = useI18n();
// 链接上的修饰键在 Mac 上是 ⌘
const LINK_MODIFIER = /Mac/i.test(navigator.platform) ? "⌘" : "Ctrl";
// URL 指向这些主机时，说的是服务器自己，不是本机
const LOOPBACK_HOSTS = new Set(["localhost", "0.0.0.0", "[::1]", "[::]"]);
const session = computed(() => sessionStore.findSession(props.sessionId));
// 后台终端仍需向原会话回复光标/设备查询；不能因不可见而关闭整个输入通道。
const canSendData = computed(() => session.value?.status === "connected"
  && session.value.terminalRevision === props.revision);
const viewportRoot = ref(null);
const terminalMount = ref(null);
// 悬停在路径 / 端口上时的提示：{ text, x, y }，相对终端区域
const linkHint = ref(null);
const searchAddon = shallowRef(null);
const searchPanel = ref(null);
const isSearchOpen = ref(false);
const searchDecorations = computed(() => buildSearchDecorations(props.theme));
// 片段也是联想的候选：新服务器上什么都没敲过时先由它们顶上；多行脚本没法补进一行里，排除
const snippetCommands = computed(() => snippetStore.activeSnippets
  .map((snippet) => snippet.command)
  .filter((command) => command && !command.includes("\n")));
let terminal = null;
let fitAddon = null;
let webglAddon = null;
let webglContextListener = null;
let webglUnavailable = false;
let resizeObserver = null;
let commandMarks = null;
let commandSuggest = null;
let terminalCwd = null;
let detachSink = null;
let linkProvider = null;
let layoutFrame = null;
let disposed = false;
// 上一次真正设到 xterm 上的外观；切回这个终端时外观多半没变，没变就不重设
let appliedAppearance = null;
const webglHolder = { release: releaseWebgl };
const listeners = [];

// 实例只在会话关闭、清屏或重连时销毁；隐藏期间仍解析输出，保留光标和 alternate screen。
onMounted(() => {
  terminal = new Terminal({
    allowProposedApi: true,
    allowTransparency: true,
    cursorBlink: true,
    fontFamily: props.fontFamily,
    fontSize: props.fontSize,
    lineHeight: 1.18,
    scrollback: 8000,
    cols: session.value?.terminalColumns ?? 120,
    rows: session.value?.terminalRows ?? 32,
    disableStdin: !canSendData.value,
    theme: buildXtermTheme(props.theme, props.material),
  });
  fitAddon = new FitAddon();
  searchAddon.value = new SearchAddon();
  terminal.loadAddon(fitAddon);
  terminal.loadAddon(searchAddon.value);
  terminal.loadAddon(new Unicode11Addon());
  terminal.unicode.activeVersion = "11";
  terminal.loadAddon(new WebLinksAddon(openTerminalLink));
  terminal.open(terminalMount.value);
  linkProvider = registerTerminalLinks(terminal, {
    activate: activateLink,
    hover: showLinkHint,
    leave: () => { linkHint.value = null; },
  });
  // 终端目录跟着这个终端实例走：重连会换一个新终端，旧的目录记录随之作废
  terminalCwd = createTerminalCwd(terminal, {
    loginUser: session.value?.username ?? null,
    resolveLoginHome: () => resolveRemoteHome(props.sessionId),
    onChange: (cwd, { navigate }) => sessionStore.updateTerminalCwd(props.sessionId, props.revision, cwd, { navigate }),
  });
  commandMarks = createCommandMarks(terminal, {
    onCommand: handleCommand,
    onPrompt: (prompt) => terminalCwd.handlePrompt(prompt),
    knownUsers: () => [session.value?.username].filter(Boolean),
  });
  commandMarks.setTheme(props.theme);
  commandSuggest = createCommandSuggest(terminal, {
    suggest: (input) => suggestCommand(session.value?.profileId ?? null, input, snippetCommands.value),
  });
  commandSuggest.setTheme(props.theme);
  commandSuggest.setEnabled(settingsStore.commandSuggestions);
  loadCommandHistory();
  if (settingsStore.commandSuggestions) ensureSnippetsLoaded();
  terminal.attachCustomKeyEventHandler(handleTerminalKey);
  // 捕获阶段：赶在 xterm 自己的粘贴处理之前，生产机上多行粘贴先确认
  terminalMount.value.addEventListener("paste", guardPaste, true);
  listeners.push(
    terminal.onData((data) => {
      if (canSendData.value) {
        commandMarks.handleInput(data);
        commandSuggest.handleInput(data);
        sessionStore.sendSessionTerminalInput(props.sessionId, data).catch((error) => uiStore.setError(error));
      }
    }),
    terminal.onResize(({ cols, rows }) => {
      if (session.value?.terminalRevision === props.revision) {
        sessionStore.resizeSessionTerminal(props.sessionId, cols, rows);
      }
    }),
  );
  detachSink = sessionStore.attachTerminalSink(props.sessionId, props.revision, (data) => terminal.write(data));
  resizeObserver = new ResizeObserver(scheduleLayout);
  resizeObserver.observe(terminalMount.value);
  updateAppearance();
  document.fonts?.ready.then(scheduleLayout);
});

onBeforeUnmount(() => {
  disposed = true;
  cancelAnimationFrame(layoutFrame);
  terminalMount.value?.removeEventListener("paste", guardPaste, true);
  detachSink?.();
  linkProvider?.dispose();
  resizeObserver?.disconnect();
  listeners.forEach((listener) => listener.dispose());
  releaseWebgl();
  commandMarks?.dispose();
  commandSuggest?.dispose();
  terminalCwd?.dispose();
  terminal?.dispose();
});

watch(() => settingsStore.commandSuggestions, (enabled) => {
  commandSuggest?.setEnabled(enabled);
  if (enabled) ensureSnippetsLoaded();
});
watch(canSendData, (enabled) => {
  if (!terminal) return;
  terminal.options.disableStdin = !enabled;
  // 握手期间 fit 只更新本地记录，连上后必须再送一次，远端 PTY 才能追上当前尺寸。
  if (enabled) sessionStore.resizeSessionTerminal(props.sessionId, terminal.cols, terminal.rows);
}, { flush: "sync" });
watch(() => props.active, (active) => {
  if (active) updateAppearance();
  else {
    closeSearch(false);
    terminal?.blur();
  }
}, { flush: "post" });
watch(() => [props.theme, props.material, props.fontFamily, props.fontSize], updateAppearance);

// 隐藏节点的尺寸为零，不能拿来覆盖 PTY 大小；字体完成加载后再量一次可见区域。
function scheduleLayout() {
  if (disposed || !props.active || layoutFrame !== null) return;
  layoutFrame = requestAnimationFrame(() => {
    layoutFrame = null;
    const mount = terminalMount.value;
    if (disposed || !props.active || !mount?.clientWidth || !mount.clientHeight) return;
    fitAddon.fit();
    updateRenderer();
  });
}

// 切换终端时每次都会走到这里。重设主题不便宜：xterm 会清空字形图集整屏重画，
// 命令标记也会把整个回滚区的提示符装饰撤掉重建——所以外观真变了才设。
function updateAppearance() {
  if (!terminal || !props.active || disposed) return;
  const next = [props.theme, props.material, props.fontFamily, props.fontSize];
  if (!appliedAppearance || next.some((value, index) => value !== appliedAppearance[index])) {
    appliedAppearance = next;
    terminal.options.theme = buildXtermTheme(props.theme, props.material);
    commandMarks.setTheme(props.theme);
    commandSuggest.setTheme(props.theme);
    terminal.options.fontFamily = props.fontFamily;
    terminal.options.fontSize = props.fontSize;
  }
  scheduleLayout();
  document.fonts?.ready.then(scheduleLayout);
}

// WebGL 上下文由 webglPool 统一分配：最近看过的几个终端留着，切回来不用重建
function updateRenderer() {
  if (!props.active) return;
  if (webglAddon) {
    touchWebgl(webglHolder);
    return;
  }
  if (webglUnavailable) return;
  let addon = null;
  try {
    addon = new WebglAddon();
    terminal.loadAddon(addon);
    webglAddon = addon;
    touchWebgl(webglHolder);
    webglContextListener = addon.onContextLoss(() => {
      webglUnavailable = true;
      releaseWebgl();
    });
  } catch (error) {
    // WebView/GPU 不支持时终端仍需可用，该实例后续保持 DOM 渲染。
    webglUnavailable = true;
    addon?.dispose();
      console.warn("[Xin SSH] WebGL unavailable, using DOM renderer", error);
  }
}

function releaseWebgl() {
  forgetWebgl(webglHolder);
  webglContextListener?.dispose();
  webglContextListener = null;
  const addon = webglAddon;
  webglAddon = null;
  addon?.dispose();
}

function focus() {
  if (props.active && !isSearchOpen.value && session.value?.status !== "authRequired") terminal?.focus();
}

function handleTerminalKey(event) {
  if (!props.active) return false;
  if (event.type === "keydown") commandMarks.noteKeystroke();
  if (event.type === "keydown" && acceptSuggestion(event)) return false;
  if (event.type === "keydown" && guardEnter(event)) return false;
  if (event.type !== "keydown" || !event.ctrlKey || event.altKey || event.metaKey) return true;
  const key = event.key.toLowerCase();
  // Ctrl+↑ / Ctrl+↓ 在执行过的命令之间跳；vim 这类全屏程序里不拦，原样交给它
  if ((key === "arrowup" || key === "arrowdown") && !event.shiftKey
    && commandMarks.jump(key === "arrowup" ? -1 : 1)) {
    event.preventDefault();
    return false;
  }
  if (key === "f" && event.shiftKey) {
    event.preventDefault();
    openSearch();
    return false;
  }
  if (key === "c" && terminal.hasSelection()) {
    event.preventDefault();
    copySelection();
    return false;
  }
  // 保留浏览器的原生 paste 事件，交给 xterm 处理 bracketed paste。
  return key !== "v";
}

// → / End 采纳联想：补上的那截走 terminal.input，和用户亲手键入走同一条路发给服务器
function acceptSuggestion(event) {
  const plain = !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey;
  if (!plain || (event.key !== "ArrowRight" && event.key !== "End")) return false;
  const suffix = commandSuggest.accept();
  if (!suffix) return false;
  event.preventDefault();
  terminal.input(suffix);
  return true;
}

// 生产机上回车之前先读出这一行要执行的命令，命中危险规则就先不发回车，弹确认；
// 点了确认再补发回车，取消就什么都不发，命令留在原处可以改。没命中的回车原样放过去。
// vim / top 这类全屏程序里不拦（那里的回车不是在执行命令），输入法选字时的回车也不拦。
function guardEnter(event) {
  const plain = !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey;
  if (!plain || event.key !== "Enter" || event.isComposing) return false;
  if (!terminalGuard.isProduction(session.value) || terminal.buffer.active.type !== "normal") return false;

  const request = terminalGuard.inspectCommand(session.value, readCommandCandidates());
  if (!request) return false;
  event.preventDefault();
  terminalGuard.confirm(request).then((confirmed) => {
    if (disposed) return;
    if (confirmed && canSendData.value) terminal.input("\r");
    focus();
  });
  return true;
}

// 光标所在这一行要执行的命令（长命令折行时拼回整条）。提示符在哪结束不一定认得准，
// 所以按可信程度给出几种读法，生产机保护任何一种命中都拦：
//   ① 从开始按键的那一格往后读——不管提示符长什么样都准；
//   ② 认得出提示符（发行版默认样式）就去掉提示符；
//   ③ bash 的 Ctrl+R 把整行改写成了搜索框，取冒号后面那段；
//   ④ 整行——在 mysql 客户端里敲 drop database 靠它拦。
function readCommandCandidates() {
  const buffer = terminal.buffer.active;
  let start = buffer.baseY + buffer.cursorY;
  while (start > 0 && buffer.getLine(start)?.isWrapped) start -= 1;

  let text = "";
  let line = start;
  do {
    text += readTerminalRow(buffer, line)?.text ?? "";
    line += 1;
  } while (buffer.getLine(line)?.isWrapped);

  const first = readTerminalRow(buffer, start);
  const prompt = first && parseShellPrompt(first);
  const candidates = [
    commandMarks.readInput(),
    prompt && text.slice(prompt.end + 1),
    historySearchCommand(text),
    text,
  ];
  return [...new Set(candidates.map((command) => command?.trim()).filter(Boolean))];
}

function guardPaste(event) {
  if (!terminalGuard.isProduction(session.value)) return;
  const text = event.clipboardData?.getData("text/plain") ?? "";
  if (!text.includes("\n") && !text.includes("\r")) return;

  event.preventDefault();
  event.stopPropagation();
  terminalGuard.confirmPaste(session.value, text).then((confirmed) => {
    if (disposed) return;
    // terminal.paste 会按 shell 是否开了括号粘贴模式包好再发，和直接粘贴走同一条路
    if (confirmed && canSendData.value) terminal.paste(text);
    focus();
  });
}

// 关掉联想时也不记历史：用户关掉它，多半就是不希望本机留下命令记录
function handleCommand(command) {
  terminalCwd.handleCommand(command);
  if (settingsStore.commandSuggestions) recordCommand(session.value?.profileId ?? null, command);
}

function ensureSnippetsLoaded() {
  if (!snippetStore.loaded) snippetStore.loadSnippets();
}

async function copySelection() {
  const selection = terminal.getSelection();
  if (!selection) return;
  try {
    await navigator.clipboard.writeText(selection);
    if (!disposed && terminal.getSelection() === selection) terminal.clearSelection();
  } catch (error) {
    uiStore.setError(error);
  }
}

// 服务器上打印的 http://localhost:5173 指的是服务器自己：直接在本机打开会连到本机的 5173，
// 所以先把端口转发过来再打开
function openTerminalLink(event, uri) {
  event.preventDefault();
  const url = new URL(uri);
  if (!["http:", "https:"].includes(url.protocol)) return;
  if (LOOPBACK_HOSTS.has(url.hostname) || url.hostname.startsWith("127.")) {
    const port = Number(url.port) || (url.protocol === "https:" ? 443 : 80);
    terminalLinks.openPort(session.value, port, {
      scheme: url.protocol.slice(0, -1),
      path: `${url.pathname}${url.search}${url.hash}`,
    });
    return;
  }
  openUrl(url.href).catch((error) => uiStore.setError(error));
}

function activateLink(link, line) {
  linkHint.value = null;
  if (link.kind === "port") {
    terminalLinks.openPort(session.value, link.port);
    return;
  }
  // 完整路径样式的提示符（user@host:/var/log$）才写得出目录；RHEL 那种只有目录名的说明不了
  const prompt = commandMarks.promptAbove(line);
  terminalLinks.openPath(session.value, link, { promptDir: prompt?.pathStyle === "full" ? prompt.pathText : null });
}

function showLinkHint(link, event) {
  const rect = viewportRoot.value?.getBoundingClientRect();
  if (!rect) return;
  linkHint.value = {
    text: link.kind === "port"
      ? t("terminal.links.forwardHint", { key: LINK_MODIFIER, port: link.port })
      : t("terminal.links.openHint", { key: LINK_MODIFIER }),
    x: event.clientX - rect.left,
    y: event.clientY - rect.top,
  };
}

// 从文件面板拖进来的路径插到光标处：走 paste，shell 开了括号粘贴时原样插入、不会被当成按键执行
function insertText(text) {
  if (!terminal || !text || !canSendData.value) return;
  terminal.paste(text);
  focus();
}

async function openSearch() {
  if (!props.active || disposed) return;
  const selection = terminal?.getSelection()?.trim() ?? "";
  const seed = selection && !selection.includes("\n") && selection.length <= 200 ? selection : "";
  isSearchOpen.value = true;
  await nextTick();
  if (props.active && !disposed && isSearchOpen.value) searchPanel.value?.activate(seed);
}

function closeSearch(refocus = true) {
  if (!isSearchOpen.value) return;
  searchPanel.value?.deactivate();
  isSearchOpen.value = false;
  searchAddon.value?.clearDecorations();
  if (refocus) focus();
}

function toggleSearch() {
  if (isSearchOpen.value) closeSearch();
  else openSearch();
}

defineExpose({ sessionId: props.sessionId, focus, toggleSearch, insertText, isSearchOpen });
</script>

<template>
  <div ref="viewportRoot" v-show="active" class="terminal-viewport" :inert="!active" data-terminal-drop>
    <div ref="terminalMount" class="terminal-xterm"></div>
    <div v-if="linkHint" class="terminal-link-hint" :style="{ left: `${linkHint.x}px`, top: `${linkHint.y}px` }">
      {{ linkHint.text }}
    </div>
    <Transition name="terminal-search-pop">
      <TerminalSearchPanel
        v-if="isSearchOpen"
        ref="searchPanel"
        :search-addon="searchAddon"
        :decorations="searchDecorations"
        @click.stop
        @close="closeSearch()"
      />
    </Transition>
  </div>
</template>

<style scoped src="../../assets/css/terminal-viewport.css"></style>
