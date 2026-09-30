<script setup>
import { computed } from "vue";
import { clampPercent, formatPercent } from "../../utils/formatMetrics";

const props = defineProps({
  label: { type: String, required: true },
  // 标题旁的补充（如磁盘挂载点「/」）
  hint: { type: String, default: "" },
  percent: { type: Number, default: null },
  caption: { type: String, default: "" },
  // rose = 内存，violet = 磁盘；同一栏里两种颜色，扫一眼就能分开
  tone: { type: String, default: "rose" },
});

// 超过 80% 转琥珀、90% 转红：占用高才需要注意，平时保持品牌色不喧宾夺主
const level = computed(() => {
  const value = Number(props.percent);
  if (value >= 90) return "critical";
  if (value >= 80) return "high";
  return "normal";
});
</script>

<template>
  <section class="monitor-meter" :class="[tone, level]">
    <header>
      <span class="monitor-meter-label">
        {{ label }}
        <em v-if="hint">{{ hint }}</em>
      </span>
      <strong>{{ formatPercent(percent) }}</strong>
    </header>
    <UProgress
      :model-value="percent === null ? 0 : clampPercent(percent)"
      size="sm"
      :ui="{ root: 'monitor-meter-root', base: 'monitor-meter-track', indicator: 'monitor-meter-fill' }"
    />
    <small v-if="caption">{{ caption }}</small>
  </section>
</template>

<style scoped src="../../assets/css/monitor-meter.css"></style>
