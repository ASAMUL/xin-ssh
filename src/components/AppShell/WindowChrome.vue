<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";

const { t } = useI18n();
const appWindow = getCurrentWindow();
const platform = navigator.userAgentData?.platform ?? navigator.platform ?? "";

const isMac = computed(() => platform.toLowerCase().includes("mac"));

async function startWindowDrag(event) {
  if (event.target.closest("button")) return;

  try {
    await appWindow.startDragging();
  } catch (error) {
    console.warn("Failed to start window drag", error);
  }
}

async function minimizeWindow() {
  await appWindow.minimize();
}

async function toggleMaximizeWindow() {
  await appWindow.toggleMaximize();
}

async function closeWindow() {
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

    <div class="window-drag-space"></div>

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
