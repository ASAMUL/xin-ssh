<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { areaY, defineChart, dot, lineY, ruleY } from "@tanstack/charts";
import { crosshair } from "@tanstack/charts/crosshair";
import { decorative } from "@tanstack/charts/mark/decorative";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { scaleOrdinal } from "@tanstack/charts/scales/ordinal";
import { scalePoint } from "@tanstack/charts/scales/point";
import { chartCurves } from "@nuxt/ui/utils/chart";
import { formatClock } from "../../utils/formatMetrics";

const props = defineProps({
  // metricHistory 的采样行（按 capturedAt 先后排好）
  samples: { type: Array, required: true },
  // [{ key, label, tone: "rose" | "violet" }]；第一条画渐隐面积，其余只画线
  categories: { type: Array, required: true },
  // 纵轴上限；不传就按数据自适应（网络速率没有固定上限）
  max: { type: Number, default: null },
  height: { type: Number, default: 44 },
  // 悬停提示里读数的格式
  format: { type: Function, default: (value) => String(value) },
});

const TONE_COLORS = { rose: "var(--spark-rose)", violet: "var(--spark-violet)" };
const AREA_GRADIENT = "spark-area";

const { locale } = useI18n();

const colors = computed(() => props.categories.map(({ tone }) => TONE_COLORS[tone] ?? TONE_COLORS.rose));
const ariaLabel = computed(() => props.categories.map(({ label }) => label).join(" / "));

const series = computed(() => props.categories.map(({ key }) => props.samples
  .filter((sample) => Number.isFinite(sample[key]))
  .map((sample) => ({ x: sample.capturedAt, y: sample[key], category: key }))));

// 上限自己算、不交给 scaleLinear 推断：CPU 要钉死 0–100，否则 3% 的读数也会顶满整张图；
// 网络留 20% 头部空间，全 0 时退成 1，免得值域塌成一个点
const ceiling = computed(() => {
  if (props.max) return props.max;
  const peak = Math.max(0, ...series.value.flat().map(({ y }) => y));
  return peak > 0 ? peak * 1.2 : 1;
});

const definition = computed(() => {
  const keys = props.categories.map(({ key }) => key);
  const latest = series.value.map((rows) => rows.at(-1)).filter(Boolean);
  const curve = chartCurves.monotone;

  return defineChart({
    marks: [
      ruleY([0], { stroke: "var(--spark-guide)", strokeDasharray: "2 3" }),
      ...series.value.flatMap((rows, index) => [
        ...(index === 0
          ? [decorative(areaY(rows, {
            x: "x",
            y1: 0,
            y2: "y",
            z: "category",
            color: "category",
            curve,
            fill: `url(#${AREA_GRADIENT})`,
            fillOpacity: 1,
          }))]
          : []),
        lineY(rows, { x: "x", y: "y", z: "category", color: "category", curve, strokeWidth: index === 0 ? 1.8 : 1.4 }),
      ]),
      // 末端标出「现在」：外圈半透明光晕 + 实心点，描边取卡片底色，把点从线上抠出来
      decorative(dot(latest, { x: "x", y: "y", color: "category", r: 6, fillOpacity: 0.18, strokeWidth: 0 })),
      decorative(dot(latest, {
        x: "x",
        y: "y",
        color: "category",
        r: 3.5,
        stroke: "var(--xin-canvas-raised)",
        strokeWidth: 1.5,
      })),
      crosshair({ x: { stroke: "var(--spark-guide-strong)", strokeDasharray: "2 3" }, y: false }),
    ],
    guides: false,
    // 上、右留出光晕半径：顶格的读数和最新那颗点，否则会被 SVG 边界切掉一半
    margin: { top: 6, right: 6, bottom: 1, left: 1 },
    scales: {
      x: { scale: () => scalePoint().padding(0) },
      y: { scale: scaleLinear().domain([0, ceiling.value]) },
    },
    color: {
      scale: scaleOrdinal().domain(keys).range(keys.map((_, index) => `var(--ts-chart-${index + 1})`)),
    },
    gradients: [{
      id: AREA_GRADIENT,
      x1: 0,
      y1: 0,
      x2: 0,
      y2: 1,
      stops: [
        { offset: 0, color: "var(--ts-chart-1)", opacity: 0.3 },
        { offset: 1, color: "var(--ts-chart-1)", opacity: 0 },
      ],
    }],
    focus: "group-x",
    // 刚连上只有两三个采样时点距很大，默认 48px 的捕捉范围会在点之间留出悬停死区
    maxFocusDistance: Number.POSITIVE_INFINITY,
    // 每轮采样整条线左移一格，开动画才是「流过去」，而不是一帧一帧地跳
    svgAnimation: { duration: 400, easing: "ease-out", respectReducedMotion: true },
  });
});

// 按 categories 的顺序排：发送在上、接收在下，和上方读数的顺序一致
function tooltipRows(points) {
  return props.categories
    .map((category) => ({ category, point: points.find((point) => point.datum?.category === category.key) }))
    .filter(({ point }) => point);
}
</script>

<template>
  <UChart
    class="metric-sparkline"
    :definition="definition"
    :colors="colors"
    :height="height"
    :aria-label="ariaLabel"
    :ui="{ tooltip: 'spark-tooltip' }"
  >
    <template #tooltip="{ points }">
      <p class="spark-tooltip-time">{{ formatClock(points[0]?.datum?.x, locale) }}</p>
      <div v-for="{ category, point } in tooltipRows(points)" :key="category.key" class="spark-tooltip-row">
        <i :style="{ backgroundColor: point.color }"></i>
        <span>{{ category.label }}</span>
        <strong>{{ format(point.datum.y) }}</strong>
      </div>
    </template>
  </UChart>
</template>

<style scoped src="../../assets/css/metric-sparkline.css"></style>
