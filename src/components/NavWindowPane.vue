<script setup>
import { useTemplateRef } from "vue";
import { useScroll } from "@vueuse/core";

// 左导航窗口（设置、系统信息）右侧的一页：标题和工具栏钉在顶上，只有下面的内容滚动。
// 整页一起滚的话，往下翻时连“这是哪一页”都跟着滚走了，搜索框也得滚回顶上才能用
defineProps({
  title: {
    type: String,
    required: true,
  },
});

const body = useTemplateRef("body");
// 内容滚到标题底下时才画分隔线：停在顶部时多一条线显得碎，和 macOS 设置窗口一样
const { arrivedState } = useScroll(body);
</script>

<template>
  <header class="nav-window-pane-header" :class="{ 'is-scrolled': !arrivedState.top }">
    <h3 class="nav-window-pane-title">{{ title }}</h3>
    <slot name="toolbar" />
  </header>
  <div ref="body" class="nav-window-pane-body">
    <slot />
  </div>
</template>
