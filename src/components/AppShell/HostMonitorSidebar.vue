<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "../../stores/sessionStore";

const { t } = useI18n();
const sessionStore = useSessionStore();

const activeSession = computed(() => sessionStore.activeSession);
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

const metricRows = computed(() => [
  { id: "cpu", label: t("monitor.cpu"), value: "--", percent: 0 },
  { id: "memory", label: t("monitor.memory"), value: "--", percent: 0 },
  { id: "swap", label: t("monitor.swap"), value: "--", percent: 0 },
]);
const processRows = computed(() => [
  { id: "ssh", memory: "--", cpu: "--", command: "ssh" },
  { id: "sftp", memory: "--", cpu: "--", command: "sftp" },
  { id: "shell", memory: "--", cpu: "--", command: "shell" },
]);
const diskRows = computed(() => [
  { id: "root", path: "/", size: "--" },
  { id: "home", path: "/home", size: "--" },
  { id: "var", path: "/var", size: "--" },
]);
const networkBars = Array.from({ length: 28 }, (_, index) => ({
  id: index,
  height: 8 + ((index * 17) % 36),
}));
</script>

<template>
  <aside class="host-monitor-sidebar" aria-label="Host monitor">
    <section class="monitor-block compact">
      <div class="sync-row">
        <span>{{ t("monitor.syncStatus") }}</span>
        <span class="monitor-dot" :class="{ active: activeSession }"></span>
      </div>
      <div class="monitor-ip">
        <small>{{ t("monitor.host") }}</small>
        <strong>{{ activeSession?.host ?? "--" }}</strong>
      </div>
      <button type="button" class="monitor-info-button">
        {{ t("monitor.systemInfo") }}
      </button>
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
      </dl>

      <div class="metric-list">
        <div v-for="metric in metricRows" :key="metric.id" class="metric-row">
          <div class="metric-label">
            <span>{{ metric.label }}</span>
            <strong>{{ metric.value }}</strong>
          </div>
          <span class="metric-track">
            <span :style="{ width: `${metric.percent}%` }"></span>
          </span>
        </div>
      </div>
    </section>

    <section class="monitor-block grow">
      <div class="mini-table process-table">
        <div class="mini-heading">
          <span>{{ t("monitor.memory") }}</span>
          <span>{{ t("monitor.cpu") }}</span>
          <span>{{ t("monitor.command") }}</span>
        </div>
        <div v-for="process in processRows" :key="process.id" class="mini-row">
          <span>{{ process.memory }}</span>
          <span>{{ process.cpu }}</span>
          <span>{{ process.command }}</span>
        </div>
      </div>

      <div class="network-panel">
        <div class="network-head">
          <span>{{ t("monitor.network") }}</span>
          <strong>eth0</strong>
        </div>
        <div class="network-chart" aria-hidden="true">
          <span
            v-for="bar in networkBars"
            :key="bar.id"
            :style="{ height: `${bar.height}px` }"
          ></span>
        </div>
      </div>
    </section>

    <section class="monitor-block disk-block">
      <div class="mini-table disk-table">
        <div class="mini-heading">
          <span>{{ t("monitor.path") }}</span>
          <span>{{ t("monitor.available") }}</span>
        </div>
        <div v-for="disk in diskRows" :key="disk.id" class="mini-row">
          <span>{{ disk.path }}</span>
          <span>{{ disk.size }}</span>
        </div>
      </div>
    </section>
  </aside>
</template>
