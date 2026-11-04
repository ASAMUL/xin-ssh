<script setup>
import { computed, ref, watch } from "vue";
import { usePreferredReducedMotion } from "@vueuse/core";
import { AnimateIcon } from "@respeak/lucide-motion-vue";

const props = defineProps({
  // @respeak/lucide-motion-vue 导出的图标组件；包声明了 sideEffects: false，从入口引入也只打包用到的那几个
  icon: {
    type: Object,
    required: true,
  },
  animation: {
    type: String,
    default: "default",
  },
  // 悬停整颗按钮就播放：图标本身只有十几像素，只认图标的话很难碰到
  hover: {
    type: Boolean,
    default: false,
  },
  // 每次由假变真播放一次；挂载时就为真不播，否则首页一打开所有已收藏的星星会一起跳
  playOn: {
    type: Boolean,
    default: false,
  },
  // 和 UIcon 一样默认 1em；写成属性而不是 CSS，Nuxt UI 给的 size-4 之类的类名才能盖过它
  size: {
    type: [Number, String],
    default: "1em",
  },
});

const reducedMotion = usePreferredReducedMotion();
const motionAllowed = computed(() => reducedMotion.value !== "reduce");

const playing = ref(false);
watch(
  () => props.playOn,
  (on) => {
    playing.value = on;
  },
);
</script>

<template>
  <!--
    外面套一层 AnimateIcon，图标自己就不带触发器：播放开关来回切时组件树保持不变，动画不会被重挂载打断。
    playOn 模式停在终帧：开关在动画中途关掉时，不会倒放回去。
  -->
  <AnimateIcon
    :animation="animation"
    :animate-on-hover="motionAllowed && hover"
    :animate="motionAllowed && playing"
    :persist-on-animate-end="!hover"
    :trigger-target="hover ? 'closest:button' : 'self'"
  >
    <component :is="icon" :size="size" aria-hidden="true" />
  </AnimateIcon>
</template>
