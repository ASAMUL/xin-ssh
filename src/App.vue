<script setup>
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import TransferToast from "./components/TransferToast.vue";
import WindowChrome from "./components/AppShell/WindowChrome.vue";
import WorkspaceShell from "./components/AppShell/WorkspaceShell.vue";
import { useAppStore } from "./stores/appStore";
import { useFileExplorerStore } from "./stores/fileExplorerStore";
import { useSessionStore } from "./stores/sessionStore";
import { useTransferStore } from "./stores/transferStore";
import { useTunnelStore } from "./stores/tunnelStore";
import { useUiStore } from "./stores/uiStore";

const { t } = useI18n();
const appStore = useAppStore();
const fileExplorerStore = useFileExplorerStore();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
const tunnelStore = useTunnelStore();
const uiStore = useUiStore();

const visibleError = computed(() => appStore.error || uiStore.error);

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

        <TransferToast
          v-if="transferStore.hasVisibleTasks"
          :summary="transferStore.summary"
          :tasks="transferStore.visibleTasks"
          @clear-finished="transferStore.clearFinished"
          @cancel="transferStore.cancelTask"
        />

        <div v-if="visibleError" class="error-banner">
          <strong>{{ t("errors.title") }}</strong>
          <span>{{ visibleError }}</span>
        </div>
      </section>
    </main>
  </UApp>
</template>
