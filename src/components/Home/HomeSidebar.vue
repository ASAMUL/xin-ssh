<script setup>
import { useI18n } from "vue-i18n";

defineProps({
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

const emit = defineEmits(["selectNav", "selectGroup", "openSettings", "openTunnels"]);
const { t } = useI18n();

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

    <div v-if="groups.length > 0" class="home-sidebar-section">
      <span class="home-sidebar-label">{{ t("home.groups") }}</span>
      <button
        v-for="({ group, name, count }, index) in groups"
        :key="group.id"
        type="button"
        class="home-sidebar-item home-sidebar-group"
        :class="{ active: activeGroupId === group.id }"
        :style="groupColor(index)"
        @click="emit('selectGroup', group.id)"
      >
        <span class="home-sidebar-group-icon">
          <UIcon name="i-lucide-folder" />
        </span>
        <span>{{ name }}</span>
        <em>{{ count }}</em>
      </button>
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
