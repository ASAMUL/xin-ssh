<script setup>
import { computed } from "vue";
import { useBackgroundUrl } from "../../composables/useBackgroundUrl";
import { useSettingsStore } from "../../stores/settingsStore";

// 应用壁纸：垫在整个窗口最底层的一张图，液态玻璃终于有「底下的东西」可折射。
// 默认没有；设置 → 应用壁纸里上传。它是窗口的第一个定位子元素，
// 顶栏和内容区（都是 position: relative）按文档顺序画在它上面，不用额外的 z-index。
const settingsStore = useSettingsStore();
const wallpaper = computed(() => settingsStore.appWallpaper);
const imageUrl = useBackgroundUrl(() => wallpaper.value?.file ?? null);

const layerStyle = computed(() => ({
  "--wallpaper-image": `url("${imageUrl.value}")`,
  "--wallpaper-opacity": wallpaper.value.opacity,
  "--wallpaper-blur": `${wallpaper.value.blur}px`,
}));
</script>

<template>
  <div v-if="wallpaper && imageUrl" class="app-wallpaper" :style="layerStyle" aria-hidden="true"></div>
</template>

<style scoped src="../../assets/css/app-wallpaper.css"></style>
