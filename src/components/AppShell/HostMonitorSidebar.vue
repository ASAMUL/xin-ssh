<script setup>
import { computed, onBeforeUnmount, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSessionStore } from "../../stores/sessionStore";

const PROCESS_ROW_LIMIT = 5;
const DISK_ROW_LIMIT = 3;
const NETWORK_CHART_SAMPLE_LIMIT = 30;
const NETWORK_CHART_WIDTH = 220;
const NETWORK_CHART_HEIGHT = 86;
const NETWORK_CHART_PADDING = 8;

const { t, locale } = useI18n();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
let refreshTimer = null;

const cardUi = {
  body: "p-0",
};
const tableUi = {
  root: "monitor-table-root",
  base: "monitor-table-base",
  thead: "monitor-table-thead",
  tbody: "monitor-table-body",
  tr: "monitor-table-row",
  th: "monitor-table-head",
  td: "monitor-table-cell",
  empty: "monitor-table-empty",
};
const progressUi = {
  root: "monitor-progress-root",
  base: "monitor-progress-base",
  indicator: "monitor-progress-indicator",
};

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
const statusColor = computed(() => getStatusColor(activeSession.value?.status));
const syncBadgeLabel = computed(() => (isConnected.value ? t("monitor.synced") : statusLabel.value));
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
const summaryRows = computed(() => [
  {
    id: "endpoint",
    label: t("monitor.endpoint"),
    value: endpoint.value,
  },
  {
    id: "state",
    label: t("monitor.state"),
    value: statusLabel.value,
  },
  {
    id: "updatedAt",
    label: t("monitor.updatedAt"),
    value: lastUpdatedLabel.value,
  },
  {
    id: "uptime",
    label: t("monitor.uptime"),
    value: uptimeLabel.value,
  },
  {
    id: "loadAverage",
    label: t("monitor.loadAverage"),
    value: loadAverageLabel.value,
  },
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
      color: "primary",
    },
    {
      id: "memory",
      icon: "i-lucide-memory-stick",
      label: t("monitor.memory"),
      value: formatPercent(memory.usagePercent),
      detail: formatMemoryPair(memory),
      percent: progressPercent(memory.usagePercent),
      color: "success",
    },
    {
      id: "swap",
      icon: "i-lucide-hard-drive",
      label: t("monitor.swap"),
      value: formatPercent(swap.usagePercent),
      detail: formatMemoryPair(swap),
      percent: progressPercent(swap.usagePercent),
      color: "warning",
    },
  ];
});
const processRows = computed(() =>
  sortProcessRows(systemInfo.value?.processes ?? [])
    .slice(0, PROCESS_ROW_LIMIT)
    .map((process, index) => ({
      id: `${process.pid}-${index}`,
      cpuDisplay: formatPercent(process.cpuPercent),
      memoryDisplay: formatProcessMemory(process),
      command: process.command || "--",
    })),
);
const processColumns = computed(() => [
  { accessorKey: "cpuDisplay", header: t("monitor.cpu") },
  { accessorKey: "memoryDisplay", header: t("monitor.memory") },
  { accessorKey: "command", header: t("monitor.command") },
]);
const diskRows = computed(() =>
  (systemInfo.value?.disks ?? []).slice(0, DISK_ROW_LIMIT).map((disk) => ({
    id: disk.path,
    path: disk.path,
    availableDisplay: formatDiskAvailable(disk),
  })),
);
const diskColumns = computed(() => [
  { accessorKey: "path", header: t("monitor.path") },
  { accessorKey: "availableDisplay", header: t("monitor.available") },
]);
const primaryNetwork = computed(() => systemInfo.value?.networks?.[0] ?? null);
const latestNetworkSample = computed(() => activeSession.value?.networkHistory?.at(-1) ?? null);
const networkStats = computed(() => {
  const latestSample = latestNetworkSample.value;

  return [
    {
      id: "rx",
      icon: "i-lucide-arrow-down",
      label: t("monitor.rx"),
      value: formatBytesPerSecond(latestSample?.rxBytesPerSecond),
      total: formatBytes(primaryNetwork.value?.rxBytes),
    },
    {
      id: "tx",
      icon: "i-lucide-arrow-up",
      label: t("monitor.tx"),
      value: formatBytesPerSecond(latestSample?.txBytesPerSecond),
      total: formatBytes(primaryNetwork.value?.txBytes),
    },
  ];
});
const networkChart = computed(() => buildNetworkChart(activeSession.value?.networkHistory ?? []));
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

function getStatusColor(status) {
  if (status === "connected") return "success";
  if (status === "connecting") return "warning";
  if (status === "error") return "error";

  return "neutral";
}

function progressPercent(value) {
  return Math.min(Math.max(Number(value) || 0, 0), 100);
}

function sortProcessRows(processes) {
  return [...processes].sort((left, right) => processScore(right) - processScore(left));
}

function processScore(process) {
  return Math.max(Number(process.cpuPercent) || 0, Number(process.memoryPercent) || 0);
}

function formatPercent(value) {
  if (value === null || value === undefined) return "--";

  return `${Number(value).toFixed(Number(value) % 1 === 0 ? 0 : 1)}%`;
}

function formatProcessMemory(process) {
  if (process.memoryBytes) return formatBytes(process.memoryBytes);
  if (process.memoryPercent || process.memoryPercent === 0) return formatPercent(process.memoryPercent);

  return "--";
}

function formatMemoryPair(metric = {}) {
  if (!metric.totalBytes) return "--";

  return `${formatBytes(metric.usedBytes)} / ${formatBytes(metric.totalBytes)}`;
}

function formatDiskAvailable(disk = {}) {
  if (!disk.totalBytes) return "--";

  return `${formatBytes(disk.availableBytes)} / ${formatBytes(disk.totalBytes)}`;
}

function formatBytes(value) {
  if (value === null || value === undefined) return "--";

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = Math.max(Number(value) || 0, 0);
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function formatBytesPerSecond(value) {
  if (value === null || value === undefined) return "--";

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

function buildNetworkChart(history) {
  const recentSamples = history.slice(-NETWORK_CHART_SAMPLE_LIMIT);
  const emptySampleCount = Math.max(NETWORK_CHART_SAMPLE_LIMIT - recentSamples.length, 0);
  const samples = [
    ...Array.from({ length: emptySampleCount }, () => null),
    ...recentSamples,
  ].map(normalizeNetworkSample);
  const maxRate = Math.max(
    ...samples.flatMap((sample) => [sample.rxBytesPerSecond, sample.txBytesPerSecond]),
    1,
  );
  const rxPoints = getChartPoints(samples, "rxBytesPerSecond", maxRate);
  const txPoints = getChartPoints(samples, "txBytesPerSecond", maxRate);
  const baseline = NETWORK_CHART_HEIGHT - NETWORK_CHART_PADDING;

  return {
    hasData: samples.some((sample) => sample.rxBytesPerSecond > 0 || sample.txBytesPerSecond > 0),
    rxPath: pointsToPath(rxPoints),
    txPath: pointsToPath(txPoints),
    rxAreaPath: pointsToAreaPath(rxPoints, baseline),
    txAreaPath: pointsToAreaPath(txPoints, baseline),
    maxLabel: formatBytesPerSecond(maxRate),
  };
}

function normalizeNetworkSample(sample) {
  return {
    rxBytesPerSecond: Math.max(Number(sample?.rxBytesPerSecond ?? sample?.bytesPerSecond ?? 0), 0),
    txBytesPerSecond: Math.max(Number(sample?.txBytesPerSecond ?? 0), 0),
  };
}

function getChartPoints(samples, key, maxRate) {
  const innerWidth = NETWORK_CHART_WIDTH - NETWORK_CHART_PADDING * 2;
  const innerHeight = NETWORK_CHART_HEIGHT - NETWORK_CHART_PADDING * 2;
  const step = innerWidth / Math.max(samples.length - 1, 1);

  return samples.map((sample, index) => ({
    x: NETWORK_CHART_PADDING + index * step,
    y: NETWORK_CHART_PADDING + innerHeight - (sample[key] / maxRate) * innerHeight,
  }));
}

function pointsToPath(points) {
  return points
    .map((point, index) => `${index === 0 ? "M" : "L"} ${roundPathValue(point.x)} ${roundPathValue(point.y)}`)
    .join(" ");
}

function pointsToAreaPath(points, baseline) {
  if (!points.length) return "";

  const firstPoint = points[0];
  const lastPoint = points.at(-1);

  return `${pointsToPath(points)} L ${roundPathValue(lastPoint.x)} ${roundPathValue(baseline)} L ${roundPathValue(firstPoint.x)} ${roundPathValue(baseline)} Z`;
}

function roundPathValue(value) {
  return Number(value).toFixed(2);
}
</script>

<template>
  <aside class="host-monitor-sidebar" :aria-label="t('monitor.systemInfo')">
    <div class="monitor-scroll">
      <UCard class="monitor-card monitor-hero-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content hero-content">
          <div class="sync-row">
            <span>{{ t("monitor.syncStatus") }}</span>
            <UBadge
              :label="syncBadgeLabel"
              :color="isConnected ? 'success' : statusColor"
              variant="soft"
              size="sm"
            />
          </div>

          <div class="monitor-address">
            <small>{{ t("monitor.ipAddress") }}</small>
            <strong>{{ activeSession?.host ?? "--" }}</strong>
          </div>

          <UButton
            icon="i-lucide-monitor"
            trailing-icon="i-lucide-chevron-right"
            color="neutral"
            variant="soft"
            size="sm"
            block
            class="system-info-action"
            :label="t('monitor.systemInfo')"
            :disabled="!isConnected || activeSession?.systemInfoLoading"
            :loading="activeSession?.systemInfoLoading"
            @click="refreshCurrentSystemInfo"
          />
        </div>
      </UCard>

      <UCard class="monitor-card monitor-summary-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <dl class="monitor-summary">
            <div v-for="row in summaryRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd>
                <UBadge
                  v-if="row.id === 'state'"
                  :label="row.value"
                  :color="statusColor"
                  variant="soft"
                  size="sm"
                />
                <span v-else>{{ row.value }}</span>
              </dd>
            </div>
          </dl>

          <div class="metric-list">
            <div v-for="metric in metricRows" :key="metric.id" class="metric-row" :class="metric.id">
              <div class="metric-label">
                <span>
                  <UIcon :name="metric.icon" />
                  {{ metric.label }}
                </span>
                <strong>{{ metric.value }}</strong>
              </div>
              <small>{{ metric.detail }}</small>
              <UProgress
                :model-value="metric.percent"
                :color="metric.color"
                size="sm"
                :ui="progressUi"
              />
            </div>
          </div>

          <UAlert
            v-if="activeSession?.systemInfoError"
            class="monitor-error"
            icon="i-lucide-circle-alert"
            color="error"
            variant="soft"
            :title="activeSession.systemInfoError"
          />
        </div>
      </UCard>

      <UCard class="monitor-card monitor-process-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <div class="monitor-card-title">
            <span>{{ t("monitor.processes") }}</span>
            <UBadge
              :label="t('monitor.topProcesses', { count: PROCESS_ROW_LIMIT })"
              color="neutral"
              variant="soft"
              size="sm"
            />
          </div>

          <UTable
            :data="processRows"
            :columns="processColumns"
            :empty="t('monitor.noData')"
            :ui="tableUi"
            class="process-table"
          >
            <template #command-cell="{ row }">
              <span class="command-cell" :title="row.original.command">
                {{ row.original.command }}
              </span>
            </template>
          </UTable>
        </div>
      </UCard>

      <UCard class="monitor-card monitor-network-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <div class="monitor-card-title">
            <span>{{ t("monitor.network") }}</span>
            <UBadge
              :label="primaryNetwork?.name ?? '--'"
              color="neutral"
              variant="soft"
              size="sm"
            />
          </div>

          <div class="network-stat-grid">
            <div v-for="stat in networkStats" :key="stat.id" class="network-stat" :class="stat.id">
              <span>
                <UIcon :name="stat.icon" />
                {{ stat.label }}
              </span>
              <strong>{{ stat.value }}</strong>
              <small>{{ t("monitor.total") }} {{ stat.total }}</small>
            </div>
          </div>

          <div class="network-chart-shell">
            <svg
              class="network-line-chart"
              :viewBox="`0 0 ${NETWORK_CHART_WIDTH} ${NETWORK_CHART_HEIGHT}`"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <path class="chart-grid" d="M 8 22 H 212 M 8 43 H 212 M 8 64 H 212" />
              <path class="chart-area rx" :d="networkChart.rxAreaPath" />
              <path class="chart-area tx" :d="networkChart.txAreaPath" />
              <path class="chart-line rx" :d="networkChart.rxPath" />
              <path class="chart-line tx" :d="networkChart.txPath" />
            </svg>
            <span class="chart-max-label">{{ networkChart.maxLabel }}</span>
            <span v-if="!networkChart.hasData" class="chart-empty">{{ t("monitor.noData") }}</span>
          </div>
        </div>
      </UCard>

      <UCard class="monitor-card monitor-disk-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <div class="monitor-card-title">
            <span>{{ t("monitor.storage") }}</span>
          </div>

          <UTable
            :data="diskRows"
            :columns="diskColumns"
            :empty="t('monitor.noData')"
            :ui="tableUi"
            class="disk-table"
          />
        </div>
      </UCard>
    </div>
  </aside>
</template>

<style scoped src="../../assets/css/host-monitor.css"></style>
