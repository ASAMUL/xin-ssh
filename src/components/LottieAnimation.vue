<script setup>
import { onBeforeUnmount, onMounted, useTemplateRef, watch } from "vue";
import { usePreferredReducedMotion } from "@vueuse/core";
import { DotLottie } from "@lottiefiles/dotlottie-web";
import wasmUrl from "@lottiefiles/dotlottie-web/dotlottie-player.wasm?url";
import { resolvedAppearance } from "../utils/appearanceMode";

// 播放器默认去 jsdelivr / unpkg 拉 WASM，应用离线就放不出来；打进包里走本地地址，
// setWasmUrl 顺带关掉 CDN 兜底，本地加载失败会直接报错，不会悄悄连外网
DotLottie.setWasmUrl(wasmUrl);

const props = defineProps({
  // .lottie 文件地址（`import url from "….lottie?url"`）
  src: {
    type: String,
    required: true,
  },
  // 深色模式下要换掉的颜色槽：{ 槽 id: [r, g, b, a] }，0~1。
  // 素材是照着浅底画的，放到深色卡片上压不住的那几块（深灰线条之类）在文件里标了槽，这里换色；
  // 一个文件管两套明暗，不用找两份素材
  darkSlots: {
    type: Object,
    default: () => ({}),
  },
});

const canvas = useTemplateRef("canvas");
const reducedMotion = usePreferredReducedMotion();
let player = null;

function paintSlots() {
  if (!player?.isLoaded) return;

  if (resolvedAppearance.value === "dark") {
    for (const [slotId, color] of Object.entries(props.darkSlots)) player.setColorSlot(slotId, color);
  } else {
    player.resetSlots();
  }
}

onMounted(() => {
  player = new DotLottie({
    canvas: canvas.value,
    src: props.src,
    loop: true,
    // 系统开了「减少动态效果」就停在第一帧，画面还在、只是不动
    autoplay: reducedMotion.value !== "reduce",
    // 滚出视口、切到工作区（v-show 藏起来）时播放器自己冻住，不白白每帧重画
    renderConfig: { autoResize: true, freezeOnOffscreen: true },
  });
  player.addEventListener("load", paintSlots);
});

onBeforeUnmount(() => {
  player?.destroy();
  player = null;
});

watch(resolvedAppearance, paintSlots);
watch(reducedMotion, (preference) => {
  if (!player?.isLoaded) return;
  if (preference === "reduce") player.stop();
  else player.play();
});
</script>

<template>
  <canvas ref="canvas" class="lottie-animation" aria-hidden="true"></canvas>
</template>

<style scoped src="../assets/css/lottie-animation.css"></style>
