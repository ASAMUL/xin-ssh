<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import OsIconTile from "../ConnectionManager/OsIconTile.vue";
import { resolveProfileOsIcon } from "../../config/osIcons";

const props = defineProps({
  session: { type: Object, required: true },
  profile: { type: Object, required: true },
  menuItems: { type: Array, default: () => [] },
});
const emit = defineEmits(["resume"]);
const { t } = useI18n();

const osIcon = computed(() => resolveProfileOsIcon(props.profile));
const address = computed(() => {
  const target = `${props.session.username}@${props.session.host}`;
  return Number(props.session.port) === 22 ? target : `${target}:${props.session.port}`;
});
const statusLabel = computed(() => t(`monitor.status.${props.session.status}`));
</script>

<template>
  <article class="home-session-card liquid-rim" :class="session.status">
    <div class="home-session-texture" aria-hidden="true"></div>

    <OsIconTile :icon="osIcon" :size="60" tone="ink" />

    <div class="home-session-copy">
      <strong>
        <i class="home-session-status" :title="statusLabel"></i>
        <span>{{ session.name }}</span>
      </strong>
      <span class="home-session-address">{{ address }}</span>
      <span class="home-session-path">
        <UIcon name="i-lucide-folder" />
        <span>{{ t("home.currentDirectory") }}</span>
        <code>{{ session.remotePath || "/" }}</code>
      </span>
    </div>

    <div class="home-session-actions">
      <UButton
        class="home-session-resume"
        size="lg"
        trailing-icon="i-lucide-arrow-right"
        :label="t('home.resumeSession')"
        @click="emit('resume', session.id)"
      />
      <UDropdownMenu v-if="menuItems.length" :items="menuItems" size="sm" :content="{ align: 'end' }">
        <UButton
          class="home-session-more"
          color="neutral"
          variant="ghost"
          size="lg"
          square
          icon="i-lucide-ellipsis"
          :aria-label="t('home.moreActions')"
          :title="t('home.moreActions')"
        />
      </UDropdownMenu>
    </div>
  </article>
</template>

<style scoped src="../../assets/css/home-session-card.css"></style>
