<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "../../stores/sessionStore";
import { useTransferStore } from "../../stores/transferStore";
import { useSessionLatency } from "../../composables/useSessionLatency";

const { t } = useI18n();
const sessionStore = useSessionStore();
const transferStore = useTransferStore();
useSessionLatency(sessionStore);

const session = computed(() => sessionStore.activeSession);
const statusLabel = computed(() => (session.value ? t(`monitor.status.${session.value.status}`) : ""));
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
const terminalSize = computed(() => (session.value
  ? `${session.value.terminalColumns} × ${session.value.terminalRows}`
  : ""));
</script>

<template>
  <footer v-liquid-glass="{ scale: 12, blur: 3 }" class="workspace-status-bar liquid-glass">
    <span class="status-item status-connection" :class="session?.status" :title="statusLabel">
      <i></i>
      <span>SSH</span>
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

    <span class="status-spacer"></span>
    <span v-if="terminalSize" class="status-item status-muted" :title="t('statusBar.terminalSize')">
      {{ terminalSize }}
    </span>
  </footer>
</template>

<style scoped src="../../assets/css/workspace-status-bar.css"></style>
