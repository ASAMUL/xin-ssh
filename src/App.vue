<script setup>
import { computed, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@nuxt/ui/composables";
import { en, zh_cn } from "@nuxt/ui/locale";
import TransferPanel from "./components/TransferPanel.vue";
import WindowChrome from "./components/AppShell/WindowChrome.vue";
import WindowCloseGuard from "./components/AppShell/WindowCloseGuard.vue";
import WorkspaceShell from "./components/AppShell/WorkspaceShell.vue";
import { revealTaskInFolder } from "./services/localOpener";
import { useAppStore } from "./stores/appStore";
import { useFileExplorerStore } from "./stores/fileExplorerStore";
import { useFileOperationStore } from "./stores/fileOperationStore";
import { useSessionStore } from "./stores/sessionStore";
import { useTransferStore } from "./stores/transferStore";
import { useTunnelStore } from "./stores/tunnelStore";
import { useUiStore } from "./stores/uiStore";
import { useProfileStore } from "./stores/profileStore";
import { localizeError } from "./utils/appError";

const { t, locale } = useI18n();
const toast = useToast();
const appStore = useAppStore();
const fileExplorerStore = useFileExplorerStore();
const fileOperationStore = useFileOperationStore();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
const tunnelStore = useTunnelStore();
const uiStore = useUiStore();
const profileStore = useProfileStore();
const uiLocale = computed(() => (locale.value === "zh-CN" ? zh_cn : en));

function showError(error) {
  toast.add({
    title: t("errors.title"),
    description: localizeError(error),
    color: "error",
    icon: "i-lucide-circle-alert",
    duration: 6000,
  });
}

// 每次消费后清空，让连续发生的相同错误也能再次提示。
watch(
  () => uiStore.error,
  (error) => {
    if (!error) return;
    uiStore.clearError();
    showError(error);
  },
  { flush: "sync" },
);

watch(
  () => appStore.error,
  (error) => {
    if (!error) return;
    appStore.error = null;
    showError(error);
  },
  { flush: "sync" },
);

let credentialCleanupToastId = null;

// 重试动作会关闭旧 toast，每次使用新标识，避免旧通知的延迟销毁删掉新的重试入口。
function showCredentialCleanupToast(error = null) {
  if (credentialCleanupToastId) toast.remove(credentialCleanupToastId);
  credentialCleanupToastId = null;
  if (!profileStore.profilesAwaitingCredentialCleanup.length) return;
  const notification = toast.add({
    title: t("home.credentialCleanup.title"),
    description: error ? localizeError(error) : t("home.credentialCleanup.description"),
    color: "warning",
    icon: "i-lucide-key-round",
    duration: 0,
    actions: [
      {
        label: t("common.retry"),
        icon: "i-lucide-refresh-cw",
        onClick: async () => {
          try {
            await profileStore.retryCredentialCleanup();
          } catch (error) {
            showCredentialCleanupToast(error);
          }
        },
      },
    ],
  });
  credentialCleanupToastId = notification.id;
}

// 待清理引用保存在删除记录里，重启后仍能给用户提供重试入口。
watch(
  () => profileStore.profilesAwaitingCredentialCleanup.map((profile) => profile.id).sort().join(","),
  () => showCredentialCleanupToast(),
);

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
  fileOperationStore.startListening().catch((error) => uiStore.setError(error));
  tunnelStore.startListening();
});
</script>

<template>
  <UApp :locale="uiLocale" :toaster="{ position: 'top-right' }">
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
      </section>
    </main>
    <WindowCloseGuard />
  </UApp>
</template>
