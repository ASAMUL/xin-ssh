<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  formatBytes,
  formatDateTime,
  formatPercent,
  formatUptime,
} from "../../utils/formatMetrics";

const props = defineProps({
  session: { type: Object, default: null },
});
const open = defineModel("open", { type: Boolean, default: false });
const { t } = useI18n();

const systemInfo = computed(() => props.session?.systemInfo ?? null);

const basicRows = computed(() => [
  { id: "os", label: t("monitor.os"), value: systemInfo.value?.osName ?? "--" },
  {
    id: "endpoint",
    label: t("monitor.endpoint"),
    value: props.session ? `${props.session.username}@${props.session.host}` : "--",
  },
  { id: "hostname", label: t("monitor.hostname"), value: systemInfo.value?.hostname ?? "--" },
  { id: "kernel", label: t("monitor.kernel"), value: systemInfo.value?.kernel ?? "--" },
  { id: "uptime", label: t("monitor.uptime"), value: formatUptime(systemInfo.value?.uptimeSeconds, t) },
  {
    id: "loadAverage",
    label: t("monitor.loadAverage"),
    value: systemInfo.value?.loadAverage?.length
      ? systemInfo.value.loadAverage.map((value) => value.toFixed(2)).join(" / ")
      : "--",
  },
  { id: "updatedAt", label: t("monitor.updatedAt"), value: formatDateTime(props.session?.systemInfoUpdatedAt) },
]);

const cpuRows = computed(() => {
  const cpu = systemInfo.value?.cpu ?? {};
  return [
    { id: "usage", label: t("monitor.cpuUsage"), value: formatPercent(cpu.usagePercent) },
    { id: "cores", label: t("monitor.cpuCores"), value: cpu.coreCount ? t("monitor.coreCount", { count: cpu.coreCount }) : "--" },
    { id: "model", label: t("monitor.cpuModel"), value: cpu.modelName ?? "--" },
  ];
});

const memoryRows = computed(() => [
  ...memoryMetricRows("memory", t("monitor.memory"), systemInfo.value?.memory),
  ...memoryMetricRows("swap", t("monitor.swap"), systemInfo.value?.swap),
]);

const networkBlocks = computed(() => (systemInfo.value?.networks ?? []).map((network) => ({
  id: network.name,
  title: network.name,
  rows: [
    { label: t("monitor.ipv4Address"), value: network.ipv4Address ?? "--" },
    { label: t("monitor.rxTotal"), value: formatBytes(network.rxBytes) },
    { label: t("monitor.txTotal"), value: formatBytes(network.txBytes) },
  ],
})));

const diskBlocks = computed(() => (systemInfo.value?.disks ?? []).map((disk) => ({
  id: disk.path,
  title: disk.path,
  rows: [
    { label: t("monitor.filesystem"), value: disk.filesystem || "--" },
    { label: t("monitor.used"), value: formatPercent(disk.usagePercent) },
    {
      label: t("monitor.available"),
      value: disk.totalBytes ? `${formatBytes(disk.availableBytes)} / ${formatBytes(disk.totalBytes)}` : "--",
    },
  ],
})));

const processRows = computed(() => [...(systemInfo.value?.processes ?? [])]
  .sort((left, right) => Number(right.cpuPercent) - Number(left.cpuPercent))
  .map((process, index) => ({
    id: `${process.pid}-${index}`,
    command: process.command || "--",
    meta: `${t("monitor.pid")} ${process.pid} · ${process.user || "--"}`,
    cpu: formatPercent(process.cpuPercent),
    memory: process.memoryBytes ? formatBytes(process.memoryBytes) : formatPercent(process.memoryPercent),
  })));

// null = 远端没有 systemd，和「一个都没跑」分开提示
const services = computed(() => systemInfo.value?.services ?? null);

function memoryMetricRows(prefix, label, metric = {}) {
  return [
    { id: `${prefix}-usage`, label: `${label} ${t("monitor.used")}`, value: formatPercent(metric?.usagePercent) },
    { id: `${prefix}-total`, label: `${label} ${t("monitor.total")}`, value: formatBytes(metric?.totalBytes) },
    { id: `${prefix}-available`, label: `${label} ${t("monitor.available")}`, value: formatBytes(metric?.availableBytes) },
  ];
}
</script>

<template>
  <USlideover
    v-model:open="open"
    :title="t('monitor.systemInfoDetailTitle')"
    :description="session?.host ?? undefined"
    :ui="{ content: 'liquid-sheet system-info-drawer', body: 'system-info-drawer-body' }"
  >
    <template #body>
      <div class="system-info-detail">
        <section class="detail-section">
          <h3>{{ t("monitor.basicInfo") }}</h3>
          <dl class="detail-list">
            <div v-for="row in basicRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd :title="row.value">{{ row.value }}</dd>
            </div>
          </dl>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.cpu") }}</h3>
          <dl class="detail-list">
            <div v-for="row in cpuRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd :title="row.value">{{ row.value }}</dd>
            </div>
          </dl>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.memory") }}</h3>
          <dl class="detail-list">
            <div v-for="row in memoryRows" :key="row.id">
              <dt>{{ row.label }}</dt>
              <dd>{{ row.value }}</dd>
            </div>
          </dl>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.network") }}</h3>
          <div v-if="networkBlocks.length" class="detail-stack">
            <article v-for="block in networkBlocks" :key="block.id" class="detail-mini-block">
              <strong>{{ block.title }}</strong>
              <dl class="detail-list">
                <div v-for="row in block.rows" :key="row.label">
                  <dt>{{ row.label }}</dt>
                  <dd>{{ row.value }}</dd>
                </div>
              </dl>
            </article>
          </div>
          <p v-else class="detail-empty">{{ t("monitor.noData") }}</p>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.storage") }}</h3>
          <div v-if="diskBlocks.length" class="detail-stack">
            <article v-for="block in diskBlocks" :key="block.id" class="detail-mini-block">
              <strong>{{ block.title }}</strong>
              <dl class="detail-list">
                <div v-for="row in block.rows" :key="row.label">
                  <dt>{{ row.label }}</dt>
                  <dd>{{ row.value }}</dd>
                </div>
              </dl>
            </article>
          </div>
          <p v-else class="detail-empty">{{ t("monitor.noData") }}</p>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.processTab") }}</h3>
          <div v-if="processRows.length" class="detail-row-list">
            <article v-for="process in processRows" :key="process.id" class="detail-process-row">
              <div>
                <strong :title="process.command">{{ process.command }}</strong>
                <small>{{ process.meta }}</small>
              </div>
              <span>{{ process.cpu }}</span>
              <span>{{ process.memory }}</span>
            </article>
          </div>
          <p v-else class="detail-empty">{{ t("monitor.noData") }}</p>
        </section>

        <section class="detail-section">
          <h3>{{ t("monitor.serviceTab") }}</h3>
          <p v-if="services === null" class="detail-empty">{{ t("monitor.servicesUnsupported") }}</p>
          <div v-else-if="services.length" class="detail-row-list">
            <article v-for="service in services" :key="service.name" class="detail-service-row">
              <i></i>
              <div>
                <strong :title="service.name">{{ service.name }}</strong>
                <small :title="service.description">{{ service.description || service.subState }}</small>
              </div>
            </article>
          </div>
          <p v-else class="detail-empty">{{ t("monitor.noServices") }}</p>
        </section>
      </div>
    </template>
  </USlideover>
</template>

<style scoped src="../../assets/css/system-info-drawer.css"></style>
