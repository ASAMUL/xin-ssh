<script setup>
import { computed, onBeforeUnmount, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSessionStore } from "../../stores/sessionStore";

const { t, locale } = useI18n();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
let refreshTimer = null;

const activeSession = computed(() => sessionStore.activeSession);
const systemInfo = computed(() => activeSession.value?.systemInfo ?? null);
const isConnected = computed(() => activeSession.value?.status === "connected");
const endpoint = computed(() => {
  const session = activeSession.value;
  if (!session) return "--";

  return `${session.username}@${session.host}`;
});
const statusLabel = computed(() => {
  const session = activeSession.value;
  if (!session) return t("monitor.noSession");

  return t(`monitor.status.${session.status}`);
});
const selectedRefreshSeconds = computed({
  get: () => settingsStore.monitorRefreshSeconds,
  set: (value) => {
    settingsStore.setMonitorRefreshSeconds(Number(value));
  },
});
const refreshOptions = computed(() => [
  { label: t("monitor.refreshOption", { seconds: 2 }), value: 2 },
  { label: t("monitor.refreshOption", { seconds: 5 }), value: 5 },
  { label: t("monitor.refreshOption", { seconds: 10 }), value: 10 },
  { label: t("monitor.refreshOption", { seconds: 30 }), value: 30 },
]);
const metricRows = computed(() => {
  const cpu = systemInfo.value?.cpu ?? {};
  const memory = systemInfo.value?.memory ?? {};
  const swap = systemInfo.value?.swap ?? {};
  const cpuDetail = [
    cpu.coreCount ? t("monitor.coreCount", { count: cpu.coreCount }) : null,
    cpu.modelName,
  ].filter(Boolean).join(" · ");

  return [
    {
      id: "cpu",
      icon: "i-lucide-cpu",
      label: t("monitor.cpu"),
      value: formatPercent(cpu.usagePercent),
      detail: cpuDetail || "--",
      percent: progressPercent(cpu.usagePercent),
    },
    {
      id: "memory",
      icon: "i-lucide-memory-stick",
      label: t("monitor.memory"),
      value: formatPercent(memory.usagePercent),
      detail: formatMemoryPair(memory),
      percent: progressPercent(memory.usagePercent),
    },
    {
      id: "swap",
      icon: "i-lucide-hard-drive",
      label: t("monitor.swap"),
      value: formatPercent(swap.usagePercent),
      detail: formatMemoryPair(swap),
      percent: progressPercent(swap.usagePercent),
    },
  ];
});
const processRows = computed(() => (systemInfo.value?.processes ?? []).slice(0, 6));
const diskRows = computed(() => (systemInfo.value?.disks ?? []).slice(0, 3));
const primaryNetwork = computed(() => systemInfo.value?.networks?.[0] ?? null);
const networkBars = computed(() => {
  const history = activeSession.value?.networkHistory ?? [];
  const maxRate = Math.max(...history.map((sample) => sample.bytesPerSecond), 1);
  const emptySlots = Math.max(28 - history.length, 0);
  const samples = [
    ...Array.from({ length: emptySlots }, () => null),
    ...history.slice(-28),
  ];

  return samples.map((sample, index) => ({
    id: sample?.id ?? `empty-${index}`,
    height: sample ? 8 + Math.round((sample.bytesPerSecond / maxRate) * 54) : 5,
    active: Boolean(sample),
  }));
});
const networkRate = computed(() => {
  const latestSample = activeSession.value?.networkHistory?.at(-1);
  return formatBytesPerSecond(latestSample?.bytesPerSecond);
});
const lastUpdatedLabel = computed(() => {
  const updatedAt = activeSession.value?.systemInfoUpdatedAt;
  if (!updatedAt) return "--";

  return new Intl.DateTimeFormat(locale.value, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(updatedAt));
});
const uptimeLabel = computed(() => formatUptime(systemInfo.value?.uptimeSeconds));
const loadAverageLabel = computed(() => {
  const loadAverage = systemInfo.value?.loadAverage ?? [];
  if (!loadAverage.length) return "--";

  return loadAverage.map((value) => value.toFixed(2)).join(" / ");
});

watch(
  () => [
    activeSession.value?.id,
    activeSession.value?.status,
    settingsStore.monitorRefreshSeconds,
  ],
  () => restartRefreshLoop(),
  { immediate: true },
);

onBeforeUnmount(() => {
  stopRefreshLoop();
});

function restartRefreshLoop() {
  stopRefreshLoop();
  if (!isConnected.value) return;

  refreshCurrentSystemInfo();
  refreshTimer = window.setInterval(
    () => refreshCurrentSystemInfo(),
    Math.max(settingsStore.monitorRefreshSeconds, 2) * 1000,
  );
}

function stopRefreshLoop() {
  if (!refreshTimer) return;

  window.clearInterval(refreshTimer);
  refreshTimer = null;
}

function refreshCurrentSystemInfo() {
  if (!activeSession.value || !isConnected.value) return;

  sessionStore.refreshSystemInfo(activeSession.value.id);
}

function progressPercent(value) {
  return Math.min(Math.max(Number(value) || 0, 0), 100);
}

function formatPercent(value) {
  if (value === null || value === undefined) return "--";

  return `${Number(value).toFixed(Number(value) % 1 === 0 ? 0 : 1)}%`;
}

function formatMemoryPair(metric = {}) {
  if (!metric.totalBytes) return "--";

  return `${formatBytes(metric.usedBytes)} / ${formatBytes(metric.totalBytes)}`;
}

function formatBytes(value) {
  if (value === null || value === undefined) return "--";

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = Number(value);
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatBytesPerSecond(value) {
  if (!value) return "--";

  return `${formatBytes(value)}/s`;
}

function formatUptime(seconds) {
  if (!seconds) return "--";

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (days > 0) return t("monitor.uptimeDays", { days, hours });
  if (hours > 0) return t("monitor.uptimeHours", { hours, minutes });

  return t("monitor.uptimeMinutes", { minutes });
}
</script>

<template>
  <aside class="host-monitor-sidebar" :aria-label="t('monitor.systemInfo')">
    <section class="monitor-block compact">
      <div class="sync-row">
        <span>{{ t("monitor.syncStatus") }}</span>
        <span class="monitor-dot" :class="{ active: isConnected }"></span>
      </div>
      <div class="monitor-ip">
        <small>{{ t("monitor.host") }}</small>
        <strong>{{ activeSession?.host ?? "--" }}</strong>
      </div>
      <div class="monitor-actions">
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :aria-label="t('monitor.refreshNow')"
          :disabled="!isConnected || activeSession?.systemInfoLoading"
          :loading="activeSession?.systemInfoLoading"
          @click="refreshCurrentSystemInfo"
        />
        <label class="refresh-control">
          <span>{{ t("monitor.refreshEvery") }}</span>
          <USelect
            v-model.number="selectedRefreshSeconds"
            :items="refreshOptions"
            value-key="value"
            label-key="label"
            size="xs"
            class="monitor-refresh-select"
          />
        </label>
      </div>
    </section>

    <section class="monitor-block">
      <dl class="monitor-summary">
        <div>
          <dt>{{ t("monitor.endpoint") }}</dt>
          <dd>{{ endpoint }}</dd>
        </div>
        <div>
          <dt>{{ t("monitor.state") }}</dt>
          <dd>{{ statusLabel }}</dd>
        </div>
        <div>
          <dt>{{ t("monitor.updatedAt") }}</dt>
          <dd>{{ lastUpdatedLabel }}</dd>
        </div>
        <div>
          <dt>{{ t("monitor.uptime") }}</dt>
          <dd>{{ uptimeLabel }}</dd>
        </div>
        <div>
          <dt>{{ t("monitor.loadAverage") }}</dt>
          <dd>{{ loadAverageLabel }}</dd>
        </div>
      </dl>

      <div class="metric-list">
        <div v-for="metric in metricRows" :key="metric.id" class="metric-row">
          <div class="metric-label">
            <span>
              <UIcon :name="metric.icon" />
              {{ metric.label }}
            </span>
            <strong>{{ metric.value }}</strong>
          </div>
          <small>{{ metric.detail }}</small>
          <span class="metric-track">
            <span :style="{ width: `${metric.percent}%` }"></span>
          </span>
        </div>
      </div>

      <p v-if="activeSession?.systemInfoError" class="monitor-error">
        {{ activeSession.systemInfoError }}
      </p>
    </section>

    <section class="monitor-block grow">
      <div class="mini-table process-table">
        <div class="mini-heading">
          <span>{{ t("monitor.cpu") }}</span>
          <span>{{ t("monitor.memory") }}</span>
          <span>{{ t("monitor.command") }}</span>
        </div>
        <div v-if="!processRows.length" class="mini-row empty">
          <span>{{ t("monitor.noData") }}</span>
        </div>
        <div v-for="process in processRows" :key="process.pid" class="mini-row">
          <span>{{ formatPercent(process.cpuPercent) }}</span>
          <span>{{ formatBytes(process.memoryBytes) }}</span>
          <span :title="process.command">{{ process.command }}</span>
        </div>
      </div>

      <div class="network-panel">
        <div class="network-head">
          <span>{{ t("monitor.network") }}</span>
          <strong>{{ primaryNetwork?.name ?? "--" }}</strong>
        </div>
        <div class="network-totals">
          <span>{{ t("monitor.rx") }} {{ formatBytes(primaryNetwork?.rxBytes) }}</span>
          <span>{{ t("monitor.tx") }} {{ formatBytes(primaryNetwork?.txBytes) }}</span>
        </div>
        <div class="network-chart" aria-hidden="true">
          <span
            v-for="bar in networkBars"
            :key="bar.id"
            :class="{ active: bar.active }"
            :style="{ height: `${bar.height}px` }"
          ></span>
        </div>
        <small>{{ t("monitor.rate") }} {{ networkRate }}</small>
      </div>
    </section>

    <section class="monitor-block disk-block">
      <div class="mini-table disk-table">
        <div class="mini-heading">
          <span>{{ t("monitor.path") }}</span>
          <span>{{ t("monitor.used") }}</span>
        </div>
        <div v-if="!diskRows.length" class="mini-row empty">
          <span>{{ t("monitor.noData") }}</span>
        </div>
        <div v-for="disk in diskRows" :key="disk.path" class="mini-row">
          <span>{{ disk.path }}</span>
          <span>{{ formatPercent(disk.usagePercent) }}</span>
        </div>
      </div>
    </section>
  </aside>
</template>

<style scoped src="../../assets/css/host-monitor.css"></style>
