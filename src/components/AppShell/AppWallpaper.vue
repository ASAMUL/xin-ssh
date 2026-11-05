<script setup>
import { computed, onBeforeUnmount, ref, watch, watchEffect } from "vue";
import { useBackgroundUrl } from "../../composables/useBackgroundUrl";
import { BUILT_IN_WALLPAPERS } from "../../config/appWallpaper";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { resolvedAppearance } from "../../utils/appearanceMode";
import { measureWallpaperTone, needsLegibilityAssist } from "../../utils/wallpaperTone";

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

// 壁纸压不住字时（浅色模式配暗图、深色模式配亮图）打开的开关：
// 挂在 <html> 上给透明玻璃和首页空状态用（liquid-glass.css、home-dashboard.css），这里再铺一道顶部淡化纱
const needsAssist = ref(false);

watch(
  [imageUrl, resolvedAppearance, () => wallpaper.value.opacity],
  async ([url, mode, opacity], _previous, onCleanup) => {
    let stale = false;
    onCleanup(() => { stale = true; });
    // 内置图本来就是照着两套配色挑的
    if (!url || url === BUILT_IN_WALLPAPERS[mode]) {
      needsAssist.value = false;
      return;
    }

    try {
      const tone = await measureWallpaperTone(url);
      if (!stale) needsAssist.value = needsLegibilityAssist(tone, mode, opacity);
    } catch (error) {
      // 量不出来就保持原样：最差也只是回到没有这层保护的样子
      console.warn("[Xin SSH] wallpaper tone unavailable", error);
      if (!stale) needsAssist.value = false;
    }
  },
  { immediate: true },
);

watchEffect(() => {
  document.documentElement.toggleAttribute("data-wallpaper-assist", needsAssist.value);
});

onBeforeUnmount(() => {
  document.documentElement.removeAttribute("data-wallpaper-assist");
});
</script>

<template>
  <template v-if="imageUrl">
    <div class="app-wallpaper" :style="layerStyle" aria-hidden="true"></div>
    <div v-if="needsAssist" class="app-wallpaper-veil" aria-hidden="true"></div>
  </template>
</template>

<style scoped src="../../assets/css/app-wallpaper.css"></style>
