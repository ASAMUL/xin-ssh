<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import OsIconTile from "../ConnectionManager/OsIconTile.vue";
import { resolveProfileOsIcon } from "../../config/osIcons";
import { formatRelativeTime } from "../../utils/relativeTime";

const props = defineProps({
  profile: {
    type: Object,
    required: true,
  },
  status: {
    type: String,
    default: "notOpen",
  },
  // 默认分组传 null：人人都有的「未分组」标签只是噪音，不显示
  groupName: {
    type: String,
    default: null,
  },
  // 「更多」菜单与右键菜单共用的一组 items（由首页按连接组装）
  menuItems: {
    type: Array,
    default: () => [],
  },
});

const emit = defineEmits(["open", "toggleFavorite"]);
const { t } = useI18n();

const osIcon = computed(() => resolveProfileOsIcon(props.profile));
const address = computed(() => {
  const target = `${props.profile.username}@${props.profile.host}`;
  return Number(props.profile.port) === 22 ? target : `${target}:${props.profile.port}`;
});
const hasSession = computed(() => props.status !== "notOpen");
const statusLabel = computed(() => (hasSession.value ? t(`monitor.status.${props.status}`) : ""));
const lastConnectedLabel = computed(() => formatRelativeTime(props.profile.lastConnectedAt, t));
const hasMenu = computed(() => props.menuItems.length > 0);
defineOptions({ inheritAttrs: false });
</script>

<template>
  <UContextMenu :items="menuItems" :disabled="!hasMenu" size="sm">
    <article v-bind="$attrs" class="connection-card liquid-rim" :class="[status, { favorite: profile.favorite }]">
      <button type="button" class="connection-card-main" @click="emit('open', profile)">
        <span class="connection-card-icon">
          <OsIconTile :icon="osIcon" :size="52" />
        </span>

        <span class="connection-card-copy">
          <strong>
            <i v-if="hasSession" class="connection-card-status" :title="statusLabel"></i>
            <span>{{ profile.name }}</span>
            <span v-if="profile.production" class="production-badge">{{ t("profile.productionBadge") }}</span>
          </strong>
          <span class="connection-card-meta">
            <span class="connection-card-address">{{ address }}</span>
            <UBadge v-if="groupName" class="connection-card-group" color="neutral" variant="soft" size="md" :label="groupName" />
            <em>· {{ lastConnectedLabel }}</em>
          </span>
        </span>
      </button>

      <span class="connection-card-tools">
        <UButton
          class="connection-card-star"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          icon="i-lucide-star"
          :aria-label="profile.favorite ? t('home.unfavorite') : t('home.favorite')"
          :title="profile.favorite ? t('home.unfavorite') : t('home.favorite')"
          @click="emit('toggleFavorite', profile.id)"
        />
        <UDropdownMenu v-if="hasMenu" :items="menuItems" size="sm" :content="{ align: 'end' }">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-ellipsis"
            :aria-label="t('home.moreActions')"
            :title="t('home.moreActions')"
          />
        </UDropdownMenu>
      </span>

      <UButton
        class="connection-card-connect"
        color="primary"
        variant="soft"
        size="sm"
        trailing-icon="i-lucide-arrow-right"
        :label="t('home.connect')"
        tabindex="-1"
        @click="emit('open', profile)"
      />
    </article>
  </UContextMenu>
</template>

<style scoped src="../../assets/css/connection-card.css"></style>
