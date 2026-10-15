<script setup>
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUiStore } from "../../stores/uiStore";

// 只给 Windows / Linux 用：macOS 走系统原生红绿灯（见 tauri.macos.conf.json）
const { t } = useI18n();
const uiStore = useUiStore();
const appWindow = window.__TAURI_INTERNALS__ ? getCurrentWindow() : null;

async function minimizeWindow() {
  await appWindow?.minimize();
}

async function toggleMaximizeWindow() {
  await appWindow?.toggleMaximize();
}

// 关闭走应用层确认（未保存文件 / 传输中任务），不直接关窗
function closeWindow() {
  if (appWindow) uiStore.requestWindowClose();
}
</script>

<template>
  <div class="window-controls" @mousedown.stop>
    <button
      type="button"
      class="window-control minimize"
      :aria-label="t('window.minimize')"
      @click.stop="minimizeWindow"
    >
      <UIcon name="i-lucide-minus" />
    </button>
    <button
      type="button"
      class="window-control maximize"
      :aria-label="t('window.maximize')"
      @click.stop="toggleMaximizeWindow"
    >
      <UIcon name="i-lucide-square" />
    </button>
    <button
      type="button"
      class="window-control close"
      :aria-label="t('window.close')"
      @click.stop="closeWindow"
    >
      <UIcon name="i-lucide-x" />
    </button>
  </div>
</template>

<style scoped src="../../assets/css/window-controls.css"></style>
