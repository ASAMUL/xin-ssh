<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
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
  // 「更多」菜单与右键菜单共用的一组 items（由首页按连接组装）
  menuItems: {
    type: Array,
    default: () => [],
  },
  sortable: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits(["open", "edit", "toggleFavorite"]);
const { t } = useI18n();

const statusLabel = computed(() =>
  props.status === "notOpen" ? t("workspace.notOpen") : t(`monitor.status.${props.status}`),
);
const lastConnectedLabel = computed(() =>
  formatRelativeTime(props.profile.lastConnectedAt, t),
);
const hasMenu = computed(() => props.menuItems.length > 0);
defineOptions({ inheritAttrs: false });
</script>

<template>
  <UContextMenu :items="menuItems" :disabled="!hasMenu" size="xs">
    <article v-bind="$attrs" class="connection-card" :class="[status, { favorite: profile.favorite }]">
      <button type="button" class="connection-card-main" @click="emit('open', profile)">
        <span
          class="connection-card-icon"
          :class="{ 'connection-card-drag-handle': sortable }"
          :title="sortable ? t('home.sortConnection') : undefined"
        >
          <UIcon name="i-lucide-server" />
        </span>

        <span class="connection-card-copy">
          <strong>{{ profile.name }}</strong>
          <small>{{ profile.username }}@{{ profile.host }}:{{ profile.port }}</small>
        </span>

        <span class="connection-card-meta">
          <span class="connection-card-status">
            <i></i>
            <span>{{ statusLabel }}</span>
          </span>
          <em>{{ lastConnectedLabel }}</em>
        </span>
      </button>

      <span class="connection-card-tools">
        <UButton
          class="connection-card-star"
          color="neutral"
          variant="ghost"
          size="xs"
          icon="i-lucide-star"
          :aria-label="profile.favorite ? t('home.unfavorite') : t('home.favorite')"
          :title="profile.favorite ? t('home.unfavorite') : t('home.favorite')"
          @click="emit('toggleFavorite', profile.id)"
        />
        <UButton
          color="neutral"
          variant="ghost"
          size="xs"
          icon="i-lucide-pencil"
          :aria-label="t('sidebar.editConnection')"
          :title="t('sidebar.editConnection')"
          @click="emit('edit', profile.id)"
        />
        <UDropdownMenu v-if="hasMenu" :items="menuItems" size="xs" :content="{ align: 'end' }">
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-lucide-ellipsis"
            :aria-label="t('home.moreActions')"
            :title="t('home.moreActions')"
          />
        </UDropdownMenu>
      </span>
    </article>
  </UContextMenu>
</template>

<style scoped src="../../assets/css/connection-card.css"></style>
