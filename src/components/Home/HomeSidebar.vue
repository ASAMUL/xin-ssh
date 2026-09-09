<script setup>
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useHomeSortable } from "../../composables/useHomeSortable";

const props = defineProps({
  busy: { type: Boolean, default: false },
  navItems: {
    type: Array,
    default: () => [],
  },
  groups: {
    type: Array,
    default: () => [],
  },
  activeNav: {
    type: String,
    default: "home",
  },
  activeGroupId: {
    type: String,
    default: null,
  },
});

const emit = defineEmits([
  "selectNav",
  "selectGroup",
  "createGroup",
  "renameGroup",
  "deleteGroup",
  "reorderGroups",
  "openSettings",
  "openTunnels",
]);
const { t } = useI18n();
const groupList = ref(null);
useHomeSortable(groupList, {
  draggable: ".home-sidebar-group-row",
  handle: ".home-group-drag-handle",
  disabled: () => props.busy,
  onDrop: ({ order }) => emit("reorderGroups", order),
});

// 默认分组是「未分组」兜底位，不能改名也不能删
const DEFAULT_GROUP_ID = "group-default";

// 菜单提供拖拽的键盘等价入口，同时阻止普通分组越过置顶的默认分组。
function groupMenuItems(group) {
  const index = props.groups.findIndex((item) => item.group.id === group.id);
  return [
    [
      {
        label: t("sidebar.renameGroup"),
        icon: "i-lucide-pencil",
        onSelect: () => emit("renameGroup", group),
      },
    ],
    [
      {
        label: t("home.moveUp"),
        icon: "i-lucide-arrow-up",
        disabled: props.busy || index <= 1,
        onSelect: () => moveGroup(index, -1),
      },
      {
        label: t("home.moveDown"),
        icon: "i-lucide-arrow-down",
        disabled: props.busy || index === props.groups.length - 1,
        onSelect: () => moveGroup(index, 1),
      },
    ],
    [
      {
        label: t("sidebar.deleteGroup"),
        icon: "i-lucide-trash-2",
        color: "error",
        onSelect: () => emit("deleteGroup", group),
      },
    ],
  ];
}

function moveGroup(index, offset) {
  const ids = props.groups.map(({ group }) => group.id);
  const [id] = ids.splice(index, 1);
  ids.splice(index + offset, 0, id);
  emit("reorderGroups", ids);
}

// 分组图标的点缀色，按序循环
const GROUP_COLORS = [
  { color: "var(--xin-accent)", soft: "rgb(var(--xin-accent-rgb) / 0.14)" },
  { color: "var(--xin-success-strong)", soft: "rgb(var(--xin-success-rgb) / 0.14)" },
  { color: "#a855f7", soft: "rgba(168, 85, 247, 0.13)" },
  { color: "#e8993c", soft: "rgb(var(--xin-warning-rgb) / 0.15)" },
  { color: "#ec4899", soft: "rgba(236, 72, 153, 0.12)" },
  { color: "#0ea5e9", soft: "rgba(14, 165, 233, 0.14)" },
];

function groupColor(index) {
  const palette = GROUP_COLORS[index % GROUP_COLORS.length];
  return { "--group-color": palette.color, "--group-soft": palette.soft };
}
</script>

<template>
  <aside class="home-sidebar">
    <nav class="home-sidebar-nav" :aria-label="t('home.nav.home')">
      <button
        v-for="item in navItems"
        :key="item.id"
        type="button"
        class="home-sidebar-item"
        :class="{ active: activeNav === item.id && !activeGroupId }"
        @click="emit('selectNav', item.id)"
      >
        <UIcon :name="item.icon" class="nav-icon" />
        <span>{{ item.label }}</span>
        <em v-if="item.count != null">{{ item.count }}</em>
      </button>
    </nav>

    <div class="home-sidebar-section">
      <span class="home-sidebar-label home-sidebar-label-row">
        <span>{{ t("home.groups") }}</span>
        <UButton
          color="neutral"
          variant="ghost"
          size="xs"
          icon="i-lucide-plus"
          :aria-label="t('sidebar.addGroup')"
          :title="t('sidebar.addGroup')"
          @click="emit('createGroup')"
        />
      </span>
      <div ref="groupList" class="home-sidebar-group-list">
        <div
          v-for="({ group, name, count }, index) in groups"
          :key="group.id"
          class="home-sidebar-group-row"
          :data-sort-id="group.id"
          :data-sort-locked="group.id === DEFAULT_GROUP_ID"
          :style="groupColor(index)"
        >
          <button
            type="button"
            class="home-sidebar-item home-sidebar-group"
            :class="{ active: activeGroupId === group.id }"
            @click="emit('selectGroup', group.id)"
          >
            <span
              class="home-sidebar-group-icon"
              :class="{ 'home-group-drag-handle': group.id !== DEFAULT_GROUP_ID }"
              :title="group.id !== DEFAULT_GROUP_ID ? t('home.sortGroup') : undefined"
            >
              <UIcon name="i-lucide-folder" />
            </span>
            <span>{{ name }}</span>
            <em>{{ count }}</em>
          </button>
          <UDropdownMenu
            v-if="group.id !== DEFAULT_GROUP_ID"
            :items="groupMenuItems(group)"
            size="xs"
            :content="{ align: 'start', side: 'right' }"
          >
            <UButton
              class="home-sidebar-group-menu"
              :disabled="busy"
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-lucide-ellipsis"
              :aria-label="t('home.moreActions')"
              :title="t('home.moreActions')"
            />
          </UDropdownMenu>
        </div>
      </div>
    </div>

    <div class="home-sidebar-section">
      <span class="home-sidebar-label">{{ t("home.tools") }}</span>
      <button type="button" class="home-sidebar-item" @click="emit('openTunnels')">
        <UIcon name="i-lucide-waypoints" class="nav-icon" />
        <span>{{ t("tunnel.title") }}</span>
      </button>
    </div>

    <div class="home-sidebar-footer">
      <button type="button" class="home-sidebar-item" @click="emit('openSettings')">
        <UIcon name="i-lucide-settings" class="nav-icon" />
        <span>{{ t("app.settings") }}</span>
      </button>
    </div>
  </aside>
</template>

<style scoped src="../../assets/css/home-sidebar.css"></style>
