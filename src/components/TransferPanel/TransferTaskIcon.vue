<script setup>
import { computed, ref, watch } from "vue";
import { CircleCheck } from "@respeak/lucide-motion-vue";
import MotionIcon from "../MotionIcon.vue";

const props = defineProps({
  task: {
    type: Object,
    required: true,
  },
});

const STATUS_ICONS = {
  planning: "i-lucide-folder-search",
  failed: "i-lucide-circle-alert",
  canceled: "i-lucide-circle-minus",
  paused: "i-lucide-circle-pause",
};

const isCompleted = computed(() => props.task.status === "completed");
const iconName = computed(
  () => STATUS_ICONS[props.task.status] ?? (props.task.direction === "upload" ? "i-lucide-arrow-up" : "i-lucide-arrow-down"),
);

// 只给亲眼看着传完的任务画对勾，从历史记录读出来的保持静止。
// 用 post 时机是要等对勾组件先挂载再置真：MotionIcon 只在挂载之后由假变真时才播放
const justCompleted = ref(false);
watch(
  isCompleted,
  (completed) => {
    justCompleted.value = completed;
  },
  { flush: "post" },
);
</script>

<template>
  <MotionIcon v-if="isCompleted" :icon="CircleCheck" animation="lucide-animated" :play-on="justCompleted" />
  <UIcon v-else :name="iconName" :class="{ 'spin-icon': task.status === 'running' }" />
</template>
