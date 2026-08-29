<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import QuickConnectMenu from "./AppShell/QuickConnectMenu.vue";

const props = defineProps({
  sessions: {
    type: Array,
    default: () => [],
  },
  activeSessionId: {
    type: String,
    default: null,
  },
});

const emit = defineEmits(["showConnectionHub", "select", "close"]);
const { t } = useI18n();

const stripRef = ref(null);
const fadeStart = ref(false);
const fadeEnd = ref(false);
let resizeObserver = null;

// 标签溢出时两端渐隐，提示还能横向滚动
function updateOverflowFades() {
  const strip = stripRef.value;
  if (!strip) return;

  fadeStart.value = strip.scrollLeft > 2;
  fadeEnd.value = strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 2;
}

function scrollActiveTabIntoView() {
  if (!props.activeSessionId || !stripRef.value) return;

  const activeTab = stripRef.value.querySelector('[data-active="true"]');
  activeTab?.scrollIntoView({ inline: "nearest", block: "nearest", behavior: "smooth" });
}

onMounted(() => {
  updateOverflowFades();
  resizeObserver = new ResizeObserver(updateOverflowFades);
  if (stripRef.value) {
    resizeObserver.observe(stripRef.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
});

watch(
  () => [props.sessions.length, props.activeSessionId],
  async () => {
    await nextTick();
    scrollActiveTabIntoView();
    updateOverflowFades();
  },
);
</script>

<template>
  <nav class="session-tabs" :aria-label="t('workspace.openSessions')">
    <button
      type="button"
      class="home-tab"
      :class="{ active: !activeSessionId }"
      :title="t('workspace.connectionHub')"
      @click.stop="emit('showConnectionHub')"
    >
      <UIcon name="i-lucide-house" />
      <span>{{ t("workspace.connectionHub") }}</span>
    </button>

    <div
      ref="stripRef"
      class="tabs-strip"
      :class="{ 'fade-start': fadeStart, 'fade-end': fadeEnd }"
      @scroll.passive="updateOverflowFades"
    >
      <TransitionGroup name="tab">
        <div
          v-for="session in sessions"
          :key="session.id"
          class="session-tab"
          :class="{ active: session.id === activeSessionId }"
          :data-active="session.id === activeSessionId || null"
        >
          <button
            type="button"
            class="session-tab-main"
            :aria-label="session.name"
            :title="session.name"
            @click.stop="emit('select', session.id)"
          >
            <span class="tab-status" :class="session.status"></span>
            <span class="tab-title">{{ session.name }}</span>
          </button>
          <button
            type="button"
            class="tab-close"
            :aria-label="t('terminal.closeSession')"
            @click.stop="emit('close', session.id)"
          >
            <UIcon name="i-lucide-x" />
          </button>
        </div>
      </TransitionGroup>
    </div>

    <QuickConnectMenu />

    <div class="tabs-drag-space"></div>
  </nav>
</template>

<style scoped>
.session-tabs {
  min-width: 0;
  height: 100%;
  /* 首页 | 会话标签 | 加号 | 拖拽空域 —— 加号紧跟最后一个标签（Chrome 模式） */
  display: grid;
  grid-template-columns: auto minmax(0, max-content) auto minmax(12px, 1fr);
  align-items: center;
  gap: 6px;
  overflow: hidden;
  padding: 0 6px 0 4px;
}

.tabs-drag-space {
  height: 100%;
}

.home-tab,
.session-tab {
  height: 36px;
  border: 1px solid transparent;
  border-radius: 12px;
  background: transparent;
  box-shadow: none;
  color: var(--xin-text-muted);
  transition:
    background 160ms ease,
    border-color 160ms ease,
    color 160ms ease,
    box-shadow 160ms ease,
    transform 240ms cubic-bezier(0.32, 0.72, 0.24, 1);
}

.home-tab:hover,
.session-tab:hover {
  color: var(--xin-text);
  background: rgb(var(--xin-fill-rgb) / 0.42);
}

/* iOS 的选中态靠「发丝描边 + 顶部高光 + 极紧的两层投影」立起来，
   不是一团大扩散阴影：偏移 ≤2px、模糊 ≤6px。药丸这么小的尺寸下，
   大模糊阴影只会糊成一片灰、显廉价，边缘必须由描边来交代。 */
.home-tab.active,
.session-tab.active {
  color: var(--xin-text);
  border-color: rgb(var(--xin-ink-rgb) / 0.07);
  background: linear-gradient(180deg, rgb(var(--xin-fill-rgb) / 0.97) 0%, rgb(var(--xin-fill-rgb) / 0.8) 100%);
  box-shadow:
    var(--xin-specular-strong),
    0 1px 1px rgb(var(--xin-shadow-rgb) / 0.05),
    0 2px 6px -2px rgb(var(--xin-shadow-rgb) / 0.12);
  backdrop-filter: var(--xin-glass-blur);
  -webkit-backdrop-filter: var(--xin-glass-blur);
}

/* 按下去要有回弹，这是 iOS 手感里最省事也最像的一处 */
.home-tab:active,
.session-tab:active {
  transform: scale(0.97);
}

@media (prefers-reduced-motion: reduce) {
  .home-tab:active,
  .session-tab:active {
    transform: none;
  }
}

.home-tab {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 104px;
  padding: 0 15px;
  font-size: 12px;
  font-weight: 640;
  white-space: nowrap;
}

.home-tab :deep(svg) {
  width: 16px;
  height: 16px;
}

.tabs-strip {
  position: relative;
  min-width: 0;
  height: 100%;
  display: flex;
  align-items: center;
  gap: 6px;
  overflow-x: auto;
  scrollbar-width: none;
}

.tabs-strip::-webkit-scrollbar {
  display: none;
}

/* 溢出提示：可继续滚动的一侧渐隐 */
.tabs-strip.fade-start {
  mask-image: linear-gradient(to right, transparent 0, black 28px);
}

.tabs-strip.fade-end {
  mask-image: linear-gradient(to right, black calc(100% - 28px), transparent 100%);
}

.tabs-strip.fade-start.fade-end {
  mask-image: linear-gradient(
    to right,
    transparent 0,
    black 28px,
    black calc(100% - 28px),
    transparent 100%
  );
}

.session-tab {
  width: auto;
  min-width: 128px;
  max-width: 226px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 30px;
  align-items: center;
  flex: 0 0 auto;
}

/* 新标签淡入上浮；关闭后相邻标签平滑补位（FLIP） */
.tab-enter-active {
  transition:
    opacity 170ms ease,
    transform 170ms ease;
}

.tab-enter-from {
  opacity: 0;
  transform: translateY(5px) scale(0.94);
}

.tab-move {
  transition: transform 190ms ease;
}

.tab-leave-active {
  position: absolute;
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .tab-enter-active,
  .tab-move {
    transition: none;
  }
}

.session-tab-main {
  min-width: 0;
  height: 100%;
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  padding: 0 8px 0 13px;
  color: inherit;
  border: 0;
  background: transparent;
  text-align: left;
}

.tab-status {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--xin-status-idle);
  box-shadow: 0 0 0 4px rgb(var(--xin-neutral-rgb) / 0.12);
}

.tab-status.authRequired {
  background: var(--xin-accent);
  box-shadow: 0 0 0 4px rgb(var(--xin-accent-rgb) / 0.13);
}

.tab-status.connected {
  background: var(--xin-success);
  box-shadow: 0 0 0 4px rgb(var(--xin-success-rgb) / 0.16);
}

.tab-status.connecting {
  background: var(--xin-warning);
  box-shadow: 0 0 0 4px rgb(var(--xin-warning-rgb) / 0.14);
  animation: tab-status-pulse 1.15s ease-in-out infinite;
}

@keyframes tab-status-pulse {
  50% {
    opacity: 0.38;
  }
}

@media (prefers-reduced-motion: reduce) {
  .tab-status.connecting {
    animation: none;
  }
}

.tab-status.error {
  background: var(--xin-danger);
  box-shadow: 0 0 0 4px rgb(var(--xin-danger-rgb) / 0.12);
}

.tab-title {
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  font-weight: 640;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 关闭按钮悬停/激活/键盘聚焦才现身，平时保持安静 */
.tab-close {
  width: 22px;
  height: 22px;
  justify-self: center;
  display: grid;
  place-items: center;
  color: var(--xin-text-soft);
  font-size: 13px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  opacity: 0;
  transition:
    opacity 130ms ease,
    background 130ms ease,
    color 130ms ease,
    transform 200ms cubic-bezier(0.32, 0.72, 0.24, 1);
}

.session-tab:hover .tab-close,
.session-tab.active .tab-close,
.tab-close:focus-visible {
  opacity: 1;
}

.tab-close:hover {
  color: var(--xin-text);
  background: rgb(var(--xin-ink-rgb) / 0.09);
}

.tab-close:active {
  transform: scale(0.88);
}

@media (max-width: 980px) {
  .home-tab {
    min-width: 44px;
    justify-content: center;
    padding: 0 10px;
  }

  .home-tab span {
    display: none;
  }

  .session-tab {
    max-width: 168px;
  }
}

@media (max-width: 760px) {
  .session-tab {
    min-width: 108px;
    max-width: 140px;
  }
}
</style>
