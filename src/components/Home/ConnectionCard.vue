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
});

const emit = defineEmits(["open", "edit", "toggleFavorite"]);
const { t } = useI18n();

const statusLabel = computed(() =>
  props.status === "notOpen" ? t("workspace.notOpen") : t(`monitor.status.${props.status}`),
);
const lastConnectedLabel = computed(() =>
  formatRelativeTime(props.profile.lastConnectedAt, t),
);
</script>

<template>
  <article class="connection-card" :class="[status, { favorite: profile.favorite }]">
    <button type="button" class="connection-card-main" @click="emit('open', profile)">
      <span class="connection-card-icon">
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
    </span>
  </article>
</template>

<style scoped src="../../assets/css/connection-card.css"></style>
