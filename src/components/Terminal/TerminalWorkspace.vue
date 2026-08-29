<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { Pane, Splitpanes } from "splitpanes";
import { useI18n } from "vue-i18n";
import FileExplorer from "../FileExplorer.vue";
import SnippetPanel from "./SnippetPanel.vue";
import TerminalSearchPanel from "./TerminalSearchPanel.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { useCredentialStore } from "../../stores/credentialStore";
import { localizeError } from "../../utils/appError";
import {
  TERMINAL_FONTS,
  TERMINAL_FONT_SIZES,
  TERMINAL_THEMES,
  TERMINAL_THEME_GROUPS,
  buildXtermTheme,
  getTerminalTheme,
  getThemePreviewBackground,
  getThemeTypography,
  resolveTerminalFontStack,
} from "../../config/terminalThemes";
import {
  TERMINAL_BACKGROUND_FITS,
  importTerminalBackground,
  normalizeTerminalBackground,
  pickBackgroundImage,
  resolveTerminalBackgroundUrl,
} from "../../services/backgroundApi";

const { t } = useI18n();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();
const credentialStore = useCredentialStore();
const terminalMount = ref(null);
const credentialInput = ref(null);
const shouldSaveCredential = ref(false);
const pendingCredentialSave = ref(null);
const credentials = reactive({
  password: "",
  passphrase: "",
});
let terminal = null;
let fitAddon = null;
let resizeObserver = null;
let disposeDataListener = null;
let disposeResizeListener = null;
let persistBackgroundTimer = null;

// 终端内查找：addon 实例给面板用，面板经 Ctrl+Shift+F / 工具栏按钮开关
const searchAddon = shallowRef(null);
const searchPanel = ref(null);
const isSearchOpen = ref(false);

const activeSession = computed(() => sessionStore.activeSession);
const isAppearanceOpen = ref(false);
const isSnippetsOpen = ref(false);
const hoveredThemeId = ref(null);
const activeTheme = computed(() => getTerminalTheme(settingsStore.terminalTheme));
// hovering a card previews the theme live; leaving the panel reverts it
const displayTheme = computed(() =>
  hoveredThemeId.value ? getTerminalTheme(hoveredThemeId.value) : activeTheme.value,
);
const terminalFontStack = computed(() =>
  resolveTerminalFontStack(settingsStore.terminalFontFamily),
);
const themeGroups = computed(() =>
  TERMINAL_THEME_GROUPS.map((group) => ({
    id: group,
    themes: TERMINAL_THEMES.filter((theme) => theme.group === group),
  })),
);
const themeStyleVars = computed(() => {
  const ui = displayTheme.value.ui;
  return {
    "--term-panel-bg": ui.panelBg,
    "--term-panel-border": ui.panelBorder,
    "--term-toolbar-bg": ui.toolbarBg,
    "--term-divider": ui.divider,
    "--term-screen-bg": ui.screenBg,
    "--term-screen-overlay": ui.screenOverlay ?? "none",
    "--term-text-glow": getThemeTypography(displayTheme.value).glow ?? "none",
    "--term-glow": ui.glow,
    "--term-accent": displayTheme.value.accent,
    "--term-text": ui.text,
    "--term-text-muted": ui.textMuted,
    "--term-soft": ui.soft,
  };
});

// —— 终端背景图 ——
// 优先级：当前连接自己的那张 > 全局设置那张。生产机 / 测试机各配一张图，
// 切 tab 时背景直接变，比只看 tab 标题更难认错机器。
const activeProfile = computed(() =>
  profileStore.profiles.find((profile) => profile.id === activeSession.value?.profileId),
);
const profileBackground = computed(() =>
  normalizeTerminalBackground(activeProfile.value?.terminalBackground),
);
const globalBackground = computed(() => settingsStore.terminalBackground);
const activeBackground = computed(() => profileBackground.value ?? globalBackground.value);
// 面板里正在编辑的是哪一档；没有连接上下文时只能编辑全局
const backgroundScope = ref("global");
const editingBackground = computed(() =>
  backgroundScope.value === "profile" ? profileBackground.value : globalBackground.value,
);
const backgroundUrl = ref(null);
const backgroundStyleVars = computed(() => {
  const background = activeBackground.value;
  if (!background || !backgroundUrl.value) return null;

  return {
    "--term-image": `url("${backgroundUrl.value}")`,
    "--term-image-opacity": background.opacity,
    "--term-image-blur": `${background.blur}px`,
    "--term-image-size": background.fit === "tile" ? "auto" : background.fit,
    "--term-image-repeat": background.fit === "tile" ? "repeat" : "no-repeat",
    // 压暗层随图片可见度一起走：图越显眼，越需要压一层底才读得清字。
    // 浅色主题反过来铺白，否则深色照片会把深色文字吃掉。
    "--term-image-scrim":
      displayTheme.value.mode === "light"
        ? `rgba(255, 255, 255, ${(background.opacity * 0.45).toFixed(3)})`
        : `rgba(0, 0, 0, ${(background.opacity * 0.42).toFixed(3)})`,
  };
});
const isPasswordAuth = computed(() => activeSession.value?.authMethod === "password");
const isPrivateKeyAuth = computed(() => activeSession.value?.authMethod === "privateKey");
const needsCredentials = computed(() => activeSession.value?.status === "authRequired");
const terminalStatusText = computed(() => {
  const session = activeSession.value;
  if (!session) return "";

  return t(`monitor.status.${session.status}`);
});
const saveCredentialLabel = computed(() =>
  t(isPasswordAuth.value ? "terminal.savePassword" : "terminal.savePassphrase"),
);

watch(
  // 会话切换或 terminalRevision 变化（连接重开 / 清屏）都需要全量重建终端内容
  () => [activeSession.value?.id, activeSession.value?.terminalRevision],
  async ([sessionId], previous = []) => {
    const [previousSessionId] = previous;
    if (sessionId !== previousSessionId) {
      credentials.password = "";
      credentials.passphrase = "";
      shouldSaveCredential.value = Boolean(activeSession.value?.credentialRef);
      pendingCredentialSave.value = null;
      closeTerminalSearch({ refocus: false });
    }

    await nextTick();
    resyncTerminal();
    fitTerminal();
    focusActiveInput();
  },
);

watch(needsCredentials, async () => {
  await nextTick();
  focusActiveInput();
});

watch(
  () => activeSession.value?.status,
  (status) => {
    if (status === "connected") {
      savePendingCredential();
      clearCredentialInputs();
      return;
    }

    if (status === "error" || status === "authRequired") {
      pendingCredentialSave.value = null;
    }
  },
);

watch(displayTheme, (theme) => {
  if (!terminal) return;

  const text = getThemeTypography(theme);
  terminal.options.theme = buildXtermTheme(theme);
  // 字重与光标形状都不改变 xterm 的字符测量结果，所以切主题不必 refit
  terminal.options.fontWeight = text.weight;
  terminal.options.cursorStyle = text.cursorStyle;
});

// 背景图字节走 IPC 读一次转 Blob URL，之后由 backgroundApi 缓存；
// 换图 / 换连接 / 清除都只是换 file 字段，这里跟着解析即可。
watch(
  () => activeBackground.value?.file ?? null,
  async (file) => {
    if (!file) {
      backgroundUrl.value = null;
      return;
    }

    try {
      backgroundUrl.value = await resolveTerminalBackgroundUrl(file);
    } catch (error) {
      console.warn("[Xin Terminal] terminal background load failed", error);
      backgroundUrl.value = null;
    }
  },
  { immediate: true },
);

// 打开面板时默认停在「更具体」的那一档：本连接已单独设过就编辑本连接
watch(isAppearanceOpen, (open) => {
  if (!open) return;

  backgroundScope.value = profileBackground.value && activeProfile.value ? "profile" : "global";
});

watch(terminalFontStack, (stack) => {
  if (!terminal) return;

  terminal.options.fontFamily = stack;
  refitAfterFontLoad();
});

watch(
  () => settingsStore.terminalFontSize,
  (size) => {
    if (!terminal) return;

    terminal.options.fontSize = size;
    refitAfterFontLoad();
  },
);

onMounted(async () => {
  terminal = new Terminal({
    // addon-search 的匹配高亮依赖 registerDecoration（experimental API，受此开关拦截）
    allowProposedApi: true,
    allowTransparency: true,
    cursorBlink: true,
    cursorStyle: getThemeTypography(displayTheme.value).cursorStyle,
    fontFamily: terminalFontStack.value,
    fontSize: settingsStore.terminalFontSize,
    fontWeight: getThemeTypography(displayTheme.value).weight,
    lineHeight: 1.18,
    scrollback: 8000,
    theme: buildXtermTheme(displayTheme.value),
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  searchAddon.value = new SearchAddon();
  terminal.loadAddon(searchAddon.value);
  terminal.open(terminalMount.value);
  setupClipboardShortcuts();

  disposeDataListener = terminal.onData((data) => {
    sessionStore.sendActiveTerminalInput(data).catch(() => { });
  });
  disposeResizeListener = terminal.onResize(({ cols, rows }) => {
    sessionStore.resizeActiveTerminal(cols, rows);
  });

  resizeObserver = new ResizeObserver(() => {
    fitTerminal();
  });
  resizeObserver.observe(terminalMount.value);

  await nextTick();
  resyncTerminal();
  fitTerminal();
  focusActiveInput();
  // webfont metrics may land after the terminal measured itself
  document.fonts?.ready?.then(() => fitTerminal());
});

onBeforeUnmount(() => {
  sessionStore.detachTerminalSink();
  resizeObserver?.disconnect();
  disposeDataListener?.dispose();
  disposeResizeListener?.dispose();
  terminal?.dispose();
  // 拖完滑块立刻切走时，防抖里那一次还没落盘——补写，别丢用户刚调的值
  if (persistBackgroundTimer) {
    clearTimeout(persistBackgroundTimer);
    writeBackground(editingBackground.value);
  }
});

function focusTerminal() {
  if (needsCredentials.value) return;

  terminal?.focus();
}

function focusCredentialInput() {
  credentialInput.value?.inputRef?.focus();
}

function focusActiveInput() {
  if (needsCredentials.value) {
    focusCredentialInput();
    return;
  }

  focusTerminal();
}

function fitTerminal() {
  if (!terminal || !fitAddon || !terminalMount.value) return;

  fitAddon.fit();
}

// Ctrl+C 有选区时复制而不发送中断;Ctrl+V 交给原生 paste 事件送入 xterm;
// Ctrl+Shift+F 打开终端内查找（Ctrl+F 留给 shell 的光标前移）
function setupClipboardShortcuts() {
  terminal.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown" || !event.ctrlKey || event.altKey || event.metaKey) {
      return true;
    }

    const key = event.key.toLowerCase();

    if (key === "f" && event.shiftKey) {
      openTerminalSearch();
      return false;
    }

    if (key === "c" && terminal.hasSelection()) {
      copyTerminalSelection();
      return false;
    }

    if (key === "v") {
      return false;
    }

    return true;
  });
}

function copyTerminalSelection() {
  const selection = terminal?.getSelection();
  if (!selection) return;

  navigator.clipboard?.writeText(selection).catch(() => { });
  // 清掉选区作为“已复制”的视觉反馈，下一次 Ctrl+C 恢复为中断信号
  terminal.clearSelection();
}

async function openTerminalSearch() {
  // 单行短选区作为搜索词预填，贴近编辑器查找的手感
  const selection = terminal?.getSelection()?.trim() ?? "";
  const seed = selection && !selection.includes("\n") && selection.length <= 200 ? selection : "";

  isSearchOpen.value = true;
  await nextTick();
  searchPanel.value?.activate(seed);
}

function closeTerminalSearch({ refocus = true } = {}) {
  if (!isSearchOpen.value) {
    if (!refocus) searchAddon.value?.clearDecorations();
    return;
  }

  isSearchOpen.value = false;
  searchAddon.value?.clearDecorations();
  if (refocus) focusTerminal();
}

function toggleTerminalSearch() {
  if (isSearchOpen.value) {
    closeTerminalSearch();
    return;
  }

  openTerminalSearch();
}

// 会话切换 / 清屏 / 重连时全量重建：回放有界缓冲快照，然后注册直写通道，
// 后续输出事件由 sessionStore 直接写入 xterm，不经过响应式状态
function resyncTerminal() {
  if (!terminal) return;

  sessionStore.detachTerminalSink();
  terminal.reset();
  // 装饰随 buffer 重置失效，但引擎的缓存词与结果集仍在——一并清掉，
  // 否则切会话后面板会显示旧会话的计数，且同词重搜不会重建高亮
  searchAddon.value?.clearDecorations();

  const session = activeSession.value;
  if (!session) return;

  const snapshot = sessionStore.getTerminalSnapshot(session.id);
  if (snapshot) {
    // 回放分片解析，写完回调里再重算搜索，避免只搜到部分回放内容
    terminal.write(snapshot, () => {
      if (isSearchOpen.value) searchPanel.value?.refresh();
    });
  }

  sessionStore.attachTerminalSink(session.id, (data) => {
    terminal?.write(data);
  });
}

function startTerminal() {
  const session = activeSession.value;
  const password = credentials.password;
  const passphrase = credentials.passphrase;

  pendingCredentialSave.value = shouldSaveCredential.value && session
    ? {
      sessionId: session.id,
      password: isPasswordAuth.value ? password : "",
      passphrase: isPrivateKeyAuth.value ? passphrase : "",
    }
    : null;

  sessionStore.startActiveTerminal({
    password,
    passphrase,
  });
  focusTerminal();
}

function reconnectTerminal() {
  credentials.password = "";
  credentials.passphrase = "";
  shouldSaveCredential.value = Boolean(activeSession.value?.credentialRef);
  pendingCredentialSave.value = null;
  sessionStore.reconnectActiveTerminal();
}

async function trustCurrentHostKey() {
  await sessionStore.trustHostKeyAndReconnect();
}

function selectTheme(themeId) {
  applyAppearance({ terminalTheme: themeId });
}

function selectFont(fontName) {
  applyAppearance({ terminalFontFamily: fontName });
}

function selectFontSize(fontSize) {
  applyAppearance({ terminalFontSize: fontSize });
}

// —— 背景图操作 ——
// 全局档写 settings.json，连接档写 profiles.json；两条链路都已有乐观更新，
// 这里只负责组装完整的 background 对象（后端字段有 serde default，但前端
// 传半个对象会让 opacity/fit 悄悄回落到默认值，不如每次写全）。
async function chooseBackgroundImage() {
  try {
    const sourcePath = await pickBackgroundImage(t("terminal.background.pick"));
    if (!sourcePath) return;

    const file = await importTerminalBackground(sourcePath);
    const current = editingBackground.value;
    await writeBackground({
      file,
      opacity: current?.opacity ?? 0.35,
      blur: current?.blur ?? 0,
      fit: current?.fit ?? "cover",
    });
  } catch (error) {
    uiStore.setError(error);
  }
}

function patchBackground(partial) {
  const current = editingBackground.value;
  if (!current) return;

  const next = { ...current, ...partial };
  // 先就地改内存让终端立刻跟着变，落盘防抖 250ms——拖滑块每一格都写一次
  // settings.json / profiles.json 是没必要的磁盘与 IPC 开销
  applyBackgroundLocally(next);
  clearTimeout(persistBackgroundTimer);
  persistBackgroundTimer = setTimeout(() => writeBackground(next), 250);
}

function clearBackground() {
  clearTimeout(persistBackgroundTimer);
  applyBackgroundLocally(null);
  writeBackground(null);
}

function applyBackgroundLocally(background) {
  if (backgroundScope.value === "profile" && activeProfile.value) {
    activeProfile.value.terminalBackground = background;
    return;
  }

  if (settingsStore.settings) {
    settingsStore.settings.terminalBackground = background;
  }
}

async function writeBackground(background) {
  try {
    if (backgroundScope.value === "profile" && activeProfile.value) {
      await profileStore.setTerminalBackground(activeProfile.value.id, background);
      return;
    }

    await settingsStore.setTerminalBackground(background);
  } catch (error) {
    uiStore.setError(error);
  }
}

function applyAppearance(partial) {
  if (settingsStore.settings) {
    // optimistic: re-skin immediately, persistence follows
    Object.assign(settingsStore.settings, partial);
    settingsStore.patchSettings(partial).catch((error) => {
      uiStore.setError(error);
    });
    return;
  }

  settingsStore.patchSettings(partial);
}

// 外观与片段共用右缘滑入位，互斥打开
function toggleAppearance() {
  isAppearanceOpen.value = !isAppearanceOpen.value;
  if (isAppearanceOpen.value) isSnippetsOpen.value = false;
  hoveredThemeId.value = null;
}

function toggleSnippets() {
  isSnippetsOpen.value = !isSnippetsOpen.value;
  if (isSnippetsOpen.value) isAppearanceOpen.value = false;
}

// 文件面板拖拽结束后持久化高度占比（xterm 的 ResizeObserver 会自动 refit）
function persistFilePaneSize(event) {
  const panes = Array.isArray(event) ? event : event?.panes ?? [];
  if (!settingsStore.showFileExplorer || panes.length < 2) return;

  settingsStore.applySettingsPatch({ filePaneSize: panes[panes.length - 1].size });
}

function refitAfterFontLoad() {
  fitTerminal();
  document.fonts?.ready?.then(() => fitTerminal());
}

function themePreviewColors(theme) {
  const { red, yellow, green, cyan, blue, magenta } = theme.xterm;
  return [red, yellow, green, cyan, blue, magenta];
}

function clearCredentialInputs() {
  credentials.password = "";
  credentials.passphrase = "";
}

async function savePendingCredential() {
  const pending = pendingCredentialSave.value;
  const session = activeSession.value;
  if (!pending || !session || pending.sessionId !== session.id) return;

  pendingCredentialSave.value = null;

  const secret = {
    password: pending.password,
    passphrase: pending.passphrase,
  };

  if (!secret.password && !secret.passphrase) return;

  const credentialRef = session.credentialRef || crypto.randomUUID();
  try {
    await credentialStore.saveCredential(credentialRef, secret);
    session.credentialRef = credentialRef;

    const profile = profileStore.profiles.find((item) => item.id === session.profileId);
    if (profile && profile.credentialRef !== credentialRef) {
      await profileStore.saveProfile({
        ...profile,
        credentialRef,
      });
    }
  } catch (error) {
    uiStore.setError(`${t("terminal.saveCredentialFailed")}: ${localizeError(error)}`);
  }
}
</script>

<template>
  <Splitpanes horizontal class="xin-split terminal-split" @resized="persistFilePaneSize">
    <Pane
      :size="settingsStore.showFileExplorer ? 100 - settingsStore.filePaneSize : 100"
      min-size="26"
    >
      <section class="terminal-panel-top" :style="themeStyleVars">
      <header class="terminal-panel-toolbar">
        <div class="terminal-title-block">
          <span class="terminal-live-dot" :class="activeSession?.status"></span>
          <div class="terminal-title-copy">
            <span class="terminal-name-row">
              <strong>{{ activeSession?.name }}</strong>
              <em>SSH</em>
            </span>
            <small>{{ activeSession?.username }}@{{ activeSession?.host }}:{{ activeSession?.port }}</small>
          </div>
        </div>

        <div class="terminal-window-actions">
          <span class="terminal-state-label">{{ terminalStatusText }}</span>
          <button
            type="button"
            :class="{ active: isSearchOpen }"
            :aria-label="t('terminal.search.toggle')"
            :title="`${t('terminal.search.toggle')} (Ctrl+Shift+F)`"
            @click="toggleTerminalSearch"
          >
            <UIcon name="i-lucide-search" />
          </button>
          <button
            type="button"
            :class="{ active: !settingsStore.showMonitorSidebar }"
            :aria-label="t(settingsStore.showMonitorSidebar ? 'terminal.hideSidebar' : 'terminal.showSidebar')"
            :title="t(settingsStore.showMonitorSidebar ? 'terminal.hideSidebar' : 'terminal.showSidebar')"
            @click="settingsStore.toggleMonitorSidebar()"
          >
            <UIcon :name="settingsStore.showMonitorSidebar ? 'i-lucide-panel-left-close' : 'i-lucide-panel-left-open'" />
          </button>
          <button
            type="button"
            :class="{ active: !settingsStore.showFileExplorer }"
            :aria-label="t(settingsStore.showFileExplorer ? 'terminal.hideFilePanel' : 'terminal.showFilePanel')"
            :title="t(settingsStore.showFileExplorer ? 'terminal.hideFilePanel' : 'terminal.showFilePanel')"
            @click="settingsStore.toggleFileExplorer()"
          >
            <UIcon :name="settingsStore.showFileExplorer ? 'i-lucide-panel-bottom-close' : 'i-lucide-panel-bottom-open'" />
          </button>
          <button
            type="button"
            :class="{ active: isSnippetsOpen }"
            :aria-label="t('snippets.title')"
            :title="t('snippets.title')"
            @click="toggleSnippets"
          >
            <UIcon name="i-lucide-scroll-text" />
          </button>
          <button
            type="button"
            :class="{ active: isAppearanceOpen }"
            :aria-label="t('terminal.appearance')"
            :title="t('terminal.appearance')"
            @click="toggleAppearance"
          >
            <UIcon name="i-lucide-palette" />
          </button>
          <button type="button" :aria-label="t('terminal.clear')" @click="sessionStore.clearActiveTerminal">
            <UIcon name="i-lucide-eraser" />
          </button>
          <button type="button" :aria-label="t('terminal.reconnect')" @click="reconnectTerminal">
            <UIcon name="i-lucide-rotate-cw" />
          </button>
        </div>
      </header>

      <div class="terminal-screen" :style="backgroundStyleVars" @click="focusTerminal">
        <!-- 自定义背景图：图片层 + 压暗层，都垫在 xterm 之下 -->
        <template v-if="backgroundStyleVars">
          <div class="terminal-screen-image" aria-hidden="true"></div>
          <div class="terminal-screen-scrim" aria-hidden="true"></div>
        </template>

        <div ref="terminalMount" class="terminal-xterm"></div>

        <!-- 扫描线 / 暗角这类「显像管」纹理必须压在字之上才有味道，故单独一层 -->
        <div
          v-if="displayTheme.ui.screenOverlay"
          class="terminal-screen-overlay"
          aria-hidden="true"
        ></div>

        <Transition name="terminal-search-pop">
          <TerminalSearchPanel
            v-if="isSearchOpen"
            ref="searchPanel"
            :search-addon="searchAddon"
            @click.stop
            @close="closeTerminalSearch"
          />
        </Transition>

        <form v-if="needsCredentials" class="terminal-auth-panel" @click.stop @submit.prevent="startTerminal">
          <div class="auth-panel-copy">
            <strong>{{ t("terminal.authTitle") }}</strong>
            <small>{{ t("terminal.authDescription") }}</small>
          </div>

          <UFormField v-if="isPasswordAuth" class="terminal-auth-field" :label="t('profile.password')">
            <UInput
              ref="credentialInput"
              v-model="credentials.password"
              type="password"
              size="lg"
              class="w-full"
              autocomplete="current-password"
            />
          </UFormField>

          <UFormField v-if="isPrivateKeyAuth" class="terminal-auth-field" :label="t('profile.privateKeyPath')">
            <UInput :model-value="activeSession.privateKeyPath || ''" size="lg" class="w-full" readonly />
          </UFormField>

          <UFormField v-if="isPrivateKeyAuth" class="terminal-auth-field" :label="t('profile.passphrase')">
            <UInput
              ref="credentialInput"
              v-model="credentials.passphrase"
              type="password"
              size="lg"
              class="w-full"
              autocomplete="current-password"
            />
          </UFormField>

          <UCheckbox v-model="shouldSaveCredential" class="terminal-save-credential" :label="saveCredentialLabel"
            :description="t('terminal.saveCredentialDescription')" />

          <p v-if="activeSession?.error" class="terminal-auth-error">{{ activeSession.error }}</p>

          <UButton
            type="submit"
            color="primary"
            size="lg"
            class="cta-button"
            icon="i-lucide-plug-zap"
            :loading="activeSession?.status === 'connecting'"
            :label="t('terminal.connectReal')"
          />
        </form>

        <div v-else-if="activeSession?.status === 'connecting'" class="terminal-connecting">
          <UIcon name="i-lucide-loader-circle" class="spin-icon" />
          <span>{{ t("terminal.connecting") }}</span>
        </div>

        <div
          v-else-if="activeSession?.status === 'error' && activeSession?.hostKeyChange"
          class="terminal-hostkey-panel"
        >
          <div class="hostkey-head">
            <UIcon name="i-lucide-shield-alert" />
            <strong>{{ t("terminal.hostKeyChangedTitle") }}</strong>
          </div>
          <p>{{ t("terminal.hostKeyChangedDescription") }}</p>
          <div class="hostkey-fingerprints">
            <div>
              <span>{{ t("terminal.hostKeyExpected") }}</span>
              <code>{{ activeSession.hostKeyChange.expectedFingerprint }}</code>
            </div>
            <div>
              <span>{{ t("terminal.hostKeyActual") }}</span>
              <code>{{ activeSession.hostKeyChange.actualFingerprint }}</code>
            </div>
          </div>
          <UButton
            color="primary"
            size="lg"
            class="cta-button"
            icon="i-lucide-shield-check"
            :label="t('terminal.trustHostKey')"
            @click="trustCurrentHostKey"
          />
        </div>

        <div v-else-if="activeSession?.status === 'error'" class="terminal-connecting error">
          <UIcon name="i-lucide-circle-alert" />
          <span>{{ activeSession.error }}</span>
        </div>
      </div>

      <Transition name="snippet-slide">
        <SnippetPanel v-if="isSnippetsOpen" @close="isSnippetsOpen = false" />
      </Transition>

      <Transition name="appearance-slide">
        <aside
          v-if="isAppearanceOpen"
          class="terminal-appearance"
          @mouseleave="hoveredThemeId = null"
        >
          <header class="appearance-head">
            <strong>{{ t("terminal.appearance") }}</strong>
            <button type="button" :aria-label="t('settings.close')" @click="toggleAppearance">
              <UIcon name="i-lucide-x" />
            </button>
          </header>

          <div class="appearance-body">
            <section
              v-for="group in themeGroups"
              :key="group.id"
              class="appearance-section"
            >
              <h4>{{ t(`terminal.themeGroup.${group.id}`) }}</h4>

              <div class="theme-grid">
                <button
                  v-for="theme in group.themes"
                  :key="theme.id"
                  type="button"
                  class="theme-card"
                  :class="{ active: theme.id === activeTheme.id }"
                  :style="{ '--card-accent': theme.accent }"
                  @mouseenter="hoveredThemeId = theme.id"
                  @focus="hoveredThemeId = theme.id"
                  @click="selectTheme(theme.id)"
                >
                  <span
                    class="theme-swatch"
                    :style="{ background: getThemePreviewBackground(theme) }"
                  >
                    <span
                      v-if="theme.ui.screenOverlay"
                      class="swatch-overlay"
                      :style="{ background: theme.ui.screenOverlay }"
                    ></span>
                    <span class="swatch-line">
                      <i :style="{ color: theme.xterm.green }">➜</i>
                      <em :style="{ color: theme.xterm.blue }">~</em>
                      <em :style="{ color: theme.xterm.foreground }">ls</em>
                      <b :style="{ background: theme.xterm.cursor }"></b>
                    </span>
                    <!-- ls 把语义压在固定槽位上：蓝=目录 绿=可执行 青=符号链接。
                         预览卡照着摆，配色好不好用一眼就知道，比 6 个抽象色点实在 -->
                    <span class="swatch-line ls">
                      <em :style="{ color: theme.xterm.blue }">docs/</em>
                      <em :style="{ color: theme.xterm.green }">run.sh</em>
                      <em :style="{ color: theme.xterm.cyan }">link@</em>
                    </span>
                    <span class="swatch-dots">
                      <i
                        v-for="(color, index) in themePreviewColors(theme)"
                        :key="index"
                        :style="{ background: color }"
                      ></i>
                    </span>
                  </span>
                  <span class="theme-card-name">
                    <span>{{ theme.name }}</span>
                    <UIcon v-if="theme.id === activeTheme.id" name="i-lucide-check" />
                  </span>
                </button>
              </div>
            </section>

            <section class="appearance-section">
              <h4>{{ t("terminal.font") }}</h4>

              <div class="font-list">
                <button
                  v-for="font in TERMINAL_FONTS"
                  :key="font.name"
                  type="button"
                  class="font-option"
                  :class="{ active: font.name === settingsStore.terminalFontFamily }"
                  @click="selectFont(font.name)"
                >
                  <span class="font-sample" :style="{ fontFamily: font.stack }">Aa =&gt; {}</span>
                  <span class="font-name">{{ font.name }}</span>
                  <UIcon v-if="font.name === settingsStore.terminalFontFamily" name="i-lucide-check" />
                </button>
              </div>
            </section>

            <section class="appearance-section">
              <h4>{{ t("terminal.fontSize") }}</h4>

              <div class="size-row">
                <button
                  v-for="size in TERMINAL_FONT_SIZES"
                  :key="size"
                  type="button"
                  :class="{ active: size === settingsStore.terminalFontSize }"
                  @click="selectFontSize(size)"
                >
                  {{ size }}
                </button>
              </div>
            </section>
            <section class="appearance-section">
              <h4>{{ t("terminal.background.title") }}</h4>

              <!-- 作用范围。没有连接上下文时只剩全局一档，整条不显示 -->
              <div v-if="activeProfile" class="size-row">
                <button
                  type="button"
                  :class="{ active: backgroundScope === 'global' }"
                  @click="backgroundScope = 'global'"
                >
                  {{ t("terminal.background.scopeGlobal") }}
                </button>
                <button
                  type="button"
                  :class="{ active: backgroundScope === 'profile' }"
                  @click="backgroundScope = 'profile'"
                >
                  {{ t("terminal.background.scopeProfile") }}
                </button>
              </div>

              <!-- 编辑全局但本连接已有覆盖时，改了看不到效果，必须说清楚 -->
              <p
                v-if="backgroundScope === 'global' && profileBackground"
                class="background-note"
              >
                {{ t("terminal.background.overriddenHint") }}
              </p>

              <div
                v-if="editingBackground && backgroundUrl"
                class="background-preview"
                :style="{
                  backgroundImage: `url('${backgroundUrl}')`,
                  backgroundSize: editingBackground.fit === 'tile' ? 'auto' : editingBackground.fit,
                  backgroundRepeat: editingBackground.fit === 'tile' ? 'repeat' : 'no-repeat',
                }"
              ></div>

              <div class="background-actions">
                <UButton
                  color="neutral"
                  variant="outline"
                  size="sm"
                  icon="i-lucide-image"
                  :label="
                    editingBackground
                      ? t('terminal.background.replace')
                      : t('terminal.background.choose')
                  "
                  @click="chooseBackgroundImage"
                />
                <UButton
                  v-if="editingBackground"
                  color="neutral"
                  variant="ghost"
                  size="sm"
                  icon="i-lucide-trash-2"
                  :aria-label="t('terminal.background.remove')"
                  :title="t('terminal.background.remove')"
                  @click="clearBackground"
                />
              </div>

              <template v-if="editingBackground">
                <div class="background-slider">
                  <span>
                    {{ t("terminal.background.opacity") }}
                    <em>{{ Math.round(editingBackground.opacity * 100) }}%</em>
                  </span>
                  <USlider
                    :model-value="Math.round(editingBackground.opacity * 100)"
                    :min="4"
                    :max="100"
                    size="sm"
                    @update:model-value="patchBackground({ opacity: $event / 100 })"
                  />
                </div>

                <div class="background-slider">
                  <span>
                    {{ t("terminal.background.blur") }}
                    <em>{{ editingBackground.blur }}px</em>
                  </span>
                  <USlider
                    :model-value="editingBackground.blur"
                    :min="0"
                    :max="30"
                    size="sm"
                    @update:model-value="patchBackground({ blur: $event })"
                  />
                </div>

                <div class="size-row">
                  <button
                    v-for="fit in TERMINAL_BACKGROUND_FITS"
                    :key="fit"
                    type="button"
                    :class="{ active: fit === editingBackground.fit }"
                    @click="patchBackground({ fit })"
                  >
                    {{ t(`terminal.background.fit.${fit}`) }}
                  </button>
                </div>
              </template>
            </section>
          </div>
        </aside>
      </Transition>
    </section>
    </Pane>

    <Pane
      v-if="settingsStore.showFileExplorer"
      :size="settingsStore.filePaneSize"
      min-size="16"
      max-size="72"
    >
      <FileExplorer :active-session="activeSession" />
    </Pane>
  </Splitpanes>
</template>

<style scoped src="../../assets/css/terminal-workspace.css"></style>
<style scoped src="../../assets/css/terminal-appearance.css"></style>
