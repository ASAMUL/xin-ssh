<script setup>
import { computed } from "vue";
import { clampPercent, formatPercent, usageLevel } from "../../utils/formatMetrics";

const props = defineProps({
  label: { type: String, required: true },
  // 标题旁的补充（如磁盘挂载点「/」）
  hint: { type: String, default: "" },
  percent: { type: Number, default: null },
  caption: { type: String, default: "" },
  // rose = 内存，violet = 磁盘；同一栏里两种颜色，扫一眼就能分开
  tone: { type: String, default: "rose" },
  // 分段明细 [{ key: "used" | "cache" | "free", label, value, display }]，value 与 max 同单位；
  // 不传就只画一段，长度取 percent
  segments: { type: Array, default: null },
  max: { type: Number, default: 100 },
});

// key 落到 class 上，由 CSS 决定每段的液柱材质；不传 color，免得 ProgressGroup 写内联背景色压掉渐变
const items = computed(() => (props.segments
  ? props.segments.map((segment) => ({
    ...segment,
    ui: { indicator: `monitor-meter-fill ${segment.key}`, itemLeadingDot: `monitor-meter-dot ${segment.key}` },
  }))
  : [{ value: props.percent === null ? 0 : clampPercent(props.percent), ui: { indicator: "monitor-meter-fill used" } }]));

const level = computed(() => usageLevel(props.percent));
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
    <UProgressGroup
      :items="items"
      :max="segments ? max : 100"
      :ui="{
        base: 'monitor-meter-track',
        segment: 'monitor-meter-segment',
        list: 'monitor-meter-legend',
        itemLabel: 'monitor-meter-legend-label',
        itemTrailing: 'monitor-meter-legend-value',
      }"
    >
      <!-- 插槽只在分段时给：ProgressGroup 一见到 item 插槽就渲染图例，单段的磁盘条用不着 -->
      <template v-if="segments" #item-trailing="{ item }">{{ item.display }}</template>
    </UProgressGroup>
    <small v-if="caption">{{ caption }}</small>
  </section>
</template>

<style scoped src="../../assets/css/monitor-meter.css"></style>
