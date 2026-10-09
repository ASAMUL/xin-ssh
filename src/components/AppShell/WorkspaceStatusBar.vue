<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useToast } from "@nuxt/ui/composables";
import { useTimestamp } from "@vueuse/core";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useTransferStore } from "../../stores/transferStore";
import { useUiStore } from "../../stores/uiStore";
import { useSessionLatency } from "../../composables/useSessionLatency";
import { formatUptimeShort } from "../../utils/formatMetrics";

const { t } = useI18n();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();
const toast = useToast();
useSessionLatency(sessionStore);

const session = computed(() => sessionStore.activeSession);
const profileStore = useProfileStore();
const isProduction = computed(() => profileStore.isProductionProfile(session.value?.profileId));
const statusLabel = computed(() => (session.value ? t(`monitor.status.${session.value.status}`) : ""));
// 连接目标原来在终端头部左边；头部改成浮在画面上的胶囊后挪到这里，免得压住终端每行的开头
const endpoint = computed(() => (session.value
  ? `${session.value.username}@${session.value.host}:${session.value.port}`
  : ""));
const latencyLabel = computed(() => {
  const latency = session.value?.latencyMs;
  return Number.isFinite(latency) ? `${latency} ms` : "--";
});
// 100ms 内几乎感觉不到，300ms 起敲字能明显感到回显慢
const latencyLevel = computed(() => {
  const latency = session.value?.latencyMs;
  if (!Number.isFinite(latency)) return "unknown";
  if (latency < 100) return "good";
  if (latency < 300) return "fair";
  return "poor";
});
// 只精确到分钟：按秒跳的读数会一直勾眼角，而「连了多久」本来也只需要个大概
const now = useTimestamp({ interval: 15_000 });
// 重连会把 connectedAt 重置，时长归零就等于告诉用户刚才断过一次
const connectedFor = computed(() => {
  const connectedAt = session.value?.status === "connected" ? session.value.connectedAt : null;
  if (!connectedAt) return "";
  const seconds = Math.floor((now.value - connectedAt) / 1000);
  return seconds < 60 ? t("statusBar.connectedJustNow") : formatUptimeShort(seconds, t);
});
// 终端里点端口开的临时转发：一条一组，打开 / 复制地址 / 停止
const forwards = computed(() => session.value?.portForwards ?? []);
const forwardMenuItems = computed(() => forwards.value.map((forward) => [
  { type: "label", label: t("statusBar.forwardItem", { remote: forward.remotePort, local: forward.localPort }) },
  {
    label: t("statusBar.forwardOpen"),
    icon: "i-lucide-external-link",
    onSelect: () => openUrl(`http://localhost:${forward.localPort}/`).catch((error) => uiStore.setError(error)),
  },
  { label: t("statusBar.forwardCopy"), icon: "i-lucide-copy", onSelect: () => copyForwardAddress(forward) },
  {
    label: t("statusBar.forwardStop"),
    icon: "i-lucide-circle-stop",
    color: "error",
    onSelect: () => sessionStore.stopForward(session.value.id, forward.localPort).catch((error) => uiStore.setError(error)),
  },
]));

async function copyForwardAddress(forward) {
  const address = `127.0.0.1:${forward.localPort}`;
  try {
    await navigator.clipboard.writeText(address);
    toast.add({ title: t("statusBar.forwardCopied", { address }), icon: "i-lucide-copy", color: "success" });
  } catch (error) {
    uiStore.setError(error);
  }
}
</script>

<template>
  <footer v-liquid-glass="{ scale: 12, blur: 3 }" class="workspace-status-bar liquid-glass">
    <span class="status-item status-connection" :class="session?.status" :title="statusLabel">
      <i></i>
      <span>SSH</span>
    </span>
    <span v-if="isProduction" class="status-item">
      <span class="production-badge">{{ t("profile.productionBadge") }}</span>
    </span>
    <span v-if="endpoint" class="status-item status-endpoint" :title="t('statusBar.endpoint')">
      <span>{{ endpoint }}</span>
    </span>
    <span class="status-item">UTF-8</span>
    <span class="status-item" :title="t('statusBar.latencyHint')">
      {{ t("statusBar.latency") }}
      <strong class="status-latency" :class="latencyLevel">{{ latencyLabel }}</strong>
    </span>
    <UButton
      class="status-item status-transfers"
      :class="{ active: transferStore.panelOpen }"
      color="neutral"
      variant="ghost"
      size="xs"
      trailing-icon="i-lucide-arrow-down-up"
      :title="t('transfers.togglePanel')"
      @click="transferStore.togglePanel"
    >
      {{ t("statusBar.transfers") }}
      <strong>{{ transferStore.summary.activeCount }}</strong>
    </UButton>
    <UDropdownMenu v-if="forwards.length" :items="forwardMenuItems" :content="{ side: 'top', align: 'start' }">
      <UButton
        class="status-item status-forwards"
        color="neutral"
        variant="ghost"
        size="xs"
        trailing-icon="i-lucide-arrow-right-left"
        :title="t('statusBar.forwardsHint')"
      >
        {{ t("statusBar.forwards") }}
        <strong>{{ forwards.length }}</strong>
      </UButton>
    </UDropdownMenu>

    <span class="status-spacer"></span>
    <span v-if="connectedFor" class="status-item" :title="t('statusBar.connectedHint')">
      {{ t("statusBar.connectedFor") }}
      <strong>{{ connectedFor }}</strong>
    </span>
  </footer>
</template>

<style scoped src="../../assets/css/workspace-status-bar.css"></style>
