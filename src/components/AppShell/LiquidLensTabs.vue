<script setup>
import { nextTick, ref, useTemplateRef, watch } from "vue";
import { useResizeObserver } from "@vueuse/core";
import { stretchIndicator } from "../../directives/liquidGlass";
import { measureBox, prefersReducedMotion, useLiquidLens } from "../../composables/useLiquidLens";

// 分段控件 + 按住浮起的放大透镜：iOS 26 标签栏里按住 / 拖动选中项时冒出来的那颗玻璃。
// 按下时选中的水滴原地鼓成透镜、跟着手指走，松手滑进手指所在的选项再落回水滴（动作见 useLiquidLens）。
// 键盘和焦点仍全交给 UTabs；鼠标点选改成松手才选，不然 Reka 按下就把水滴挪走，和透镜各走各的。
const props = defineProps({
  items: { type: Array, required: true },
  // 透传给 UTabs 的 ui（list / indicator / trigger 的类名）
  ui: { type: Object, default: () => ({}) },
  // 胶囊本身的玻璃参数（v-liquid-glass 的值）
  glass: { type: Object, default: () => ({}) },
  // 胶囊用透明玻璃（.liquid-glass--clear，首页工具栏）还是带底色的常规玻璃
  clear: { type: Boolean, default: false },
  size: { type: String, default: undefined },
  ariaLabel: { type: String, default: undefined },
});
const model = defineModel({ type: String, required: true });

// 透镜比最宽的选项再宽一圈、上下各冒出胶囊一截（胶囊的上下沿会被折进透镜里）
const LENS_PAD_X = 12;
const LENS_POP_Y = 9;
const LENS_GLASS = { magnify: 1.28, blur: 0, dispersion: 0.08 };

const root = useTemplateRef("root");
const tabs = useTemplateRef("tabs");
const lensElement = useTemplateRef("lens");
const tintElement = useTemplateRef("tint");
const lensSize = ref({ width: 0, height: 0 });
let boxes = [];
let containerWidth = 0;
let rootLeft = 0;
let pointerX = 0;

const { lensing, press, move, release } = useLiquidLens({
  container: root,
  lens: lensElement,
  tint: tintElement,
  lensSize: () => lensSize.value,
  // 拖到头时允许透镜冒出胶囊两端一点，像被拽住
  clampX: (x) => {
    const half = lensSize.value.width / 2;
    return Math.min(Math.max(x, half - LENS_PAD_X), containerWidth - half + LENS_PAD_X);
  },
});

function triggers() {
  return [...(tabs.value?.$el?.querySelectorAll('[role="tab"]') ?? [])];
}

function measure() {
  const element = root.value;
  if (!element) return;
  const rootRect = element.getBoundingClientRect();
  const triggerElements = triggers();
  boxes = triggerElements.map((trigger, index) => ({
    ...measureBox(trigger, rootRect),
    value: props.items[index]?.value,
    disabled: trigger.disabled,
  }));
  containerWidth = rootRect.width;
  const widest = Math.max(0, ...boxes.map(({ width }) => width));
  lensSize.value = {
    width: Math.round(widest + LENS_PAD_X * 2),
    height: Math.round(rootRect.height + LENS_POP_Y * 2),
  };
  renderTint(triggerElements);
}

// 透镜底下那层选中色直接克隆选项：图标、角标、字体都和原样一致，叠上去严丝合缝。
// 克隆去掉 id 和无障碍属性，免得和真选项撞 id、被读屏读两遍
function renderTint(triggerElements) {
  const tint = tintElement.value;
  if (!tint) return;
  tint.replaceChildren(...triggerElements.map((trigger, index) => {
    const clone = trigger.cloneNode(true);
    for (const node of [clone, ...clone.querySelectorAll("*")]) {
      for (const name of ["id", "role", "tabindex", "aria-controls", "aria-selected", "aria-labelledby"]) {
        node.removeAttribute(name);
      }
    }
    const { left, top, width, height } = boxes[index];
    Object.assign(clone.style, {
      position: "absolute",
      left: `${left}px`,
      top: `${top}px`,
      width: `${width}px`,
      height: `${height}px`,
      margin: "0",
      color: "var(--lens-tint-color, var(--xin-accent-strong))",
    });
    return clone;
  }));
}

useResizeObserver(root, measure);
// 选项文字、角标、禁用状态变了要重新量、重新克隆；胶囊总宽可能没变，ResizeObserver 不一定会响
watch(() => props.items, () => nextTick(measure), { deep: true });
// 键盘切换没有透镜，照旧让水滴拉伸回弹；透镜在的时候水滴藏着，不用演
watch(model, () => {
  if (!lensing.value && tabs.value?.$el) stretchIndicator(tabs.value.$el);
});

function lensEnabled(event) {
  return event.button === 0 && !event.ctrlKey && !prefersReducedMotion()
    && Boolean(event.target.closest('[role="tab"]:not([disabled])'));
}

// Reka 在 mousedown 时就选中并聚焦（聚焦又会触发选中），这里在捕获阶段拦下，选择留到松手
function holdSelection(event) {
  if (!lensEnabled(event)) return;
  event.preventDefault();
  event.stopPropagation();
}

function nearestBox(x) {
  return boxes.filter((box) => !box.disabled).reduce((nearest, box) => (
    !nearest || Math.abs(box.centerX - x) < Math.abs(nearest.centerX - x) ? box : nearest
  ), null);
}

function startLens(event) {
  if (!lensEnabled(event)) return;
  measure();
  rootLeft = root.value.getBoundingClientRect().left;
  pointerX = event.clientX - rootLeft;
  const selected = boxes.find((box) => box.value === model.value) ?? nearestBox(pointerX);
  if (!selected) return;
  press(selected, pointerX);

  window.addEventListener("pointermove", followPointer);
  window.addEventListener("pointerup", releaseLens);
  window.addEventListener("pointercancel", releaseLens);
  window.addEventListener("blur", releaseLens);
}

function followPointer(event) {
  pointerX = event.clientX - rootLeft;
  move(pointerX);
}

function removeListeners() {
  window.removeEventListener("pointermove", followPointer);
  window.removeEventListener("pointerup", releaseLens);
  window.removeEventListener("pointercancel", releaseLens);
  window.removeEventListener("blur", releaseLens);
}

function releaseLens() {
  removeListeners();
  // 按手指松开的位置选，不按透镜：快速甩过去时透镜还落在后面
  const target = nearestBox(pointerX);
  if (!target) return;
  release(target);
  // 透镜还鼓着、水滴藏着的时候切过去：等透镜落下，水滴已经在新位置上了
  if (target.value !== model.value) model.value = target.value;
}
</script>

<template>
  <div
    ref="root"
    class="liquid-lens-tabs"
    :class="{ 'is-lensing': lensing }"
    :style="{ '--lens-width': `${lensSize.width}px`, '--lens-height': `${lensSize.height}px` }"
    @mousedown.capture="holdSelection"
  >
    <div
      v-liquid-glass="glass"
      class="liquid-lens-tabs-glass liquid-glass"
      :class="{ 'liquid-glass--clear': clear }"
      aria-hidden="true"
    ></div>

    <UTabs
      ref="tabs"
      v-model="model"
      class="liquid-lens-tabs-control"
      variant="pill"
      :size="size"
      :items="items"
      :content="false"
      :ui="ui"
      :aria-label="ariaLabel"
      @pointerdown="startLens"
    />

    <div ref="tint" class="liquid-lens-tabs-tint" aria-hidden="true"></div>
    <div ref="lens" v-liquid-glass="LENS_GLASS" class="liquid-lens liquid-glass" aria-hidden="true"></div>
  </div>
</template>

<style scoped src="../../assets/css/liquid-lens-tabs.css"></style>
