<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "vue-i18n";
import { useEditorStore } from "../../stores/editorStore";
import { useFileOperationStore } from "../../stores/fileOperationStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useTransferStore } from "../../stores/transferStore";
import { useTunnelStore } from "../../stores/tunnelStore";
import { useUiStore } from "../../stores/uiStore";
import { localizeError } from "../../utils/appError";

const { t } = useI18n();
const editorStore = useEditorStore();
const fileOperationStore = useFileOperationStore();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
const tunnelStore = useTunnelStore();
const uiStore = useUiStore();
const appWindow = window.__TAURI_INTERNALS__ ? getCurrentWindow() : null;
const isClosing = ref(false);
const error = ref("");
let unlistenClose = null;
let disposed = false;

const hasUnsavedFile = computed(() => editorStore.isOpen && editorStore.isDirty);
const sessionCount = computed(() => sessionStore.sessions.filter(
  (session) => ["connected", "connecting"].includes(session.status),
).length);
const transferCount = computed(() => transferStore.summary.activeCount);
const operationCount = computed(() => fileOperationStore.activeCount);
const tunnelCount = computed(() => Object.values(tunnelStore.runtime).filter(
  (tunnel) => ["running", "starting", "stopping"].includes(tunnel.status),
).length);
const needsConfirmation = computed(() => hasUnsavedFile.value || editorStore.isSaving
  || sessionCount.value || transferCount.value || tunnelCount.value || operationCount.value);

onMounted(async () => {
  if (!appWindow) return;
  try {
    const unlisten = await appWindow.onCloseRequested((event) => {
      // 系统菜单和 Alt+F4 也必须经过同一入口，不能只保护自绘关闭按钮。
      event.preventDefault();
      uiStore.requestWindowClose();
    });
    if (disposed) unlisten();
    else unlistenClose = unlisten;
  } catch (closeError) {
    uiStore.setError(closeError);
  }
});

onBeforeUnmount(() => {
  disposed = true;
  unlistenClose?.();
});

watch(() => uiStore.isWindowCloseOpen, (open) => {
  if (!open) return;
  error.value = "";
  if (!needsConfirmation.value) finishClose(false);
});

function cancelClose() {
  if (!isClosing.value) uiStore.cancelWindowClose();
}

// 保留编辑器直到窗口真正关闭；保存或关闭失败时，用户仍能继续编辑原文档。
async function finishClose(saveChanges) {
  if (!appWindow || isClosing.value || editorStore.isSaving) return;
  isClosing.value = true;
  error.value = "";
  try {
    if (saveChanges && hasUnsavedFile.value) {
      const saved = await editorStore.saveCurrentFile();
      if (!saved || hasUnsavedFile.value) {
        error.value = localizeError(editorStore.saveError) || t("window.exit.saveIncomplete");
        return;
      }
    }
    // 用户已确认，destroy 避免再次触发 close-requested 造成确认循环。
    await appWindow.destroy();
  } catch (closeError) {
    error.value = localizeError(closeError);
  } finally {
    isClosing.value = false;
  }
}
</script>

<template>
  <UModal
    :open="uiStore.isWindowCloseOpen"
    :title="t('window.exit.title')"
    :description="t('window.exit.description')"
    :dismissible="!isClosing"
    :close="!isClosing"
    :ui="{ footer: 'flex-wrap justify-end' }"
    @update:open="(open) => !open && cancelClose()"
  >
    <template #body>
      <div class="window-close-details">
        <UAlert v-if="hasUnsavedFile" color="warning" variant="soft" icon="i-lucide-file-pen-line"
          :title="t('window.exit.unsavedFile')" :description="editorStore.path" :ui="{ description: 'break-all' }" />
        <UAlert v-if="editorStore.isSaving" color="warning" variant="soft" icon="i-lucide-loader-circle"
          :title="t('window.exit.saving')" />
        <UAlert v-if="transferCount" color="warning" variant="soft" icon="i-lucide-arrow-down-up"
          :title="t('window.exit.transfers', { count: transferCount })" />
        <UAlert v-if="operationCount" color="warning" variant="soft" icon="i-lucide-trash-2"
          :title="t('window.exit.operations', { count: operationCount })" />
        <UAlert v-if="sessionCount" color="neutral" variant="soft" icon="i-lucide-terminal"
          :title="t('window.exit.sessions', { count: sessionCount })" />
        <UAlert v-if="tunnelCount" color="neutral" variant="soft" icon="i-lucide-waypoints"
          :title="t('window.exit.tunnels', { count: tunnelCount })" />
        <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-circle-alert" :title="error" />
      </div>
    </template>
    <template #footer>
      <UButton color="neutral" variant="outline" :label="t('common.cancel')" :disabled="isClosing" @click="cancelClose" />
      <UButton color="error" variant="soft" icon="i-lucide-log-out"
        :label="t(hasUnsavedFile ? 'window.exit.discard' : 'window.exit.confirm')"
        :disabled="isClosing || editorStore.isSaving" @click="finishClose(false)" />
      <UButton v-if="hasUnsavedFile" color="primary" icon="i-lucide-save"
        :label="t('window.exit.save')" :loading="isClosing" :disabled="editorStore.isSaving"
        @click="finishClose(true)" />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/window-close-guard.css"></style>
