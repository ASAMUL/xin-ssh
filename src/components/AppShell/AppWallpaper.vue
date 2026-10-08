<script setup>
import { computed } from "vue";
import { useBackgroundUrl } from "../../composables/useBackgroundUrl";
import { BUILT_IN_WALLPAPERS } from "../../config/appWallpaper";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { resolvedAppearance } from "../../utils/appearanceMode";

// 应用壁纸：垫在整个窗口最底层的一张图，液态玻璃终于有「底下的东西」可折射。
// 浅色 / 深色各一张，跟着实际明暗换；没换过的那一档用内置图。它是窗口的第一个定位子元素，
// 顶栏和内容区（都是 position: relative）按文档顺序画在它上面，不用额外的 z-index。
const appStore = useAppStore();
const settingsStore = useSettingsStore();
const wallpaper = computed(() => settingsStore.appWallpaper);
// settings.json 是异步读的：读完之前先不画，免得换过壁纸的用户每次启动都先闪一下内置图。
// 不等 settings 本身而是等加载结束：工作区读失败时也该有内置图垫底
const isReady = computed(() => !appStore.isLoading);
const imageUrl = useBackgroundUrl(
  () => (isReady.value ? wallpaper.value[resolvedAppearance.value] : null),
  () => (isReady.value ? BUILT_IN_WALLPAPERS[resolvedAppearance.value] : null),
);

const layerStyle = computed(() => ({
  "--wallpaper-image": `url("${imageUrl.value}")`,
  "--wallpaper-opacity": wallpaper.value.opacity,
  "--wallpaper-blur": `${wallpaper.value.blur}px`,
}));
</script>

<template>
  <div v-if="imageUrl" class="app-wallpaper" :style="layerStyle" aria-hidden="true"></div>
</template>

<style scoped src="../../assets/css/app-wallpaper.css"></style>
