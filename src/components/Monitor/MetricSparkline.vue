<script setup>
import { computed, useId } from "vue";

const props = defineProps({
  // [{ values: number[], tone: "rose" | "violet" }]；第一条画面积，其余只画线
  series: { type: Array, required: true },
  // 纵轴上限；不传就按数据自适应（网络速率没有固定上限）
  max: { type: Number, default: null },
  height: { type: Number, default: 44 },
});

// viewBox 横向固定 100，preserveAspectRatio="none" 让它随栏宽拉伸；线宽用 non-scaling-stroke 保持不变
const WIDTH = 100;
const PADDING_TOP = 4;
const gradientId = `spark-${useId()}`;

const ceiling = computed(() => {
  if (props.max) return props.max;
  const peak = Math.max(0, ...props.series.flatMap(({ values }) => values.filter(Number.isFinite)));
  return peak > 0 ? peak * 1.2 : 1;
});

const paths = computed(() => props.series.map(({ values, tone }, index) => {
  const points = toPoints(values);
  const line = smoothPath(points);
  const area = index === 0 && points.length > 1
    ? `${line} L ${points.at(-1).x} ${props.height} L ${points[0].x} ${props.height} Z`
    : "";
  return { tone, line, area, last: points.at(-1) ?? null };
}));

function toPoints(values) {
  const finite = values.map((value) => (Number.isFinite(value) ? value : null));
  const step = WIDTH / Math.max(finite.length - 1, 1);
  const usable = props.height - PADDING_TOP;
  return finite
    .map((value, index) => (value === null ? null : {
      x: Number((index * step).toFixed(2)),
      y: Number((props.height - Math.min(value / ceiling.value, 1) * usable).toFixed(2)),
    }))
    .filter(Boolean);
}

// Catmull-Rom 转三次贝塞尔：折线在 5 秒一个点的密度下太「机械」，平滑后更像读数在流动
function smoothPath(points) {
  if (!points.length) return "";
  if (points.length === 1) return `M 0 ${points[0].y} L ${WIDTH} ${points[0].y}`;

  let path = `M ${points[0].x} ${points[0].y}`;
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[index - 1] ?? points[index];
    const current = points[index];
    const next = points[index + 1];
    const after = points[index + 2] ?? next;
    const control1 = { x: current.x + (next.x - previous.x) / 6, y: current.y + (next.y - previous.y) / 6 };
    const control2 = { x: next.x - (after.x - current.x) / 6, y: next.y - (after.y - current.y) / 6 };
    path += ` C ${control1.x.toFixed(2)} ${control1.y.toFixed(2)}, ${control2.x.toFixed(2)} ${control2.y.toFixed(2)}, ${next.x} ${next.y}`;
  }
  return path;
}
</script>

<template>
  <div class="metric-sparkline" :style="{ height: `${height}px` }" aria-hidden="true">
    <svg :viewBox="`0 0 ${WIDTH} ${height}`" preserveAspectRatio="none">
      <defs>
        <linearGradient :id="gradientId" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" class="stop-top" :class="paths[0]?.tone" />
          <stop offset="100%" class="stop-bottom" :class="paths[0]?.tone" />
        </linearGradient>
      </defs>
      <line class="baseline" x1="0" :y1="height - 0.5" :x2="WIDTH" :y2="height - 0.5" />
      <template v-for="(path, index) in paths" :key="index">
        <path v-if="path.area" class="area" :d="path.area" :fill="`url(#${gradientId})`" />
        <path v-if="path.line" class="line" :class="path.tone" :d="path.line" />
      </template>
    </svg>
    <!-- 末端的发光点用 HTML 画：SVG 被横向拉伸后圆会变成椭圆 -->
    <span
      v-for="(path, index) in paths.filter((item) => item.last)"
      :key="`dot-${index}`"
      class="last-dot"
      :class="path.tone"
      :style="{ left: `${path.last.x}%`, top: `${path.last.y}px` }"
    ></span>
  </div>
</template>

<style scoped src="../../assets/css/metric-sparkline.css"></style>
