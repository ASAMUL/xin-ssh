<script setup>
import { computed } from "vue";
import { FALLBACK_OS_ICON } from "../../config/osIcons";

const props = defineProps({
  // config/osIcons.js 目录里的一项；由调用方用 resolveProfileOsIcon 解析好再传进来
  icon: { type: Object, default: null },
  size: { type: Number, default: 48 },
  // paper = 瓷白底座（默认）；ink = 压在墨色会话卡上的暗底座
  tone: { type: String, default: "paper" },
});

const resolvedIcon = computed(() => props.icon ?? FALLBACK_OS_ICON);
</script>

<template>
  <span
    class="os-icon-tile"
    :class="[tone, { mono: resolvedIcon.mono }]"
    :style="{ '--tile-size': `${size}px` }"
    aria-hidden="true"
  >
    <UIcon :name="resolvedIcon.icon" />
  </span>
</template>

<style scoped src="../../assets/css/os-icon-tile.css"></style>
