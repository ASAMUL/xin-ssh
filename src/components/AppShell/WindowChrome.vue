<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import FileOperationPanel from "../FileOperationPanel.vue";
import { GLASS_CLEAR } from "../../directives/liquidGlass";
import WindowControls from "./WindowControls.vue";
import brandAvatar from "../../assets/brand/xin-avatar.png";
import { useAppStore } from "../../stores/appStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useTransferStore } from "../../stores/transferStore";
import { useUiStore } from "../../stores/uiStore";
import { useSessionTabShortcuts } from "../../composables/useSessionTabShortcuts";
import { useWorkspaceNavigationLock } from "../../composables/useWorkspaceNavigationLock";

const { t } = useI18n();
const appStore = useAppStore();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();
const navigationLocked = useWorkspaceNavigationLock();
// 顶栏从启动起就常驻：快捷键挂这里，连接页也能 Ctrl+数字 跳进会话，启动期间也能吞掉 Ctrl+W
useSessionTabShortcuts({ disabled: () => navigationLocked.value || uiStore.isDraggingSessionTab });
const appWindow = window.__TAURI_INTERNALS__ ? getCurrentWindow() : null;
const platform = navigator.userAgentData?.platform ?? navigator.platform ?? "";
const isMac = platform.toLowerCase().includes("mac");

const openSessionCount = computed(() => sessionStore.sessions.filter((session) => !session.isClosing).length);

// 顶栏只分两个去处：连接（首页）和工作区（上次停留的会话）；具体会话标签在工作区内部
const viewItems = computed(() => [
  {
    value: "hub",
    label: t("workspace.connectionHub"),
    icon: "i-lucide-monitor",
    disabled: navigationLocked.value,
  },
  {
    value: "workspace",
    label: t("workspace.title"),
    icon: "i-lucide-box",
    badge: openSessionCount.value ? { label: openSessionCount.value, color: "neutral", variant: "soft" } : undefined,
    disabled: navigationLocked.value || !openSessionCount.value,
  },
]);

const activeView = computed({
  get: () => (sessionStore.activeSessionId ? "workspace" : "hub"),
  set: (view) => {
    if (navigationLocked.value) return;
    if (view === "hub") sessionStore.showConnectionHub();
    else sessionStore.showWorkspace();
  },
});

async function startWindowDrag(event) {
  if (event.button !== 0 || event.target.closest("button, a, [role='tab']")) return;
  if (!appWindow) return;

  try {
    await appWindow.startDragging();
  } catch (error) {
    console.warn("Failed to start window drag", error);
  }
}
</script>

<template>
  <header class="window-chrome" @mousedown="startWindowDrag">
    <div class="window-chrome-start">
      <span v-if="isMac" class="window-traffic-light-space" aria-hidden="true"></span>
      <div class="window-brand">
        <span class="window-brand-avatar">
          <img :src="brandAvatar" alt="" draggable="false" />
        </span>
        <span>{{ t("app.name") }}</span>
      </div>
    </div>

    <UTabs
      v-if="!appStore.isLoading"
      v-model="activeView"
      v-liquid-glass="GLASS_CLEAR"
      v-liquid-stretch="activeView"
      class="window-view-switch liquid-glass liquid-glass--clear"
      variant="pill"
      size="md"
      :items="viewItems"
      :content="false"
      :aria-label="t('workspace.switchView')"
      :ui="{
        list: 'window-view-list',
        indicator: 'window-view-indicator',
        trigger: 'window-view-trigger',
      }"
    />

    <div class="window-chrome-end">
      <!-- 弹层（文件操作面板、提示）都经 portal 挂到 body，不会被这颗胶囊的 backdrop-filter 关住 -->
      <div
        v-if="!appStore.isLoading"
        v-liquid-glass="GLASS_CLEAR"
        class="window-tools liquid-glass liquid-glass--clear"
        @mousedown.stop
      >
        <FileOperationPanel />
        <UChip
          :show="transferStore.summary.activeCount > 0"
          :text="transferStore.summary.activeCount"
          size="3xs"
          color="primary"
          inset
        >
          <UButton
            color="neutral"
            variant="ghost"
            icon="i-lucide-arrow-down-up"
            :label="t('transfers.short')"
            :class="{ 'transfer-entry-active': transferStore.panelOpen }"
            :aria-label="t('transfers.togglePanel')"
            :title="t('transfers.togglePanel')"
            @click.stop="transferStore.togglePanel"
          />
        </UChip>
        <UButton
          color="neutral"
          variant="ghost"
          icon="i-lucide-settings"
          :label="t('app.settings')"
          @click.stop="uiStore.openSettings"
        />
      </div>
      <WindowControls v-if="!isMac" />
    </div>
  </header>
</template>

<style scoped src="../../assets/css/window-chrome.css"></style>
