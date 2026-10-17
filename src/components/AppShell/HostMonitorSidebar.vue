<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import MetricSparkline from "../Monitor/MetricSparkline.vue";
import MonitorMeter from "../Monitor/MonitorMeter.vue";
import SystemInfoDrawer from "../Monitor/SystemInfoDrawer.vue";
import OsIconTile from "../ConnectionManager/OsIconTile.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSystemInfoPolling } from "../../composables/useSystemInfoPolling";
import { resolveProfileOsIcon } from "../../config/osIcons";
import {
  formatBytesPerSecond,
  formatGibibytes,
  formatPercent,
  formatUptimeShort,
} from "../../utils/formatMetrics";

const PROCESS_ROW_LIMIT = 3;
const SERVICE_ROW_LIMIT = 4;
const REFRESH_OPTIONS = [2, 5, 10, 30];

const { t } = useI18n();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const { refresh: refreshSystemInfo } = useSystemInfoPolling(sessionStore, settingsStore);

const isDrawerOpen = ref(false);
const activeTab = ref("processes");

const session = computed(() => sessionStore.activeSession);
const systemInfo = computed(() => session.value?.systemInfo ?? null);
const history = computed(() => session.value?.metricHistory ?? []);
const isConnected = computed(() => session.value?.status === "connected");
const profile = computed(() => profileStore.profiles.find((item) => item.id === session.value?.profileId) ?? null);
const osIcon = computed(() => resolveProfileOsIcon(profile.value));

const osTitle = computed(() => systemInfo.value?.osName || systemInfo.value?.hostname || session.value?.host || "--");
const osSpecs = computed(() => {
  const cores = systemInfo.value?.cpu?.coreCount;
  const memory = systemInfo.value?.memory?.totalBytes;
  const parts = [
    cores ? t("monitor.vcpu", { count: cores }) : null,
    memory ? `${Math.round(memory / 1024 ** 3) || formatGibibytes(memory)} GiB` : null,
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : "--";
});
const uptimeLabel = computed(() => (systemInfo.value?.uptimeSeconds
  ? t("monitor.uptimeShort", { value: formatUptimeShort(systemInfo.value.uptimeSeconds, t) })
  : ""));

const cpuPercent = computed(() => systemInfo.value?.cpu?.usagePercent ?? null);
const cpuCategories = computed(() => [{ key: "cpuPercent", label: t("monitor.cpu"), tone: "rose" }]);
const cpuCaption = computed(() => {
  const load = systemInfo.value?.loadAverage?.[0];
  const cores = systemInfo.value?.cpu?.coreCount;
  if (load === undefined) return "";
  return t("monitor.loadCaption", { load: load.toFixed(2), cores: cores ?? "--" });
});
const cpuLevel = computed(() => (cpuPercent.value >= 90 ? "critical" : cpuPercent.value >= 80 ? "high" : "normal"));

const memory = computed(() => systemInfo.value?.memory ?? null);
const memoryCaption = computed(() => (memory.value?.totalBytes
  ? `${formatGibibytes(memory.value.usedBytes)} / ${formatGibibytes(memory.value.totalBytes)} GiB`
  : ""));
// 已用 + 缓存 + 空闲正好等于总量（后端按 available 拆的），分段条能严丝合缝铺满；
// 拿不到缓存（/proc/meminfo 缺 MemFree）就退回单段 + 文字说明
const memorySegments = computed(() => {
  const { totalBytes, usedBytes, cacheBytes } = memory.value ?? {};
  if (!totalBytes || usedBytes == null || cacheBytes == null) return null;
  return [
    { key: "used", label: t("monitor.memoryUsed"), value: usedBytes },
    { key: "cache", label: t("monitor.memoryCache"), value: cacheBytes },
    { key: "free", label: t("monitor.memoryFree"), value: Math.max(totalBytes - usedBytes - cacheBytes, 0) },
  ].map((segment) => ({ ...segment, display: `${formatGibibytes(segment.value)} GiB` }));
});

// 概览只看根分区：它满了整台机器都会出问题，其余挂载点放在详情抽屉里
const rootDisk = computed(() => {
  const disks = systemInfo.value?.disks ?? [];
  return disks.find((disk) => disk.path === "/") ?? disks[0] ?? null;
});
const diskCaption = computed(() => (rootDisk.value?.totalBytes
  ? t("monitor.diskCaption", {
    available: `${formatGibibytes(rootDisk.value.availableBytes)} GiB`,
    total: `${formatGibibytes(rootDisk.value.totalBytes)} GiB`,
  })
  : ""));

const primaryNetwork = computed(() => systemInfo.value?.networks?.[0] ?? null);
const latestSample = computed(() => history.value.at(-1) ?? null);
const networkCategories = computed(() => [
  { key: "txBytesPerSecond", label: t("monitor.send"), tone: "rose" },
  { key: "rxBytesPerSecond", label: t("monitor.receive"), tone: "violet" },
]);

const tabItems = computed(() => [
  { value: "processes", label: t("monitor.processTab") },
  { value: "services", label: t("monitor.serviceTab") },
]);

// 条形长度按本组最大值归一：CPU 占用普遍个位数，按 100% 画就都是一条线
const processRows = computed(() => {
  const rows = [...(systemInfo.value?.processes ?? [])]
    .sort((left, right) => Number(right.cpuPercent) - Number(left.cpuPercent))
    .slice(0, PROCESS_ROW_LIMIT);
  const peak = Math.max(1, ...rows.map((process) => Number(process.cpuPercent) || 0));
  return rows.map((process, index) => ({
    id: `${process.pid}-${index}`,
    name: processName(process.command),
    command: process.command,
    value: formatPercent(process.cpuPercent),
    ratio: (Number(process.cpuPercent) || 0) / peak,
  }));
});

const services = computed(() => systemInfo.value?.services);
const serviceRows = computed(() => (services.value ?? []).slice(0, SERVICE_ROW_LIMIT));

const menuItems = computed(() => [
  [
    {
      label: t("monitor.systemDetails"),
      icon: "i-lucide-panel-right-open",
      disabled: !isConnected.value,
      onSelect: openDrawer,
    },
    {
      label: t("monitor.refreshNow"),
      icon: "i-lucide-refresh-cw",
      disabled: !isConnected.value,
      onSelect: () => refreshSystemInfo(),
    },
    {
      label: t("monitor.refreshEvery"),
      icon: "i-lucide-timer",
      children: REFRESH_OPTIONS.map((seconds) => ({
        label: t("monitor.refreshOption", { seconds }),
        icon: settingsStore.monitorRefreshSeconds === seconds ? "i-lucide-check" : undefined,
        onSelect: () => settingsStore.setMonitorRefreshSeconds(seconds),
      })),
    },
  ],
  [
    {
      label: t("monitor.collapseSidebar"),
      icon: "i-lucide-panel-left-close",
      onSelect: () => settingsStore.toggleMonitorSidebar(),
    },
  ],
]);

// 首页只隐藏工作区，传送到 body 的详情抽屉要随会话目标变化主动收起
watch(() => [session.value?.id, session.value?.currentAttemptId], () => {
  isDrawerOpen.value = false;
});

function openDrawer() {
  isDrawerOpen.value = true;
  if (!systemInfo.value) refreshSystemInfo();
}

// ps 的 args 带完整路径和参数；概览只要一眼认出是谁，取可执行文件名
function processName(command = "") {
  const executable = command.trim().split(/\s+/)[0] ?? "";
  return executable.split("/").pop() || command || "--";
}
</script>

<template>
  <aside class="host-monitor-sidebar liquid-rim" :aria-label="t('monitor.hostOverview')">
    <header class="monitor-header">
      <h2>{{ t("monitor.hostOverview") }}</h2>
      <UDropdownMenu :items="menuItems" size="sm" :content="{ align: 'end' }">
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          square
          icon="i-lucide-ellipsis"
          :aria-label="t('home.moreActions')"
          :title="t('home.moreActions')"
        />
      </UDropdownMenu>
    </header>

    <div class="monitor-scroll">
      <section class="monitor-host">
        <OsIconTile :icon="osIcon" :size="46" />
        <div class="monitor-host-copy">
          <strong :title="osTitle">{{ osTitle }}</strong>
          <span>{{ osSpecs }}</span>
          <small v-if="uptimeLabel">{{ uptimeLabel }}</small>
        </div>
      </section>

      <p v-if="!isConnected" class="monitor-waiting">
        <UIcon name="i-lucide-plug-zap" />
        <span>{{ t("monitor.waitingConnection") }}</span>
      </p>

      <template v-else-if="systemInfo">
        <UAlert
          v-if="session?.systemInfoError"
          class="monitor-error"
          icon="i-lucide-circle-alert"
          color="error"
          variant="soft"
          :title="session.systemInfoError"
        />

        <section class="monitor-cpu" :class="cpuLevel">
          <header>
            <span>{{ t("monitor.cpu") }}</span>
            <strong>{{ formatPercent(cpuPercent) }}</strong>
          </header>
          <MetricSparkline
            :samples="history"
            :categories="cpuCategories"
            :max="100"
            :height="46"
            :format="formatPercent"
          />
          <small v-if="cpuCaption">{{ cpuCaption }}</small>
        </section>

        <MonitorMeter
          :label="t('monitor.memory')"
          :percent="memory?.usagePercent ?? null"
          :segments="memorySegments"
          :max="memory?.totalBytes"
          :caption="memorySegments ? '' : memoryCaption"
          tone="rose"
        />

        <MonitorMeter
          v-if="rootDisk"
          :label="t('monitor.disk')"
          :hint="rootDisk.path"
          :percent="rootDisk.usagePercent ?? null"
          :caption="diskCaption"
          tone="violet"
        />

        <section class="monitor-network">
          <header>
            <span>{{ t("monitor.network") }}</span>
            <em v-if="primaryNetwork">{{ primaryNetwork.name }}</em>
          </header>
          <div class="monitor-network-rates">
            <span class="send">
              <UIcon name="i-lucide-arrow-up" />
              {{ t("monitor.send") }}
            </span>
            <strong>{{ formatBytesPerSecond(latestSample?.txBytesPerSecond) }}</strong>
            <span class="receive">
              <UIcon name="i-lucide-arrow-down" />
              {{ t("monitor.receive") }}
            </span>
            <strong>{{ formatBytesPerSecond(latestSample?.rxBytesPerSecond) }}</strong>
          </div>
          <MetricSparkline
            :samples="history"
            :categories="networkCategories"
            :height="30"
            :format="formatBytesPerSecond"
          />
        </section>

        <section class="monitor-activity">
          <UTabs
            v-model="activeTab"
            v-liquid-stretch="activeTab"
            class="monitor-tabs"
            variant="pill"
            size="xs"
            :items="tabItems"
            :content="false"
            :ui="{ list: 'monitor-tabs-list', indicator: 'monitor-tabs-indicator', trigger: 'monitor-tabs-trigger' }"
          />

          <template v-if="activeTab === 'processes'">
            <small class="monitor-activity-caption">{{ t("monitor.topCpu", { count: PROCESS_ROW_LIMIT }) }}</small>
            <ul v-if="processRows.length" class="monitor-rows">
              <li v-for="process in processRows" :key="process.id" :title="process.command">
                <span class="monitor-row-name">{{ process.name }}</span>
                <span class="monitor-row-bar"><i :style="{ transform: `scaleX(${process.ratio})` }"></i></span>
                <strong>{{ process.value }}</strong>
              </li>
            </ul>
            <p v-else class="monitor-empty">{{ t("monitor.noData") }}</p>
          </template>

          <template v-else>
            <p v-if="services === null" class="monitor-empty">{{ t("monitor.servicesUnsupported") }}</p>
            <template v-else>
              <small class="monitor-activity-caption">
                {{ t("monitor.runningServices", { count: services?.length ?? 0 }) }}
              </small>
              <ul v-if="serviceRows.length" class="monitor-rows services">
                <li v-for="service in serviceRows" :key="service.name" :title="service.description">
                  <i class="monitor-service-dot"></i>
                  <span class="monitor-row-name">{{ service.name }}</span>
                </li>
              </ul>
              <p v-else class="monitor-empty">{{ t("monitor.noServices") }}</p>
            </template>
          </template>

          <UButton
            class="monitor-view-all"
            color="neutral"
            variant="link"
            size="sm"
            trailing-icon="i-lucide-chevron-right"
            :label="t('monitor.viewAll')"
            @click="openDrawer"
          />
        </section>
      </template>

      <div v-else class="monitor-loading">
        <USkeleton v-for="index in 4" :key="index" class="monitor-skeleton" />
      </div>
    </div>
  </aside>

  <SystemInfoDrawer v-model:open="isDrawerOpen" :session="session" />
</template>

<style scoped src="../../assets/css/host-monitor.css"></style>
