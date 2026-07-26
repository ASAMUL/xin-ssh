<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { Pane, Splitpanes } from "splitpanes";
import { useI18n } from "vue-i18n";
import FileExplorer from "../FileExplorer.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { useCredentialStore } from "../../stores/credentialStore";
import {
  TERMINAL_FONTS,
  TERMINAL_FONT_SIZES,
  TERMINAL_THEMES,
  TERMINAL_THEME_GROUPS,
  buildXtermTheme,
  getTerminalTheme,
  resolveTerminalFontStack,
} from "../../config/terminalThemes";

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

const activeSession = computed(() => sessionStore.activeSession);
const isAppearanceOpen = ref(false);
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
    "--term-glow": ui.glow,
    "--term-accent": displayTheme.value.accent,
    "--term-text": ui.text,
    "--term-text-muted": ui.textMuted,
    "--term-soft": ui.soft,
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

  terminal.options.theme = buildXtermTheme(theme);
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
    allowProposedApi: false,
    allowTransparency: true,
    cursorBlink: true,
    cursorStyle: "bar",
    fontFamily: terminalFontStack.value,
    fontSize: settingsStore.terminalFontSize,
    lineHeight: 1.18,
    scrollback: 8000,
    theme: buildXtermTheme(displayTheme.value),
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
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
});

function focusTerminal() {
  if (needsCredentials.value) return;

  terminal?.focus();
}

function focusCredentialInput() {
  credentialInput.value?.focus();
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

// Ctrl+C 有选区时复制而不发送中断;Ctrl+V 交给原生 paste 事件送入 xterm
function setupClipboardShortcuts() {
  terminal.attachCustomKeyEventHandler((event) => {
    if (event.type !== "keydown" || !event.ctrlKey || event.altKey || event.metaKey) {
      return true;
    }

    const key = event.key.toLowerCase();

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

// 会话切换 / 清屏 / 重连时全量重建：回放有界缓冲快照，然后注册直写通道，
// 后续输出事件由 sessionStore 直接写入 xterm，不经过响应式状态
function resyncTerminal() {
  if (!terminal) return;

  sessionStore.detachTerminalSink();
  terminal.reset();

  const session = activeSession.value;
  if (!session) return;

  const snapshot = sessionStore.getTerminalSnapshot(session.id);
  if (snapshot) {
    terminal.write(snapshot);
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

function applyAppearance(partial) {
  if (settingsStore.settings) {
    // optimistic: re-skin immediately, persistence follows
    Object.assign(settingsStore.settings, partial);
    settingsStore.patchSettings(partial).catch((error) => {
      uiStore.setError(String(error));
    });
    return;
  }

  settingsStore.patchSettings(partial);
}

function toggleAppearance() {
  isAppearanceOpen.value = !isAppearanceOpen.value;
  hoveredThemeId.value = null;
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
    uiStore.setError(`${t("terminal.saveCredentialFailed")}: ${String(error)}`);
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

      <div class="terminal-screen" @click="focusTerminal">
        <div ref="terminalMount" class="terminal-xterm"></div>

        <form v-if="needsCredentials" class="terminal-auth-panel" @click.stop @submit.prevent="startTerminal">
          <div class="auth-panel-copy">
            <strong>{{ t("terminal.authTitle") }}</strong>
            <small>{{ t("terminal.authDescription") }}</small>
          </div>

          <label v-if="isPasswordAuth">
            <span>{{ t("profile.password") }}</span>
            <input ref="credentialInput" v-model="credentials.password" type="password"
              autocomplete="current-password" />
          </label>

          <label v-if="isPrivateKeyAuth">
            <span>{{ t("profile.privateKeyPath") }}</span>
            <input :value="activeSession.privateKeyPath || ''" type="text" readonly />
          </label>

          <label v-if="isPrivateKeyAuth">
            <span>{{ t("profile.passphrase") }}</span>
            <input ref="credentialInput" v-model="credentials.passphrase" type="password"
              autocomplete="current-password" />
          </label>

          <UCheckbox v-model="shouldSaveCredential" class="terminal-save-credential" :label="saveCredentialLabel"
            :description="t('terminal.saveCredentialDescription')" />

          <p v-if="activeSession?.error" class="terminal-auth-error">{{ activeSession.error }}</p>

          <button type="submit" class="primary-action" :disabled="activeSession?.status === 'connecting'">
            <UIcon :name="activeSession?.status === 'connecting' ? 'i-lucide-loader-circle' : 'i-lucide-plug-zap'"
              :class="{ 'spin-icon': activeSession?.status === 'connecting' }" />
            <span>{{ t("terminal.connectReal") }}</span>
          </button>
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
          <button type="button" class="primary-action" @click="trustCurrentHostKey">
            <UIcon name="i-lucide-shield-check" />
            <span>{{ t("terminal.trustHostKey") }}</span>
          </button>
        </div>

        <div v-else-if="activeSession?.status === 'error'" class="terminal-connecting error">
          <UIcon name="i-lucide-circle-alert" />
          <span>{{ activeSession.error }}</span>
        </div>
      </div>

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
                  <span class="theme-swatch" :style="{ background: theme.preview }">
                    <span class="swatch-line">
                      <i :style="{ color: theme.xterm.green }">➜</i>
                      <em :style="{ color: theme.xterm.foreground }">~ ssh</em>
                      <b :style="{ background: theme.xterm.cursor }"></b>
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
