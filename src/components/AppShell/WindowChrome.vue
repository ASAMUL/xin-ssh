<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import SessionTabs from "../SessionTabs.vue";
import { useAppStore } from "../../stores/appStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useTransferStore } from "../../stores/transferStore";
import { useUiStore } from "../../stores/uiStore";

const { t } = useI18n();
const appStore = useAppStore();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();
const appWindow = window.__TAURI_INTERNALS__ ? getCurrentWindow() : null;
const platform = navigator.userAgentData?.platform ?? navigator.platform ?? "";

const isMac = computed(() => platform.toLowerCase().includes("mac"));

async function startWindowDrag(event) {
  if (event.target.closest("button")) return;
  if (!appWindow) return;

  try {
    await appWindow.startDragging();
  } catch (error) {
    console.warn("Failed to start window drag", error);
  }
}

async function minimizeWindow() {
  if (!appWindow) return;

  await appWindow.minimize();
}

async function toggleMaximizeWindow() {
  if (!appWindow) return;

  await appWindow.toggleMaximize();
}

async function closeWindow() {
  if (!appWindow) return;

  await appWindow.close();
}
</script>

<template>
  <header class="window-chrome" @mousedown="startWindowDrag">
    <div class="window-brand">
      <span class="app-dot">
        <UIcon name="i-lucide-terminal" />
      </span>
      <span>{{ t("app.name") }}</span>
    </div>

    <SessionTabs
      v-if="!appStore.isLoading"
      :sessions="sessionStore.sessions"
      :active-session-id="sessionStore.activeSessionId"
      @show-connection-hub="sessionStore.showConnectionHub"
      @select="sessionStore.selectSession"
      @close="sessionStore.closeSession"
      @create="uiStore.openConnectionForm"
    />
    <div v-else class="window-drag-space"></div>

    <div v-if="!appStore.isLoading" class="window-tools" @mousedown.stop>
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
          size="sm"
          icon="i-lucide-arrow-down-up"
          :class="{ 'transfer-entry-active': transferStore.panelOpen }"
          :aria-label="t('transfers.togglePanel')"
          :title="t('transfers.togglePanel')"
          @click.stop="transferStore.togglePanel"
        />
      </UChip>
      <UButton
        color="neutral"
        variant="ghost"
        size="sm"
        icon="i-lucide-settings"
        :aria-label="t('app.settings')"
        :title="t('app.settings')"
        @click.stop="uiStore.openSettings"
      />
    </div>

    <div class="window-controls" :class="{ mac: isMac }" @mousedown.stop>
      <button
        type="button"
        class="window-control minimize"
        :aria-label="t('window.minimize')"
        @click.stop="minimizeWindow"
      >
        <UIcon v-if="!isMac" name="i-lucide-minus" />
      </button>
      <button
        type="button"
        class="window-control maximize"
        :aria-label="t('window.maximize')"
        @click.stop="toggleMaximizeWindow"
      >
        <UIcon v-if="!isMac" name="i-lucide-square" />
      </button>
      <button
        type="button"
        class="window-control close"
        :aria-label="t('window.close')"
        @click.stop="closeWindow"
      >
        <UIcon v-if="!isMac" name="i-lucide-x" />
      </button>
    </div>
  </header>
</template>
