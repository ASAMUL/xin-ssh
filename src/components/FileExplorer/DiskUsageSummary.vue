<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { localizeError } from "../../utils/appError";
import { formatBytes } from "../../utils/formatMetrics";

const props = defineProps({
  analysis: {
    type: Object,
    required: true,
  },
});
const emit = defineEmits(["stop", "rerun", "close"]);
const { t } = useI18n();

const isRunning = computed(() => props.analysis.status === "running" || props.analysis.status === "canceling");
const statusIcon = computed(() => {
  if (isRunning.value) return "i-lucide-loader-circle";
  if (props.analysis.status === "failed") return "i-lucide-circle-alert";
  return "i-lucide-chart-pie";
});
const summary = computed(() => {
  const { status, scannedBytes, totalBytes, error } = props.analysis;
  if (status === "running") return t("diskUsage.running", { size: formatBytes(scannedBytes) });
  if (status === "canceling") return t("diskUsage.stopping");
  if (status === "canceled") return t("diskUsage.stopped", { size: formatBytes(scannedBytes) });
  if (status === "failed") return localizeError(error);
  return t("diskUsage.total", { size: formatBytes(totalBytes) });
});
</script>

<template>
  <div class="disk-usage-summary" :class="analysis.status" role="status">
    <UIcon :name="statusIcon" class="disk-usage-icon" :class="{ 'spin-icon': isRunning }" />
    <span class="disk-usage-text" :title="summary">{{ summary }}</span>
    <UTooltip v-if="analysis.unreadable" :text="t('diskUsage.unreadableHint')">
      <UBadge
        color="warning"
        variant="soft"
        size="sm"
        icon="i-lucide-lock"
        :label="t('diskUsage.unreadable', { count: analysis.unreadable })"
      />
    </UTooltip>

    <div class="disk-usage-actions">
      <UButton
        v-if="isRunning"
        color="neutral"
        variant="ghost"
        size="xs"
        icon="i-lucide-square"
        :label="t('diskUsage.stop')"
        :disabled="analysis.status === 'canceling'"
        @click="emit('stop')"
      />
      <UButton
        v-else
        color="neutral"
        variant="ghost"
        size="xs"
        icon="i-lucide-refresh-cw"
        :label="t('diskUsage.rerun')"
        @click="emit('rerun')"
      />
      <UTooltip :text="t('diskUsage.close')">
        <UButton
          color="neutral"
          variant="ghost"
          size="xs"
          square
          icon="i-lucide-x"
          :aria-label="t('diskUsage.close')"
          @click="emit('close')"
        />
      </UTooltip>
    </div>
  </div>
</template>

<style src="../../assets/css/disk-usage-summary.css"></style>
