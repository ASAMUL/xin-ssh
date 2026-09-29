<script setup>
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";
import { Pane, Splitpanes } from "splitpanes";
import { useI18n } from "vue-i18n";
import FileExplorer from "../FileExplorer.vue";
import SnippetPanel from "./SnippetPanel.vue";
import TerminalViewport from "./TerminalViewport.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { useCredentialStore } from "../../stores/credentialStore";
import { localizeError } from "../../utils/appError";
import { usesSavedCredentials } from "../../utils/authentication";
import {
  TERMINAL_FONTS,
  TERMINAL_FONT_SIZES,
  TERMINAL_THEMES,
  TERMINAL_THEME_GROUPS,
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
const terminalViews = ref([]);
const activeTerminal = computed(() =>
  terminalViews.value.find((view) => view.sessionId === sessionStore.activeSessionId),
);
const credentialInput = ref(null);
const shouldSaveCredential = ref(false);
const pendingCredentialSave = ref(null);
const credentials = reactive({
  password: "",
  passphrase: "",
});
let persistBackgroundTimer = null;
let pendingBackgroundWrite = null;
const backgroundSelections = new Map();

const isSearchOpen = computed(() => activeTerminal.value?.isSearchOpen ?? false);

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
    // 液态玻璃的高光：浅色主题上要亮白，深色主题上收着，否则一圈白边像贴纸
    "--term-highlight": displayTheme.value.mode === "light" ? "rgb(255 255 255 / 0.92)" : "rgb(255 255 255 / 0.24)",
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
const isAgentAuth = computed(() => activeSession.value?.authMethod === "agent");
const hasCredentialInput = computed(() => usesSavedCredentials(activeSession.value?.authMethod));
const authenticationTitle = computed(() => t(hasCredentialInput.value
  ? "terminal.authTitle" : isAgentAuth.value ? "profile.agentAuth" : "profile.interactiveAuth"));
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
  // 切换只处理凭据和焦点，终端解析状态由独立组件持续持有。
  () => [activeSession.value?.id, activeSession.value?.terminalRevision],
  async ([sessionId], previous = []) => {
    const [previousSessionId] = previous;
    if (sessionId !== previousSessionId) {
      credentials.password = "";
      credentials.passphrase = "";
      shouldSaveCredential.value = Boolean(activeSession.value?.credentialRef);
      pendingCredentialSave.value = null;
      if (!sessionId) {
        isAppearanceOpen.value = false;
        isSnippetsOpen.value = false;
        hoveredThemeId.value = null;
      }
    }

    await nextTick();
    focusActiveInput();
  },
  { immediate: true },
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

// 背景图字节走 IPC 读一次转 Blob URL，之后由 backgroundApi 缓存；
// 换图 / 换连接 / 清除都只是换 file 字段，这里跟着解析即可。
watch(
  () => activeBackground.value?.file ?? null,
  async (file, _previous, onCleanup) => {
    let stale = false;
    onCleanup(() => { stale = true; });
    backgroundUrl.value = null;
    if (!file) {
      return;
    }

    try {
      const url = await resolveTerminalBackgroundUrl(file);
      if (!stale) backgroundUrl.value = url;
    } catch (error) {
      console.warn("[Xin SSH] terminal background load failed", error);
      if (!stale) backgroundUrl.value = null;
    }
  },
  { immediate: true },
);

// 打开面板时默认停在「更具体」的那一档：本连接已单独设过就编辑本连接
watch(isAppearanceOpen, (open) => {
  if (!open) return;

  backgroundScope.value = profileBackground.value && activeProfile.value ? "profile" : "global";
});

watch([backgroundScope, () => activeProfile.value?.id], () => flushBackgroundWrite());

onBeforeUnmount(() => {
  // 拖完滑块立刻切走时，防抖里那一次还没落盘——补写，别丢用户刚调的值
  flushBackgroundWrite();
});

function focusTerminal() {
  if (needsCredentials.value) return;

  activeTerminal.value?.focus();
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

function toggleTerminalSearch() {
  activeTerminal.value?.toggleSearch();
}

function startTerminal() {
  const session = activeSession.value;
  const password = credentials.password;
  const passphrase = credentials.passphrase;

  pendingCredentialSave.value = shouldSaveCredential.value && session && hasCredentialInput.value
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
  const target = captureBackgroundTarget();
  const targetKey = JSON.stringify(target);
  const selection = Symbol();
  backgroundSelections.set(targetKey, selection);
  const current = editingBackground.value;
  flushBackgroundWrite();
  try {
    const sourcePath = await pickBackgroundImage(t("terminal.background.pick"));
    if (!sourcePath || backgroundSelections.get(targetKey) !== selection) return;

    const file = await importTerminalBackground(sourcePath);
    // 导入期间可能已清除、调节或重新选图，迟到结果不能撤销用户的新操作。
    if (backgroundSelections.get(targetKey) !== selection) return;
    await writeBackground({
      file,
      opacity: current?.opacity ?? 0.35,
      blur: current?.blur ?? 0,
      fit: current?.fit ?? "cover",
    }, target);
  } catch (error) {
    uiStore.setError(error);
  } finally {
    if (backgroundSelections.get(targetKey) === selection) backgroundSelections.delete(targetKey);
  }
}

function patchBackground(partial) {
  const current = editingBackground.value;
  if (!current) return;

  const next = { ...current, ...partial };
  const target = captureBackgroundTarget();
  backgroundSelections.delete(JSON.stringify(target));
  if (pendingBackgroundWrite && (
    pendingBackgroundWrite.target.profileId !== target.profileId
    || pendingBackgroundWrite.target.workspaceDir !== target.workspaceDir
  )) flushBackgroundWrite();
  // 先就地改内存让终端立刻跟着变，落盘防抖 250ms——拖滑块每一格都写一次
  // settings.json / profiles.json 是没必要的磁盘与 IPC 开销
  applyBackgroundLocally(next, target);
  pendingBackgroundWrite = { background: next, target };
  clearTimeout(persistBackgroundTimer);
  persistBackgroundTimer = setTimeout(flushBackgroundWrite, 250);
}

function clearBackground() {
  const target = captureBackgroundTarget();
  backgroundSelections.delete(JSON.stringify(target));
  flushBackgroundWrite();
  writeBackground(null, target);
}

function captureBackgroundTarget() {
  return {
    scope: backgroundScope.value,
    profileId: backgroundScope.value === "profile" ? activeProfile.value?.id : null,
    workspaceDir: settingsStore.workspaceDir,
  };
}

function applyBackgroundLocally(background, target) {
  if (target.scope === "profile") {
    profileStore.previewTerminalBackground(target.profileId, background);
  } else {
    settingsStore.previewSettings({ terminalBackground: background });
  }
}

function flushBackgroundWrite() {
  clearTimeout(persistBackgroundTimer);
  persistBackgroundTimer = null;
  const pending = pendingBackgroundWrite;
  pendingBackgroundWrite = null;
  if (pending) return writeBackground(pending.background, pending.target);
}

// 捕获发起时的工作区与连接；选图和防抖跨过切标签后，仍写回原目标。
async function writeBackground(background, target) {
  try {
    if (target.scope === "profile") {
      await profileStore.setTerminalBackground(target.profileId, background, target.workspaceDir);
      return;
    }

    await settingsStore.setTerminalBackground(background, target.workspaceDir);
  } catch (error) {
    uiStore.setError(error);
  }
}

function applyAppearance(partial) {
  settingsStore.patchSettings(partial).catch((error) => uiStore.setError(error));
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
        id: profile.id,
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
      key="terminal"
      :size="settingsStore.showFileExplorer ? 100 - settingsStore.filePaneSize : 100"
      min-size="26"
    >
      <section class="terminal-panel-top liquid-rim" :style="themeStyleVars">
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
          <!-- 工具按钮收进一颗跟随终端主题的玻璃胶囊；终端面板自带 backdrop 模糊，
               这里只借材质（高光描边 + 焦散），不再挂折射指令 -->
          <div class="terminal-action-capsule liquid-glass">
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
        </div>
      </header>

      <div class="terminal-screen" :style="backgroundStyleVars" @click="focusTerminal">
        <!-- 自定义背景图：图片层 + 压暗层，都垫在 xterm 之下 -->
        <template v-if="backgroundStyleVars">
          <div class="terminal-screen-image" aria-hidden="true"></div>
          <div class="terminal-screen-scrim" aria-hidden="true"></div>
        </template>

        <TerminalViewport
          v-for="session in sessionStore.sessions"
          :key="`${session.id}:${session.terminalRevision}`"
          ref="terminalViews"
          :session-id="session.id"
          :revision="session.terminalRevision"
          :active="session.id === activeSession?.id"
          :theme="displayTheme"
          :font-family="terminalFontStack"
          :font-size="settingsStore.terminalFontSize"
        />

        <!-- 扫描线 / 暗角这类「显像管」纹理必须压在字之上才有味道，故单独一层 -->
        <div
          v-if="displayTheme.ui.screenOverlay"
          class="terminal-screen-overlay"
          aria-hidden="true"
        ></div>

        <form v-if="needsCredentials" class="terminal-auth-panel" @click.stop @submit.prevent="startTerminal">
          <div class="auth-panel-copy">
            <strong>{{ authenticationTitle }}</strong>
            <small v-if="hasCredentialInput">{{ t("terminal.authDescription") }}</small>
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

          <UCheckbox v-if="hasCredentialInput" v-model="shouldSaveCredential" class="terminal-save-credential" :label="saveCredentialLabel"
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
      key="files"
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
