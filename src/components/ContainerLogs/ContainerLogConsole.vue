<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import TerminalSearchPanel from "../Terminal/TerminalSearchPanel.vue";
import { buildSearchDecorations, buildXtermTheme, resolveTerminalTextOptions } from "../../config/terminalThemes";

// 只负责把日志画出来：用 xterm 而不是编辑器，是因为容器日志常带 ANSI 颜色，编辑器里只会是一串 ^[[32m。
// 流从哪来、什么时候停由日志窗口管
const props = defineProps({
  // 已按明暗解析好的那一版调色板（resolveThemeVariant 的结果）
  theme: { type: Object, required: true },
  fontFamily: { type: String, required: true },
  fontSize: { type: Number, required: true },
});
// 画布建好了才能开始读：之前到的输出没处写
const emit = defineEmits(["ready"]);

const { t } = useI18n();
// 比回看上限（10000 行）多留一倍：一次回看全装得下，之后还能跟一阵新日志
const SCROLLBACK_LINES = 20000;
const mount = ref(null);
const searchPanel = ref(null);
const searchAddon = shallowRef(null);
const isSearchOpen = ref(false);
// 往上翻着看旧日志时 xterm 不会把视口拽回底部，这时给一个「回到最新」
const isScrolledUp = ref(false);
const searchDecorations = computed(() => buildSearchDecorations(props.theme));
let terminal = null;
let fitAddon = null;
let resizeObserver = null;
let layoutFrame = null;
const listeners = [];

onMounted(() => {
  terminal = new Terminal({
    allowProposedApi: true,
    // 开着 PTY 时服务器不再把 \n 换成 \r\n（见 remote_exec 的 PTY 设置），换行要由这边补上回车
    convertEol: true,
    disableStdin: true,
    fontFamily: props.fontFamily,
    fontSize: props.fontSize,
    ...resolveTerminalTextOptions(props.theme),
    lineHeight: 1.18,
    scrollback: SCROLLBACK_LINES,
    theme: logTheme(),
  });
  fitAddon = new FitAddon();
  searchAddon.value = new SearchAddon();
  terminal.loadAddon(fitAddon);
  terminal.loadAddon(searchAddon.value);
  terminal.loadAddon(new Unicode11Addon());
  terminal.unicode.activeVersion = "11";
  terminal.attachCustomKeyEventHandler(handleKey);
  terminal.open(mount.value);
  listeners.push(terminal.onScroll(syncScrollState), terminal.onWriteParsed(syncScrollState));
  resizeObserver = new ResizeObserver(scheduleLayout);
  resizeObserver.observe(mount.value);
  scheduleLayout();
  document.fonts?.ready.then(scheduleLayout);
  emit("ready");
});

onBeforeUnmount(() => {
  cancelAnimationFrame(layoutFrame);
  resizeObserver?.disconnect();
  listeners.forEach((listener) => listener.dispose());
  terminal?.dispose();
  terminal = null;
});

watch(() => [props.theme, props.fontFamily, props.fontSize], () => {
  if (!terminal) return;
  terminal.options.theme = logTheme();
  terminal.options.fontFamily = props.fontFamily;
  terminal.options.fontSize = props.fontSize;
  Object.assign(terminal.options, resolveTerminalTextOptions(props.theme));
  scheduleLayout();
});

// 总是实底，和编辑器一样：底下透出壁纸会影响读字。不接收输入，光标画成透明
function logTheme() {
  return { ...buildXtermTheme(props.theme, "solid"), cursor: "rgba(0, 0, 0, 0)" };
}

function scheduleLayout() {
  if (layoutFrame !== null) return;
  layoutFrame = requestAnimationFrame(() => {
    layoutFrame = null;
    if (terminal && mount.value?.clientWidth && mount.value.clientHeight) fitAddon.fit();
  });
}

function syncScrollState() {
  if (!terminal) return;
  const buffer = terminal.buffer.active;
  isScrolledUp.value = buffer.viewportY < buffer.baseY;
}

// 不接收输入，只留查找和复制两个快捷键。Esc 不让 xterm 吃掉，留给窗口去关
function handleKey(event) {
  if (event.key === "Escape") return false;
  if (event.type !== "keydown" || !(event.ctrlKey || event.metaKey)) return true;
  const key = event.key.toLowerCase();
  if (key === "f") {
    event.preventDefault();
    openSearch();
    return false;
  }
  if (key === "c" && terminal.hasSelection()) {
    navigator.clipboard.writeText(terminal.getSelection()).catch(() => {});
    return false;
  }
  return true;
}

async function openSearch() {
  if (!terminal) return;
  const selection = terminal.getSelection().trim();
  const seed = selection && !selection.includes("\n") && selection.length <= 200 ? selection : "";
  isSearchOpen.value = true;
  await nextTick();
  if (isSearchOpen.value) searchPanel.value?.activate(seed);
}

function closeSearch() {
  if (!isSearchOpen.value) return;
  searchPanel.value?.deactivate();
  isSearchOpen.value = false;
  searchAddon.value?.clearDecorations();
  terminal?.focus();
}

function toggleSearch() {
  if (isSearchOpen.value) closeSearch();
  else openSearch();
}

function write(data) {
  terminal?.write(data);
}

function clear() {
  terminal?.reset();
  isScrolledUp.value = false;
}

function scrollToBottom() {
  terminal?.scrollToBottom();
  isScrolledUp.value = false;
}

// 导出用：被自动折行拆开的长行接回一行，行尾补的空格去掉
function readText() {
  if (!terminal) return "";
  const buffer = terminal.buffer.active;
  const lines = [];
  for (let index = 0; index < buffer.length; index += 1) {
    const line = buffer.getLine(index);
    if (!line) continue;
    const text = line.translateToString(true);
    if (line.isWrapped && lines.length) lines[lines.length - 1] += text;
    else lines.push(text);
  }
  while (lines.length && !lines[lines.length - 1]) lines.pop();
  return lines.length ? `${lines.join("\n")}\n` : "";
}

defineExpose({ write, clear, readText, toggleSearch, focus: () => terminal?.focus() });
</script>

<template>
  <div class="container-log-console">
    <div ref="mount" class="container-log-xterm"></div>
    <Transition name="terminal-search-pop">
      <TerminalSearchPanel
        v-if="isSearchOpen"
        ref="searchPanel"
        :search-addon="searchAddon"
        :decorations="searchDecorations"
        @close="closeSearch"
      />
    </Transition>
    <Transition name="container-log-jump">
      <UButton
        v-if="isScrolledUp"
        class="container-log-jump"
        color="neutral"
        variant="solid"
        size="sm"
        icon="i-lucide-arrow-down"
        :label="t('containerLogs.jumpToLatest')"
        @click="scrollToBottom"
      />
    </Transition>
  </div>
</template>

<style scoped src="../../assets/css/container-log-console.css"></style>
