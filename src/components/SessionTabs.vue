<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import QuickConnectMenu from "./AppShell/QuickConnectMenu.vue";
import { useSortableList } from "../composables/useSortableList";

const props = defineProps({
  sessions: {
    type: Array,
    default: () => [],
  },
  activeSessionId: {
    type: String,
    default: null,
  },
  disabled: { type: Boolean, default: false },
});

const emit = defineEmits(["select", "close", "move", "dragging"]);
const { t } = useI18n();

const stripRef = ref(null);
const fadeStart = ref(false);
const fadeEnd = ref(false);
const isDragging = ref(false);
const sortingDisabled = computed(() => props.disabled || props.sessions.some((session) => session.isClosing));
let resizeObserver = null;

useSortableList(stripRef, {
  direction: "horizontal",
  draggable: ".session-tab",
  handle: ".session-tab-main",
  ghostClass: "session-sort-ghost",
  chosenClass: "session-sort-chosen",
  disabled: sortingDisabled,
  onStart: () => setDragging(true),
  onEnd: () => setDragging(false),
  onDrop: ({ id, beforeId }) => emit("move", { id, beforeId }),
});

function setDragging(dragging) {
  isDragging.value = dragging;
  emit("dragging", dragging);
}

function selectSessionTab(session) {
  if (!props.disabled && !isDragging.value && !session.isClosing) emit("select", session.id);
}

function closeSessionTab(session) {
  if (!props.disabled && !isDragging.value && !session.isClosing) emit("close", session.id);
}

function handleTabMouseDown(event) {
  if (event.button === 1) event.preventDefault();
}

// 菜单与拖动共用 beforeId 语义，左右移动不依赖会在异步关闭后失效的旧下标。
async function moveSessionTab(sessionId, offset) {
  if (sortingDisabled.value || isDragging.value) return;
  const index = props.sessions.findIndex((session) => session.id === sessionId);
  const target = index + offset;
  if (index < 0 || target < 0 || target >= props.sessions.length) return;
  const beforeId = offset < 0 ? props.sessions[target].id : props.sessions[target + 1]?.id ?? null;
  emit("move", { id: sessionId, beforeId });
  await nextTick();
  if (props.disabled) return;
  // Vue 移动节点可能让按钮失焦，恢复到被移动的标签才能连续用键盘调整顺序。
  const tab = [...(stripRef.value?.children ?? [])].find((element) => element.dataset.sortId === sessionId);
  tab?.querySelector(".session-tab-main")?.focus({ preventScroll: true });
  tab?.scrollIntoView({ inline: "nearest", block: "nearest" });
}

// 拖动手柄不增加可见按钮；菜单提供同样的排序操作，标签两端明确禁用无效方向。
function tabMenuItems(session) {
  const index = props.sessions.findIndex((item) => item.id === session.id);
  return [
    [
      {
        label: t("workspace.moveTabLeft"), icon: "i-lucide-arrow-left",
        disabled: sortingDisabled.value || index <= 0,
        onSelect: () => moveSessionTab(session.id, -1),
      },
      {
        label: t("workspace.moveTabRight"), icon: "i-lucide-arrow-right",
        disabled: sortingDisabled.value || index >= props.sessions.length - 1,
        onSelect: () => moveSessionTab(session.id, 1),
      },
    ],
    [{ label: t("terminal.closeSession"), icon: "i-lucide-x", onSelect: () => closeSessionTab(session) }],
  ];
}

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
  const behavior = window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth";
  activeTab?.scrollIntoView({ inline: "nearest", block: "nearest", behavior });
}

onMounted(() => {
  updateOverflowFades();
  scrollActiveTabIntoView();
  resizeObserver = new ResizeObserver(updateOverflowFades);
  if (stripRef.value) {
    resizeObserver.observe(stripRef.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  setDragging(false);
});

watch(
  () => [props.sessions.map((session) => session.id).join(","), props.activeSessionId],
  async ([, activeId], previous = []) => {
    await nextTick();
    if (activeId !== previous[1]) scrollActiveTabIntoView();
    updateOverflowFades();
  },
);
</script>

<template>
  <nav class="session-tabs" :aria-label="t('workspace.openSessions')">
    <div
      ref="stripRef"
      class="tabs-strip"
      :class="{ 'fade-start': fadeStart, 'fade-end': fadeEnd, 'is-dragging': isDragging }"
      @scroll.passive="updateOverflowFades"
    >
      <TransitionGroup name="tab">
        <div
          v-for="session in sessions"
          :key="session.id"
          class="session-tab"
          :class="{ active: session.id === activeSessionId, closing: session.isClosing }"
          :data-active="session.id === activeSessionId || null"
          :data-sort-id="session.id"
          :aria-busy="session.isClosing"
          @mousedown="handleTabMouseDown"
          @auxclick.middle.stop.prevent="closeSessionTab(session)"
        >
          <UContextMenu :items="tabMenuItems(session)" :disabled="disabled || session.isClosing || isDragging" size="xs">
            <div class="session-tab-content">
              <button
                type="button"
                class="session-tab-main"
                :disabled="disabled || session.isClosing"
                :aria-label="session.name"
                :aria-pressed="session.id === activeSessionId"
                :title="session.name"
                @click.stop="selectSessionTab(session)"
                @keydown.ctrl.shift.left.exact.stop.prevent="moveSessionTab(session.id, -1)"
                @keydown.ctrl.shift.right.exact.stop.prevent="moveSessionTab(session.id, 1)"
              >
                <span class="tab-status" :class="session.status"></span>
                <span class="tab-title">{{ session.name }}</span>
              </button>
              <UButton
                color="neutral" variant="ghost" size="xs" square icon="i-lucide-x"
                class="tab-close"
                :disabled="disabled || session.isClosing"
                :loading="session.isClosing"
                :aria-label="t('terminal.closeSession')"
                :title="t('terminal.closeSession')"
                @click.stop="closeSessionTab(session)"
              />
            </div>
          </UContextMenu>
        </div>
      </TransitionGroup>
    </div>

    <QuickConnectMenu :disabled="disabled || isDragging" />
  </nav>
</template>

<style scoped src="../assets/css/session-tabs.css"></style>
