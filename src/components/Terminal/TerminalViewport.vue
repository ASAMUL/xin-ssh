<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { openUrl } from "@tauri-apps/plugin-opener";
import TerminalSearchPanel from "./TerminalSearchPanel.vue";
import { buildXtermTheme } from "../../config/terminalThemes";
import { loadCommandHistory, recordCommand, suggestCommand } from "../../services/commandHistory";
import { resolveRemoteHome } from "../../services/workspaceApi";
import { createCommandMarks } from "../../utils/commandMarks";
import { createCommandSuggest } from "../../utils/commandSuggest";
import { createTerminalCwd } from "../../utils/terminalCwd";
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
const session = computed(() => sessionStore.findSession(props.sessionId));
// 后台终端仍需向原会话回复光标/设备查询；不能因不可见而关闭整个输入通道。
const canSendData = computed(() => session.value?.status === "connected"
  && session.value.terminalRevision === props.revision);
const terminalMount = ref(null);
const searchAddon = shallowRef(null);
const searchPanel = ref(null);
const isSearchOpen = ref(false);
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
let layoutFrame = null;
let disposed = false;
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
  // 终端目录跟着这个终端实例走：重连会换一个新终端，旧的目录记录随之作废
  terminalCwd = createTerminalCwd(terminal, {
    loginUser: session.value?.username ?? null,
    resolveLoginHome: () => resolveRemoteHome(props.sessionId),
    onChange: (cwd, { navigate }) => sessionStore.updateTerminalCwd(props.sessionId, props.revision, cwd, { navigate }),
  });
  commandMarks = createCommandMarks(terminal, {
    onCommand: handleCommand,
    onPrompt: (prompt) => terminalCwd.handlePrompt(prompt),
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
  detachSink?.();
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
// GPU 上下文只在「别的终端要显示」时才让出去。切回连接首页时没有终端可见，
// 这时释放 WebGL 反而最贵：xterm 会先退回 DOM 渲染器把整屏重画一遍（强制重排几百毫秒），
// 回到工作区再重建 WebGL 又卡一次。留着不渲染也不耗电——终端不可见时 xterm 自己会暂停绘制。
watch(() => sessionStore.activeSessionId, (activeId) => {
  if (activeId && activeId !== props.sessionId) releaseWebgl();
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

function updateAppearance() {
  if (!terminal || !props.active || disposed) return;
  terminal.options.theme = buildXtermTheme(props.theme, props.material);
  commandMarks.setTheme(props.theme);
  commandSuggest.setTheme(props.theme);
  terminal.options.fontFamily = props.fontFamily;
  terminal.options.fontSize = props.fontSize;
  scheduleLayout();
  document.fonts?.ready.then(scheduleLayout);
}

// 同一时刻只有一个终端占用 GPU 上下文（当前可见的那个，或切到首页前最后可见的那个）。
function updateRenderer() {
  if (!props.active) {
    releaseWebgl();
    return;
  }
  if (webglAddon || webglUnavailable) return;
  let addon = null;
  try {
    addon = new WebglAddon();
    terminal.loadAddon(addon);
    webglAddon = addon;
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
  if (event.type === "keydown" && acceptSuggestion(event)) return false;
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

function openTerminalLink(event, uri) {
  event.preventDefault();
  const url = new URL(uri);
  if (!["http:", "https:"].includes(url.protocol)) return;
  openUrl(url.href).catch((error) => uiStore.setError(error));
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

defineExpose({ sessionId: props.sessionId, focus, toggleSearch, isSearchOpen });
</script>

<template>
  <div v-show="active" class="terminal-viewport" :inert="!active">
    <div ref="terminalMount" class="terminal-xterm"></div>
    <Transition name="terminal-search-pop">
      <TerminalSearchPanel
        v-if="isSearchOpen"
        ref="searchPanel"
        :search-addon="searchAddon"
        @click.stop
        @close="closeSearch()"
      />
    </Transition>
  </div>
</template>

<style scoped src="../../assets/css/terminal-viewport.css"></style>
