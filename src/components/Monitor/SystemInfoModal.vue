<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import MetricSparkline from "./MetricSparkline.vue";
import OsIconTile from "../ConnectionManager/OsIconTile.vue";
import {
  clampPercent,
  formatBytes,
  formatBytesPerSecond,
  formatClock,
  formatGibibytes,
  formatPercent,
  formatUptime,
  processName,
  splitMemory,
  usageLevel,
} from "../../utils/formatMetrics";

// 和后端采样脚本 PS 段的 head -n 15 对应
const PROCESS_SAMPLE_LIMIT = 15;
// docker 给每个容器配一对 veth，跑几个容器就是一长串没有地址的网卡，列出来只是噪音
const CONTAINER_INTERFACE = /^veth/;
// 外壳和设置窗口同一种：高度固定，切换分类时窗口不跟着内容忽高忽矮；骨架样式见 nav-window.css
const MODAL_UI = {
  content: "max-w-[min(860px,94vw)] sm:max-w-[min(860px,94vw)] h-[min(620px,88vh)] overflow-hidden divide-y-0",
};
const BAR_UI = { base: "bar-track", segment: "bar-segment" };

const props = defineProps({
  session: { type: Object, default: null },
  // 概览栏按连接配置解析好的系统图标，窗口左上角和它保持一致
  osIcon: { type: Object, default: null },
});
const open = defineModel("open", { type: Boolean, default: false });
// overview / processes / services / storage / network：从概览栏哪个页签点「查看全部」，就直接落在哪一页
const tab = defineModel("tab", { type: String, default: "overview" });
const { t, locale } = useI18n();

const processSort = ref("cpu");
const processQuery = ref("");
const serviceQuery = ref("");

const systemInfo = computed(() => props.session?.systemInfo ?? null);
const history = computed(() => props.session?.metricHistory ?? []);
const endpoint = computed(() => (props.session ? `${props.session.username}@${props.session.host}` : undefined));
const hostname = computed(() => systemInfo.value?.hostname || props.session?.host || "--");
const updatedLabel = computed(() => t("monitor.updatedAtTime", {
  time: formatClock(props.session?.systemInfoUpdatedAt, locale.value),
}));

const sectionItems = computed(() => [
  { value: "overview", label: t("monitor.overviewTab"), icon: "i-lucide-gauge", slot: "overview" },
  { value: "processes", label: t("monitor.processTab"), icon: "i-lucide-cpu", slot: "processes" },
  { value: "services", label: t("monitor.serviceTab"), icon: "i-lucide-server-cog", slot: "services" },
  { value: "storage", label: t("monitor.storageTab"), icon: "i-lucide-hard-drive", slot: "storage" },
  { value: "network", label: t("monitor.network"), icon: "i-lucide-network", slot: "network" },
]);

// —— 概览 ——

const cpuPercent = computed(() => systemInfo.value?.cpu?.usagePercent ?? null);
const cpuCategories = computed(() => [{ key: "cpuPercent", label: t("monitor.cpu"), tone: "rose" }]);
const cpuCaption = computed(() => {
  const load = systemInfo.value?.loadAverage?.[0];
  if (load === undefined) return "";
  return t("monitor.loadCaption", { load: load.toFixed(2), cores: systemInfo.value?.cpu?.coreCount ?? "--" });
});

const memory = computed(() => systemInfo.value?.memory ?? null);
const memoryParts = computed(() => splitMemory(memory.value));
const memoryBar = computed(() => (memoryParts.value
  ? ["used", "cache", "free"].map((key) => ({ value: memoryParts.value[key], ui: { indicator: `bar-fill ${key}` } }))
  : [{ value: clampPercent(memory.value?.usagePercent), ui: { indicator: "bar-fill used" } }]));
const memoryCaption = computed(() => (memory.value?.totalBytes
  ? `${formatGibibytes(memory.value.usedBytes)} / ${formatGibibytes(memory.value.totalBytes)} GiB`
  : ""));

const disks = computed(() => (systemInfo.value?.disks ?? []).map((disk) => ({
  path: disk.path,
  filesystem: disk.filesystem || "--",
  percent: disk.usagePercent ?? null,
  level: usageLevel(disk.usagePercent),
  bar: [{ value: clampPercent(disk.usagePercent), ui: { indicator: "bar-fill used" } }],
  caption: disk.totalBytes
    ? t("monitor.diskCaption", { available: formatBytes(disk.availableBytes), total: formatBytes(disk.totalBytes) })
    : "",
})));
// 概览只放根分区：它满了整台机器都会出问题，其余挂载点在「存储」页
const rootDisk = computed(() => disks.value.find((disk) => disk.path === "/") ?? disks.value[0] ?? null);

const systemRows = computed(() => {
  const info = systemInfo.value ?? {};
  const cpu = info.cpu ?? {};
  const swap = info.swap ?? {};
  return [
    { id: "distribution", label: t("monitor.distribution"), value: info.osName },
    { id: "kernel", label: t("monitor.kernel"), value: info.kernel },
    { id: "hostname", label: t("monitor.hostname"), value: info.hostname },
    { id: "connection", label: t("monitor.connection"), value: endpoint.value },
    {
      id: "cpu",
      label: t("monitor.cpu"),
      value: [cpu.modelName, cpu.coreCount ? t("monitor.coreCount", { count: cpu.coreCount }) : null]
        .filter(Boolean)
        .join(" · "),
    },
    {
      id: "swap",
      label: t("monitor.swap"),
      value: swap.totalBytes ? `${formatBytes(swap.usedBytes)} / ${formatBytes(swap.totalBytes)}` : null,
    },
    { id: "uptime", label: t("monitor.uptime"), value: formatUptime(info.uptimeSeconds, t) },
    {
      id: "loadAverage",
      label: t("monitor.loadAverage"),
      value: info.loadAverage?.length ? info.loadAverage.map((value) => value.toFixed(2)).join(" / ") : null,
    },
  ].map((row) => ({ ...row, value: row.value || "--" }));
});

// —— 进程 ——

// 两种排法用分段控件切，不做可点的表头：这里只有两列数字，一眼就能看出哪列是百分比、哪列是字节
const processSortItems = computed(() => [
  { value: "cpu", label: t("monitor.cpu") },
  { value: "memory", label: t("monitor.memory") },
]);
const processes = computed(() => (systemInfo.value?.processes ?? []).map((process) => {
  const name = processName(process.command);
  // 内核线程的 args 就是 [名字]，再写一遍是重复；普通进程才把完整命令行带上
  const command = process.command?.startsWith("[") || process.command === name ? null : process.command;
  return {
    pid: process.pid,
    name,
    command,
    meta: [process.user, `${t("monitor.pid")} ${process.pid}`, command].filter(Boolean).join(" · "),
    cpu: Number(process.cpuPercent) || 0,
    memory: Number(process.memoryBytes) || 0,
    cpuLabel: formatPercent(process.cpuPercent),
    memoryLabel: process.memoryBytes ? formatBytes(process.memoryBytes) : formatPercent(process.memoryPercent),
  };
}));
const visibleProcesses = computed(() => {
  const key = processSort.value;
  return [...filterRows(processes.value, processQuery.value, (process) => [process.name, process.meta])]
    .sort((left, right) => right[key] - left[key]);
});

// —— 服务 ——

// null = 远端没有 systemd，和「一个都没跑」分开提示
const services = computed(() => systemInfo.value?.services ?? null);
const visibleServices = computed(() => filterRows(
  services.value ?? [],
  serviceQuery.value,
  (service) => [service.name, service.description],
));

// —— 网络 ——

const networks = computed(() => systemInfo.value?.networks ?? []);
const interfaces = computed(() => networks.value.filter((network) => !CONTAINER_INTERFACE.test(network.name)));
const hiddenInterfaceCount = computed(() => networks.value.length - interfaces.value.length);
const latestSample = computed(() => history.value.at(-1) ?? null);
const networkCategories = computed(() => [
  { key: "txBytesPerSecond", label: t("monitor.send"), tone: "rose" },
  { key: "rxBytesPerSecond", label: t("monitor.receive"), tone: "violet" },
]);
const networkFootnote = computed(() => [
  t("monitor.networkTotalsHint"),
  hiddenInterfaceCount.value ? t("monitor.hiddenVethInterfaces", { count: hiddenInterfaceCount.value }) : null,
].filter(Boolean).join(""));

function filterRows(rows, query, fields) {
  const keyword = query.trim().toLowerCase();
  if (!keyword) return rows;
  return rows.filter((row) => fields(row).some((field) => field?.toLowerCase().includes(keyword)));
}
</script>

<template>
  <UModal
    v-model:open="open"
    :title="t('monitor.systemInfoDetailTitle')"
    :description="endpoint"
    :ui="MODAL_UI"
  >
    <template #content>
      <UTabs
        v-model="tab"
        class="nav-window-tabs"
        orientation="vertical"
        variant="pill"
        :items="sectionItems"
        :ui="{
          root: 'items-stretch gap-0',
          list: 'nav-window-nav',
          trigger: 'nav-window-item',
          indicator: 'nav-window-indicator',
          content: 'nav-window-pane',
        }"
      >
        <template #list-leading>
          <div class="host-identity">
            <OsIconTile :icon="osIcon" :size="36" />
            <div>
              <strong :title="hostname">{{ hostname }}</strong>
              <small :title="systemInfo?.osName">{{ systemInfo?.osName || session?.host }}</small>
            </div>
          </div>
        </template>

        <template #list-trailing>
          <p class="nav-updated">{{ updatedLabel }}</p>
        </template>

        <template #overview>
          <h3 class="nav-window-pane-title">{{ t("monitor.overviewTab") }}</h3>

          <div class="overview-tiles">
            <article class="overview-tile rose" :class="usageLevel(cpuPercent)">
              <header><UIcon name="i-lucide-cpu" />{{ t("monitor.cpu") }}</header>
              <strong>{{ formatPercent(cpuPercent) }}</strong>
              <div class="tile-visual">
                <MetricSparkline
                  :samples="history"
                  :categories="cpuCategories"
                  :max="100"
                  :height="34"
                  :format="formatPercent"
                />
              </div>
              <small>{{ cpuCaption }}</small>
            </article>

            <article class="overview-tile rose" :class="usageLevel(memory?.usagePercent)">
              <header><UIcon name="i-lucide-memory-stick" />{{ t("monitor.memory") }}</header>
              <strong>{{ formatPercent(memory?.usagePercent ?? null) }}</strong>
              <div class="tile-visual">
                <UProgressGroup
                  class="usage-bar"
                  :items="memoryBar"
                  :max="memoryParts ? memory.totalBytes : 100"
                  :ui="BAR_UI"
                />
              </div>
              <small>{{ memoryCaption }}</small>
            </article>

            <article v-if="rootDisk" class="overview-tile violet" :class="rootDisk.level">
              <header>
                <UIcon name="i-lucide-hard-drive" />{{ t("monitor.disk") }}
                <em>{{ rootDisk.path }}</em>
              </header>
              <strong>{{ formatPercent(rootDisk.percent) }}</strong>
              <div class="tile-visual">
                <UProgressGroup class="usage-bar" :items="rootDisk.bar" :ui="BAR_UI" />
              </div>
              <small>{{ rootDisk.caption }}</small>
            </article>
          </div>

          <section class="form-section">
            <h4 class="form-section-title">{{ t("monitor.basicInfo") }}</h4>
            <div class="form-list">
              <div v-for="row in systemRows" :key="row.id" class="form-row form-row-split">
                <span class="form-row-label">{{ row.label }}</span>
                <span class="row-value" :title="row.value">{{ row.value }}</span>
              </div>
            </div>
          </section>
        </template>

        <template #processes>
          <h3 class="nav-window-pane-title">{{ t("monitor.processTab") }}</h3>

          <div class="pane-toolbar">
            <UTabs
              v-model="processSort"
              v-liquid-stretch="processSort"
              class="sort-switch"
              variant="pill"
              size="sm"
              :items="processSortItems"
              :content="false"
            />
            <UInput
              v-model="processQuery"
              class="pane-search"
              icon="i-lucide-search"
              size="sm"
              variant="soft"
              :placeholder="t('monitor.searchProcesses')"
            />
          </div>

          <section class="form-section">
            <div v-if="visibleProcesses.length" class="form-list metric-list">
              <div v-for="process in visibleProcesses" :key="process.pid" class="form-row metric-row">
                <div class="metric-row-main">
                  <strong>{{ process.name }}</strong>
                  <small :title="process.meta">{{ process.meta }}</small>
                </div>
                <span class="metric-value" :class="{ active: processSort === 'cpu' }">{{ process.cpuLabel }}</span>
                <span class="metric-value" :class="{ active: processSort === 'memory' }">{{ process.memoryLabel }}</span>
              </div>
            </div>
            <UEmpty
              v-else
              size="sm"
              variant="naked"
              :icon="processQuery ? 'i-lucide-search-x' : 'i-lucide-activity'"
              :description="processQuery ? t('monitor.noMatches') : t('monitor.noData')"
            />
            <p class="form-section-footer">{{ t("monitor.processFootnote", { limit: PROCESS_SAMPLE_LIMIT }) }}</p>
          </section>
        </template>

        <template #services>
          <h3 class="nav-window-pane-title">{{ t("monitor.serviceTab") }}</h3>

          <UEmpty
            v-if="services === null"
            size="sm"
            variant="naked"
            icon="i-lucide-server-off"
            :description="t('monitor.servicesUnsupported')"
          />
          <template v-else>
            <div class="pane-toolbar">
              <span class="pane-count">{{ t("monitor.runningServices", { count: services.length }) }}</span>
              <UInput
                v-model="serviceQuery"
                class="pane-search"
                icon="i-lucide-search"
                size="sm"
                variant="soft"
                :placeholder="t('monitor.searchServices')"
              />
            </div>

            <section class="form-section">
              <div v-if="visibleServices.length" class="form-list">
                <div v-for="service in visibleServices" :key="service.name" class="form-row form-row-split">
                  <span class="service-name">
                    <UChip standalone color="success" size="sm" :ui="{ base: 'service-dot' }" />
                    {{ service.name }}
                  </span>
                  <span class="row-value" :title="service.description">
                    {{ service.description || service.subState }}
                  </span>
                </div>
              </div>
              <UEmpty
                v-else
                size="sm"
                variant="naked"
                :icon="serviceQuery ? 'i-lucide-search-x' : 'i-lucide-circle-pause'"
                :description="serviceQuery ? t('monitor.noMatches') : t('monitor.noServices')"
              />
            </section>
          </template>
        </template>

        <template #storage>
          <h3 class="nav-window-pane-title">{{ t("monitor.storageTab") }}</h3>

          <section class="form-section">
            <h4 class="form-section-title">{{ t("monitor.mountPoints") }}</h4>
            <div v-if="disks.length" class="form-list">
              <div
                v-for="disk in disks"
                :key="disk.path"
                class="form-row form-row-block disk-row violet"
                :class="disk.level"
              >
                <div class="disk-row-line">
                  <strong>{{ disk.path }}</strong>
                  <span>{{ formatPercent(disk.percent) }}</span>
                </div>
                <UProgressGroup class="usage-bar" :items="disk.bar" :ui="BAR_UI" />
                <div class="disk-row-line muted">
                  <small>{{ disk.filesystem }}</small>
                  <small>{{ disk.caption }}</small>
                </div>
              </div>
            </div>
            <UEmpty v-else size="sm" variant="naked" icon="i-lucide-hard-drive" :description="t('monitor.noData')" />
          </section>
        </template>

        <template #network>
          <h3 class="nav-window-pane-title">{{ t("monitor.network") }}</h3>

          <section class="form-section">
            <h4 class="form-section-title">{{ t("monitor.liveRate") }}</h4>
            <div class="form-list">
              <div class="form-row form-row-block rate-row">
                <div class="rate-values">
                  <span class="send">
                    <UIcon name="i-lucide-arrow-up" />{{ t("monitor.send") }}
                    <strong>{{ formatBytesPerSecond(latestSample?.txBytesPerSecond) }}</strong>
                  </span>
                  <span class="receive">
                    <UIcon name="i-lucide-arrow-down" />{{ t("monitor.receive") }}
                    <strong>{{ formatBytesPerSecond(latestSample?.rxBytesPerSecond) }}</strong>
                  </span>
                </div>
                <MetricSparkline
                  :samples="history"
                  :categories="networkCategories"
                  :height="56"
                  :format="formatBytesPerSecond"
                />
              </div>
            </div>
          </section>

          <section class="form-section">
            <h4 class="form-section-title">{{ t("monitor.interfaces") }}</h4>
            <div v-if="interfaces.length" class="form-list metric-list">
              <div v-for="network in interfaces" :key="network.name" class="form-row metric-row">
                <div class="metric-row-main">
                  <strong>{{ network.name }}</strong>
                  <small>{{ network.ipv4Address ?? "--" }}</small>
                </div>
                <span class="metric-value receive">
                  <UIcon name="i-lucide-arrow-down" />{{ formatBytes(network.rxBytes) }}
                </span>
                <span class="metric-value send">
                  <UIcon name="i-lucide-arrow-up" />{{ formatBytes(network.txBytes) }}
                </span>
              </div>
            </div>
            <UEmpty v-else size="sm" variant="naked" icon="i-lucide-network" :description="t('monitor.noData')" />
            <p class="form-section-footer">{{ networkFootnote }}</p>
          </section>
        </template>
      </UTabs>

      <UButton
        class="nav-window-close"
        color="neutral"
        variant="ghost"
        square
        icon="i-lucide-x"
        :aria-label="t('settings.close')"
        @click="open = false"
      />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/system-info-modal.css"></style>
