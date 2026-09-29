<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSystemInfoPolling } from "../../composables/useSystemInfoPolling";

const PROCESS_ROW_LIMIT = 5;
const SPARKLINE_WIDTH = 72;
const SPARKLINE_HEIGHT = 24;
const SPARKLINE_PADDING = 3;

const { t } = useI18n();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const { refresh: refreshCurrentSystemInfo } = useSystemInfoPolling(sessionStore, settingsStore);

const cardUi = {
  // liquid-rim：监控卡是内容层，只借液态玻璃的高光描边，不做折射
  root: "monitor-card-root liquid-rim",
  body: "p-0 sm:p-0 monitor-card-body",
};
const slideoverUi = {
  content: "system-info-detail-panel",
  header: "system-info-detail-header",
  body: "system-info-detail-body",
};
const progressUi = {
  root: "monitor-progress-root",
  base: "monitor-progress-base",
  indicator: "monitor-progress-indicator",
};

const activeSession = computed(() => sessionStore.activeSession);
const isSystemInfoDetailsOpen = ref(false);
const systemInfo = computed(() => activeSession.value?.systemInfo ?? null);
const isConnected = computed(() => activeSession.value?.status === "connected");
const statusLabel = computed(() => {
  const session = activeSession.value;
  if (!session) return t("monitor.noSession");

  return t(`monitor.status.${session.status}`);
});
const statusColor = computed(() => getStatusColor(activeSession.value?.status));
const connectionBadgeLabel = computed(() =>
  isConnected.value ? t("monitor.status.connected") : statusLabel.value,
);
const hostIdentity = computed(() => {
  const session = activeSession.value;
  if (!session) return systemInfo.value?.hostname ?? "--";

  return `${session.username}@${session.host}`;
});
const overviewStatusLabel = computed(() => (isConnected.value ? t("monitor.online") : statusLabel.value));
const overviewRows = computed(() => [
  {
    id: "hostname",
    label: t("monitor.hostname"),
    value: hostIdentity.value,
  },
  {
    id: "state",
    label: t("monitor.state"),
    value: overviewStatusLabel.value,
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
const resourceRows = computed(() => {
  const cpu = systemInfo.value?.cpu ?? {};
  const memory = systemInfo.value?.memory ?? {};
  const swap = systemInfo.value?.swap ?? {};

  return [
    {
      id: "cpu",
      icon: "i-lucide-cpu",
      label: t("monitor.cpuUsage"),
      value: formatPercent(cpu.usagePercent),
      percent: progressPercent(cpu.usagePercent),
      detail: null,
      sparklinePath: "",
    },
    {
      id: "memory",
      icon: "i-lucide-memory-stick",
      label: t("monitor.memoryUsage"),
      value: formatPercent(memory.usagePercent),
      percent: progressPercent(memory.usagePercent),
      detail: formatMemoryPair(memory),
      sparklinePath: buildResourceSparkline(memory.usagePercent),
    },
    {
      id: "swap",
      icon: "i-lucide-hard-drive",
      label: t("monitor.swapUsage"),
      value: formatPercent(swap.usagePercent),
      percent: progressPercent(swap.usagePercent),
      detail: formatMemoryPair(swap),
      sparklinePath: "",
    },
  ];
});
const processRows = computed(() =>
  sortProcessRows(systemInfo.value?.processes ?? [])
    .slice(0, PROCESS_ROW_LIMIT)
    .map((process, index) => ({
      id: `${process.pid}-${index}`,
      command: process.command || "--",
      cpuDisplay: formatPercent(process.cpuPercent),
      dotTone: processScore(process) > 8 ? "warning" : "online",
    })),
);
const primaryNetwork = computed(() => systemInfo.value?.networks?.[0] ?? null);
const latestNetworkSample = computed(() => activeSession.value?.networkHistory?.at(-1) ?? null);
const networkTitle = computed(() =>
  t("monitor.networkWithName", { name: primaryNetwork.value?.name ?? "--" }),
);
const networkRows = computed(() => [
  {
    id: "ipv4",
    label: t("monitor.ipv4Address"),
    value: primaryNetwork.value?.ipv4Address ?? "--",
    icon: null,
  },
  {
    id: "upload",
    label: t("monitor.uploadRate"),
    value: formatBytesPerSecond(latestNetworkSample.value?.txBytesPerSecond),
    icon: "i-lucide-arrow-up",
  },
  {
    id: "download",
    label: t("monitor.downloadRate"),
    value: formatBytesPerSecond(latestNetworkSample.value?.rxBytesPerSecond),
    icon: "i-lucide-arrow-down",
  },
]);
const basicDetailRows = computed(() => [
  {
    id: "endpoint",
    label: t("monitor.endpoint"),
    value: hostIdentity.value,
  },
  {
    id: "hostname",
    label: t("monitor.hostname"),
    value: systemInfo.value?.hostname ?? "--",
  },
  {
    id: "kernel",
    label: t("monitor.kernel"),
    value: systemInfo.value?.kernel ?? "--",
  },
  {
    id: "state",
    label: t("monitor.state"),
    value: overviewStatusLabel.value,
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
const cpuDetailRows = computed(() => {
  const cpu = systemInfo.value?.cpu ?? {};

  return [
    {
      id: "usage",
      label: t("monitor.cpuUsage"),
      value: formatPercent(cpu.usagePercent),
    },
    {
      id: "cores",
      label: t("monitor.cpuCores"),
      value: cpu.coreCount ? t("monitor.coreCount", { count: cpu.coreCount }) : "--",
    },
    {
      id: "model",
      label: t("monitor.cpuModel"),
      value: cpu.modelName ?? "--",
    },
  ];
});
const memoryDetailRows = computed(() => [
  ...buildMemoryDetailRows("memory", t("monitor.memory"), systemInfo.value?.memory),
  ...buildMemoryDetailRows("swap", t("monitor.swap"), systemInfo.value?.swap),
]);
const networkDetailRows = computed(() =>
  (systemInfo.value?.networks ?? []).map((network) => ({
    id: network.name,
    title: network.name,
    rows: [
      {
        label: t("monitor.ipv4Address"),
        value: network.ipv4Address ?? "--",
      },
      {
        label: t("monitor.rxTotal"),
        value: formatBytes(network.rxBytes),
      },
      {
        label: t("monitor.txTotal"),
        value: formatBytes(network.txBytes),
      },
    ],
  })),
);
const diskDetailRows = computed(() =>
  (systemInfo.value?.disks ?? []).map((disk) => ({
    id: disk.path,
    title: disk.path,
    rows: [
      {
        label: t("monitor.filesystem"),
        value: disk.filesystem || "--",
      },
      {
        label: t("monitor.used"),
        value: formatPercent(disk.usagePercent),
      },
      {
        label: t("monitor.available"),
        value: formatDiskAvailable(disk),
      },
    ],
  })),
);
const processDetailRows = computed(() =>
  sortProcessRows(systemInfo.value?.processes ?? []).map((process, index) => ({
    id: `${process.pid}-${index}`,
    command: process.command || "--",
    meta: `${t("monitor.pid")} ${process.pid} · ${process.user || "--"}`,
    cpuDisplay: formatPercent(process.cpuPercent),
    memoryDisplay: formatProcessMemory(process),
  })),
);
const lastUpdatedLabel = computed(() => formatDateTime(activeSession.value?.systemInfoUpdatedAt));
const uptimeLabel = computed(() => formatUptime(systemInfo.value?.uptimeSeconds));
const loadAverageLabel = computed(() => {
  const loadAverage = systemInfo.value?.loadAverage ?? [];
  if (!loadAverage.length) return "--";

  return loadAverage.map((value) => value.toFixed(2)).join(" / ");
});

// 首页只隐藏工作区，传送到 body 的详情弹窗需随会话目标变化主动收起。
watch(() => [activeSession.value?.id, activeSession.value?.currentAttemptId], () => {
  isSystemInfoDetailsOpen.value = false;
});

function openSystemInfoDetails() {
  isSystemInfoDetailsOpen.value = true;
  if (!systemInfo.value) {
    refreshCurrentSystemInfo();
  }
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

function formatMemoryPair(metric = {}) {
  if (!metric.totalBytes) return "--";

  return `${formatBytes(metric.usedBytes)} / ${formatBytes(metric.totalBytes)}`;
}

function formatDiskAvailable(disk = {}) {
  if (!disk.totalBytes) return "--";

  return `${formatBytes(disk.availableBytes)} / ${formatBytes(disk.totalBytes)}`;
}

function formatProcessMemory(process) {
  if (process.memoryBytes) return formatBytes(process.memoryBytes);
  if (process.memoryPercent || process.memoryPercent === 0) return formatPercent(process.memoryPercent);

  return "--";
}

function buildMemoryDetailRows(prefix, label, metric = {}) {
  return [
    {
      id: `${prefix}-usage`,
      label: `${label} ${t("monitor.used")}`,
      value: formatPercent(metric?.usagePercent),
    },
    {
      id: `${prefix}-total`,
      label: `${label} ${t("monitor.total")}`,
      value: formatBytes(metric?.totalBytes),
    },
    {
      id: `${prefix}-available`,
      label: `${label} ${t("monitor.available")}`,
      value: formatBytes(metric?.availableBytes),
    },
  ];
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

  let size = Math.max(Number(value) || 0, 0);
  if (size < 1024) return `${size.toFixed(0)} B/s`;

  const units = ["KB", "MB", "GB", "TB"];
  let unitIndex = 0;
  size /= 1024;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size < 10 ? 2 : size < 100 ? 1 : 0)} ${units[unitIndex]}/s`;
}

function formatDateTime(value) {
  if (!value) return "--";

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "--";

  return [
    `${date.getFullYear()}-${padDatePart(date.getMonth() + 1)}-${padDatePart(date.getDate())}`,
    `${padDatePart(date.getHours())}:${padDatePart(date.getMinutes())}:${padDatePart(date.getSeconds())}`,
  ].join(" ");
}

function padDatePart(value) {
  return String(value).padStart(2, "0");
}

function formatUptime(seconds) {
  if (!seconds) return "--";

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (days > 0) return t("monitor.uptimeDays", { days, hours, minutes });
  if (hours > 0) return t("monitor.uptimeHours", { hours, minutes });

  return t("monitor.uptimeMinutes", { minutes });
}

function buildResourceSparkline(percent) {
  if (percent === null || percent === undefined) return "";

  const base = progressPercent(percent) / 100;
  const values = [0.34, 0.5, 0.43, 0.6, 0.48, 0.37, 0.47].map((value) =>
    Math.min(Math.max(value + base * 0.12, 0.12), 0.88),
  );
  const innerWidth = SPARKLINE_WIDTH - SPARKLINE_PADDING * 2;
  const innerHeight = SPARKLINE_HEIGHT - SPARKLINE_PADDING * 2;
  const step = innerWidth / Math.max(values.length - 1, 1);

  return values
    .map((value, index) => {
      const x = SPARKLINE_PADDING + index * step;
      const y = SPARKLINE_PADDING + innerHeight - value * innerHeight;

      return `${index === 0 ? "M" : "L"} ${x.toFixed(2)} ${y.toFixed(2)}`;
    })
    .join(" ");
}
</script>

<template>
  <aside class="host-monitor-sidebar" :aria-label="t('monitor.systemInfo')">
    <div class="monitor-scroll">
      <UCard class="monitor-card monitor-hero-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content hero-content">
          <div class="connection-status-row">
            <span>{{ t("monitor.connectionStatus") }}</span>
            <UBadge
              class="connection-status-badge"
              :label="connectionBadgeLabel"
              :color="isConnected ? 'success' : statusColor"
              variant="soft"
              size="xs"
            />
          </div>

          <div class="monitor-address">
            <small>{{ t("monitor.ipAddress") }}</small>
            <strong>{{ activeSession?.host ?? "--" }}</strong>
          </div>

          <UButton
            icon="i-lucide-monitor"
            trailing-icon="i-lucide-chevron-right"
            color="primary"
            variant="solid"
            size="sm"
            block
            class="system-info-action"
            :label="t('monitor.systemInfo')"
            :disabled="!isConnected"
            @click="openSystemInfoDetails"
          />
        </div>
      </UCard>

      <UCard class="monitor-card monitor-overview-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <h2 class="monitor-card-heading">{{ t("monitor.hostOverview") }}</h2>

          <dl class="compact-info-list">
            <div v-for="row in overviewRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd>
                <span
                  v-if="row.id === 'state'"
                  class="overview-status"
                  :class="{ online: isConnected }"
                >
                  {{ row.value }}
                </span>
                <span v-else>{{ row.value }}</span>
              </dd>
            </div>
          </dl>

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

      <UCard class="monitor-card monitor-resource-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <h2 class="monitor-card-heading">{{ t("monitor.resourceMonitor") }}</h2>

          <div class="resource-list">
            <div v-for="metric in resourceRows" :key="metric.id" class="resource-row" :class="metric.id">
              <div class="resource-topline">
                <span class="resource-label">
                  <UIcon :name="metric.icon" class="resource-icon" />
                  {{ metric.label }}
                </span>
                <strong>{{ metric.value }}</strong>
              </div>

              <div class="resource-visual">
                <UProgress
                  :model-value="metric.percent"
                  color="primary"
                  size="sm"
                  :ui="progressUi"
                />
                <svg
                  v-if="metric.sparklinePath"
                  class="metric-sparkline"
                  :viewBox="`0 0 ${SPARKLINE_WIDTH} ${SPARKLINE_HEIGHT}`"
                  preserveAspectRatio="none"
                  aria-hidden="true"
                >
                  <path :d="metric.sparklinePath" />
                </svg>
              </div>

              <small v-if="metric.detail">{{ metric.detail }}</small>
            </div>
          </div>
        </div>
      </UCard>

      <UCard class="monitor-card monitor-process-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <div class="monitor-card-title">
            <h2 class="monitor-card-heading">{{ t("monitor.processes") }}</h2>
            <button type="button" class="view-all-button" @click="openSystemInfoDetails">
              {{ t("monitor.viewAll") }}
            </button>
          </div>

          <div v-if="processRows.length" class="process-list">
            <div v-for="process in processRows" :key="process.id" class="process-row">
              <span class="process-name" :title="process.command">{{ process.command }}</span>
              <strong>{{ process.cpuDisplay }}</strong>
              <span class="service-dot" :class="process.dotTone" />
            </div>
          </div>
          <p v-else class="monitor-empty">{{ t("monitor.noData") }}</p>
        </div>
      </UCard>

      <UCard class="monitor-card monitor-network-card" variant="soft" :ui="cardUi">
        <div class="monitor-card-content">
          <h2 class="monitor-card-heading">{{ networkTitle }}</h2>

          <dl class="compact-info-list network-info-list">
            <div v-for="row in networkRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd :class="row.id">
                <UIcon v-if="row.icon" :name="row.icon" class="network-rate-icon" />
                <span>{{ row.value }}</span>
              </dd>
            </div>
          </dl>
        </div>
      </UCard>
    </div>
  </aside>

  <USlideover
    v-model:open="isSystemInfoDetailsOpen"
    :title="t('monitor.systemInfoDetailTitle')"
    :description="activeSession?.host ?? undefined"
    :ui="slideoverUi"
  >
    <template #body>
      <div class="system-info-detail">
        <section class="detail-section">
          <h3>{{ t("monitor.basicInfo") }}</h3>
          <dl class="detail-list">
            <div v-for="row in basicDetailRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd>{{ row.value }}</dd>
            </div>
          </dl>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.cpu") }}</h3>
          <dl class="detail-list">
            <div v-for="row in cpuDetailRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd>{{ row.value }}</dd>
            </div>
          </dl>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.memory") }}</h3>
          <dl class="detail-list">
            <div v-for="row in memoryDetailRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd>{{ row.value }}</dd>
            </div>
          </dl>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.network") }}</h3>
          <div v-if="networkDetailRows.length" class="detail-stack">
            <article v-for="network in networkDetailRows" :key="network.id" class="detail-mini-block">
              <strong>{{ network.title }}</strong>
              <dl class="detail-list">
                <div v-for="row in network.rows" :key="row.label">
                  <dt>{{ row.label }}</dt>
                  <dd>{{ row.value }}</dd>
                </div>
              </dl>
            </article>
          </div>
          <p v-else class="monitor-empty">{{ t("monitor.noData") }}</p>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.storage") }}</h3>
          <div v-if="diskDetailRows.length" class="detail-stack">
            <article v-for="disk in diskDetailRows" :key="disk.id" class="detail-mini-block">
              <strong>{{ disk.title }}</strong>
              <dl class="detail-list">
                <div v-for="row in disk.rows" :key="row.label">
                  <dt>{{ row.label }}</dt>
                  <dd>{{ row.value }}</dd>
                </div>
              </dl>
            </article>
          </div>
          <p v-else class="monitor-empty">{{ t("monitor.noData") }}</p>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.processes") }}</h3>
          <div v-if="processDetailRows.length" class="detail-process-list">
            <article v-for="process in processDetailRows" :key="process.id" class="detail-process-row">
              <div>
                <strong :title="process.command">{{ process.command }}</strong>
                <small>{{ process.meta }}</small>
              </div>
              <span>{{ process.cpuDisplay }}</span>
              <span>{{ process.memoryDisplay }}</span>
            </article>
          </div>
          <p v-else class="monitor-empty">{{ t("monitor.noData") }}</p>
        </section>
      </div>
    </template>
  </USlideover>
</template>

<style scoped src="../../assets/css/host-monitor.css"></style>
