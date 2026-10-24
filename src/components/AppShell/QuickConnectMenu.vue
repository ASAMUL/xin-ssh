<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useUiStore } from "../../stores/uiStore";

const RECENT_LIMIT = 5;
const props = defineProps({ disabled: { type: Boolean, default: false } });

const { t } = useI18n();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const uiStore = useUiStore();

const open = ref(false);
watch(() => props.disabled, (disabled) => {
  if (disabled) open.value = false;
});

// profileId -> 聚合会话状态（多终端时取“最好”的一个），给列表项的状态点上色
const sessionStatusByProfile = computed(() => sessionStore.statusByProfile);

const hasProfiles = computed(() => profileStore.activeProfiles.length > 0);

const groups = computed(() => {
  const sections = [];
  const recentProfiles = profileStore.recentProfiles.slice(0, RECENT_LIMIT);
  if (recentProfiles.length > 0) {
    sections.push({
      id: "recent",
      label: t("quickConnect.recent"),
      items: recentProfiles.map(profileToItem),
    });
  }

  for (const { group, profiles } of profileStore.profilesByGroup) {
    if (profiles.length === 0) continue;

    sections.push({
      id: group.id,
      label: groupLabel(group),
      items: profiles.map(profileToItem),
    });
  }

  return sections;
});

function groupLabel(group) {
  if (group.name !== "Ungrouped") return group.name;

  // 用户没有自建分组时，默认组直接叫“全部连接”更符合直觉
  const visibleGroups = profileStore.profilesByGroup.filter((entry) => entry.profiles.length > 0);
  return visibleGroups.length > 1 ? t("sidebar.ungrouped") : t("quickConnect.all");
}

function profileToItem(profile) {
  return {
    label: profile.name,
    suffix: profileAddress(profile),
    chip: { color: statusColor(sessionStatusByProfile.value[profile.id]) },
    onSelect: () => openProfile(profile),
  };
}

function profileAddress(profile) {
  const address = `${profile.username}@${profile.host}`;
  return Number(profile.port) === 22 ? address : `${address}:${profile.port}`;
}

// 未打开=灰点；已打开会话沿用状态色（绿=已连接、黄=连接中、红=出错、蓝=待认证/已断开）
function statusColor(status) {
  if (!status) return "neutral";
  if (status === "connected") return "success";
  if (status === "connecting") return "warning";
  if (status === "error") return "error";
  return "info";
}

function openProfile(profile) {
  if (props.disabled) return;
  open.value = false;
  // 加号 = 总是新开一个终端（Windows Terminal 心理模型）；切换已有会话用顶栏 tab
  sessionStore.openProfileSession(profile, { forceNew: true });
}

function createConnection() {
  if (props.disabled) return;
  open.value = false;
  uiStore.openConnectionForm();
}
</script>

<template>
  <UPopover
    v-model:open="open"
    :content="{ side: 'bottom', align: 'start', sideOffset: 10, collisionPadding: 12 }"
    :ui="{ content: 'quick-connect-popover' }"
  >
    <UButton
      class="quick-connect-trigger"
      :class="{ open }"
      color="neutral"
      variant="ghost"
      size="sm"
      icon="i-lucide-plus"
      :disabled="disabled"
      :aria-label="t('quickConnect.open')"
      :title="t('quickConnect.open')"
    />

    <template #content>
      <UCommandPalette
        size="sm"
        :groups="groups"
        :placeholder="t('quickConnect.placeholder')"
      >
        <template #empty>
          <div class="quick-connect-empty">
            <UIcon :name="hasProfiles ? 'i-lucide-search-x' : 'i-lucide-server-off'" />
            <p>{{ hasProfiles ? t("quickConnect.noMatches") : t("quickConnect.empty") }}</p>
            <span v-if="!hasProfiles">{{ t("quickConnect.emptyHint") }}</span>
          </div>
        </template>

        <template #footer>
          <UButton
            class="quick-connect-create"
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-lucide-plus"
            block
            :ui="{ base: 'justify-start' }"
            @click="createConnection"
          >
            {{ t("quickConnect.create") }}
          </UButton>
        </template>
      </UCommandPalette>
    </template>
  </UPopover>
</template>

<style scoped>
/* 加号：平时安静的幽灵按钮，弹层打开时变成和选中标签同一颗透明玻璃水滴——
   两者并排在同一条玻璃胶囊里，必须是同一种材质。 */
.quick-connect-trigger {
  width: 34px;
  height: 34px;
  justify-content: center;
  color: var(--xin-text-muted);
  border: 1px solid transparent;
  border-radius: 10px;
  transition:
    background 160ms ease,
    color 160ms ease,
    border-color 160ms ease,
    box-shadow 160ms ease,
    transform 240ms cubic-bezier(0.32, 0.72, 0.24, 1);
}

.quick-connect-trigger:hover {
  color: var(--xin-text);
  background: rgb(var(--xin-fill-rgb) / 0.42);
}

.quick-connect-trigger:active {
  transform: scale(0.94);
}

.quick-connect-trigger.open {
  color: var(--xin-accent-strong);
  background: var(--lg-droplet-clear);
  box-shadow: var(--lg-droplet-clear-shadow);
}

@media (prefers-reduced-motion: reduce) {
  .quick-connect-trigger:active {
    transform: none;
  }
}
</style>
