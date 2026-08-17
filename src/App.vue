<script setup>
import { computed, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@nuxt/ui/composables";
import TransferPanel from "./components/TransferPanel.vue";
import WindowChrome from "./components/AppShell/WindowChrome.vue";
import WorkspaceShell from "./components/AppShell/WorkspaceShell.vue";
import { revealTaskInFolder } from "./services/localOpener";
import { useAppStore } from "./stores/appStore";
import { useFileExplorerStore } from "./stores/fileExplorerStore";
import { useSessionStore } from "./stores/sessionStore";
import { useTransferStore } from "./stores/transferStore";
import { useTunnelStore } from "./stores/tunnelStore";
import { useUiStore } from "./stores/uiStore";

const { t } = useI18n();
const toast = useToast();
const appStore = useAppStore();
const fileExplorerStore = useFileExplorerStore();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
const tunnelStore = useTunnelStore();
const uiStore = useUiStore();

const visibleError = computed(() => appStore.error || uiStore.error);

// 面板关闭时下载完成 → 弹快捷提示，可直接打开本地目录或查看记录
watch(
  () => transferStore.downloadReadySignal,
  (signal) => {
    if (!signal) return;

    transferStore.consumeDownloadReadySignal();
    const task = transferStore.findTask(signal.taskId);
    if (!task) return;

    toast.add({
      title: t("transfers.downloadReadyTitle"),
      description: task.label,
      icon: "i-lucide-circle-check",
      color: "success",
      duration: 6000,
      actions: [
        {
          label: t("transfers.revealInFolder"),
          icon: "i-lucide-folder-open",
          color: "neutral",
          variant: "outline",
          onClick: () => {
            revealTaskInFolder(task).catch((error) => uiStore.setError(error));
          },
        },
        {
          label: t("transfers.viewHistory"),
          color: "neutral",
          variant: "ghost",
          onClick: () => {
            transferStore.panelOpen = true;
          },
        },
      ],
    });
  },
);

onMounted(() => {
  appStore.bootstrap();
  // 终端事件监听挂在应用层：切到连接中心（终端组件卸载）时也不丢会话状态事件
  sessionStore.startListening();
  transferStore.startListening();
  fileExplorerStore.startListening();
  tunnelStore.startListening();
});
</script>

<template>
  <UApp>
    <main class="desktop-window">
      <WindowChrome />

      <section class="desktop-content" aria-label="Xin Terminal">
        <div v-if="appStore.isLoading" class="startup-state">
          <div class="startup-mark">
            <UIcon name="i-lucide-terminal" />
          </div>
          <p>{{ t("app.loading") }}</p>
        </div>

        <WorkspaceShell v-else />

        <TransferPanel v-if="transferStore.panelOpen" />

        <div v-if="visibleError" class="error-banner">
          <strong>{{ t("errors.title") }}</strong>
          <span>{{ visibleError }}</span>
        </div>
      </section>
    </main>
  </UApp>
</template>
