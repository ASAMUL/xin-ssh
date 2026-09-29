<script setup>
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUiStore } from "../../stores/uiStore";

// macOS 放在标题栏最左且是红黄绿圆点，其余平台在最右用线性图标
defineProps({ mac: { type: Boolean, default: false } });

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
  <div class="window-controls" :class="{ mac }" @mousedown.stop>
    <button
      type="button"
      class="window-control minimize"
      :aria-label="t('window.minimize')"
      @click.stop="minimizeWindow"
    >
      <UIcon v-if="!mac" name="i-lucide-minus" />
    </button>
    <button
      type="button"
      class="window-control maximize"
      :aria-label="t('window.maximize')"
      @click.stop="toggleMaximizeWindow"
    >
      <UIcon v-if="!mac" name="i-lucide-square" />
    </button>
    <button
      type="button"
      class="window-control close"
      :aria-label="t('window.close')"
      @click.stop="closeWindow"
    >
      <UIcon v-if="!mac" name="i-lucide-x" />
    </button>
  </div>
</template>

<style scoped src="../../assets/css/window-controls.css"></style>
