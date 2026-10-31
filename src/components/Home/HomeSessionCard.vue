<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import OsIconTile from "../ConnectionManager/OsIconTile.vue";
import MetricSparkline from "../Monitor/MetricSparkline.vue";
import { resolveProfileOsIcon } from "../../config/osIcons";
import {
  clampPercent,
  formatGibibytes,
  formatPercent,
  formatUptimeShort,
  latencyLevel,
  usageLevel,
} from "../../utils/formatMetrics";

const props = defineProps({
  session: { type: Object, required: true },
  profile: { type: Object, required: true },
  menuItems: { type: Array, default: () => [] },
});
const emit = defineEmits(["resume"]);
const { t } = useI18n();

const osIcon = computed(() => resolveProfileOsIcon(props.profile));
const address = computed(() => {
  const target = `${props.session.username}@${props.session.host}`;
  return Number(props.session.port) === 22 ? target : `${target}:${props.session.port}`;
});
const statusLabel = computed(() => t(`monitor.status.${props.session.status}`));

// 读数由 HomeDashboard 接着采样（工作区里攒下的走势也一并带过来）；只有连着的时候才有意义
const isConnected = computed(() => props.session.status === "connected");
const history = computed(() => props.session.metricHistory ?? []);
const systemInfo = computed(() => props.session.systemInfo ?? null);

const cpuPercent = computed(() => systemInfo.value?.cpu?.usagePercent ?? null);
const cpuCategories = computed(() => [{ key: "cpuPercent", label: t("monitor.cpu"), tone: "rose" }]);

// 内存不画走势：它一连几小时都是一条平线，看的是「还剩多少」，所以用容量条 + 已用 / 总量
const memory = computed(() => systemInfo.value?.memory ?? null);
const memoryPercent = computed(() => memory.value?.usagePercent ?? null);
const memoryCaption = computed(() => (memory.value?.totalBytes
  ? `${formatGibibytes(memory.value.usedBytes)} / ${formatGibibytes(memory.value.totalBytes)} GiB`
  : ""));

const latency = computed(() => (Number.isFinite(props.session.latencyMs) ? props.session.latencyMs : null));
const uptimeLabel = computed(() => (systemInfo.value?.uptimeSeconds
  ? t("monitor.uptimeShort", { value: formatUptimeShort(systemInfo.value.uptimeSeconds, t) })
  : ""));

// 卡上的大号数字取整：小数位每轮都在跳，只会勾眼角，精确值在走势图的悬停提示里
function roundedPercent(value) {
  return Number.isFinite(value) ? Math.round(value) : null;
}
</script>

<template>
  <article class="home-session-card liquid-rim" :class="session.status">
    <div class="home-session-texture" aria-hidden="true"></div>

    <OsIconTile :icon="osIcon" :size="60" tone="ink" />

    <div class="home-session-copy">
      <strong>
        <i class="home-session-status" :title="statusLabel"></i>
        <span>{{ session.name }}</span>
      </strong>
      <span class="home-session-address">{{ address }}</span>
      <span class="home-session-path">
        <UIcon name="i-lucide-folder" />
        <span>{{ t("home.currentDirectory") }}</span>
        <code>{{ session.remotePath || "/" }}</code>
      </span>
    </div>

    <dl v-if="isConnected" class="home-session-vitals">
      <div class="home-vital" :class="usageLevel(cpuPercent)">
        <dt>{{ t("monitor.cpu") }}</dt>
        <dd class="home-vital-value">
          <template v-if="roundedPercent(cpuPercent) !== null">{{ roundedPercent(cpuPercent) }}<small>%</small></template>
          <template v-else>--</template>
        </dd>
        <dd class="home-vital-foot home-vital-chart">
          <MetricSparkline
            :samples="history"
            :categories="cpuCategories"
            :max="100"
            :height="26"
            :format="formatPercent"
          />
        </dd>
      </div>
      <div class="home-vital" :class="usageLevel(memoryPercent)">
        <dt>{{ t("monitor.memory") }}</dt>
        <dd class="home-vital-value">
          <template v-if="roundedPercent(memoryPercent) !== null">{{ roundedPercent(memoryPercent) }}<small>%</small></template>
          <template v-else>--</template>
        </dd>
        <dd class="home-vital-foot home-vital-meter">
          <UProgress
            :model-value="clampPercent(memoryPercent)"
            :ui="{ base: 'home-vital-track', indicator: 'home-vital-fill' }"
          />
          <span class="home-vital-caption">{{ memoryCaption }}</span>
        </dd>
      </div>
      <div class="home-vital" :class="latencyLevel(latency)">
        <dt>{{ t("statusBar.latency") }}</dt>
        <dd class="home-vital-value">
          <template v-if="latency !== null">{{ latency }}<small>ms</small></template>
          <template v-else>--</template>
        </dd>
        <dd class="home-vital-foot home-vital-caption">{{ uptimeLabel }}</dd>
      </div>
    </dl>

    <div class="home-session-actions">
      <UButton
        class="home-session-resume"
        size="lg"
        trailing-icon="i-lucide-arrow-right"
        :label="t('home.resumeSession')"
        @click="emit('resume', session.id)"
      />
      <UDropdownMenu v-if="menuItems.length" :items="menuItems" size="sm" :content="{ align: 'end' }">
        <UButton
          class="home-session-more"
          color="neutral"
          variant="ghost"
          size="lg"
          square
          icon="i-lucide-ellipsis"
          :aria-label="t('home.moreActions')"
          :title="t('home.moreActions')"
        />
      </UDropdownMenu>
    </div>
  </article>
</template>

<style scoped src="../../assets/css/home-session-card.css"></style>
