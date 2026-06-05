<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps({
  profileGroups: {
    type: Array,
    default: () => [],
  },
  profileStatuses: {
    type: Object,
    default: () => ({}),
  },
});

const emit = defineEmits(["openProfile", "editProfile", "createConnection"]);
const { t } = useI18n();
const query = ref("");

const allProfiles = computed(() =>
  props.profileGroups.flatMap(({ profiles }) => profiles),
);
const hasProfiles = computed(() => allProfiles.value.length > 0);
const filteredGroups = computed(() => {
  const normalizedQuery = query.value.trim().toLowerCase();

  return props.profileGroups
    .map(({ group, profiles }) => ({
      group,
      profiles: normalizedQuery
        ? profiles.filter((profile) => matchesProfile(profile, normalizedQuery))
        : profiles,
    }))
    .filter(({ profiles }) => profiles.length > 0);
});
function matchesProfile(profile, normalizedQuery) {
  return [profile.name, profile.host, profile.username, profile.defaultRemotePath]
    .some((value) => String(value ?? "").toLowerCase().includes(normalizedQuery));
}

function groupDisplayName(group) {
  return group.name === "Ungrouped" ? t("sidebar.ungrouped") : group.name;
}

function profileStatus(profile) {
  return props.profileStatuses[profile.id] ?? "notOpen";
}

function profileStatusLabel(profile) {
  const status = profileStatus(profile);
  if (status === "notOpen") return t("workspace.notOpen");

  return t(`monitor.status.${status}`);
}
</script>

<template>
  <section class="connection-hub" :class="{ empty: !hasProfiles }">
    <template v-if="hasProfiles">
      <header class="connection-hub-header">
        <div class="connection-hub-title">
          <span>{{ t("workspace.connectionHub") }}</span>
          <small>{{ t("workspace.connectionCount", { count: allProfiles.length }) }}</small>
        </div>

        <div class="connection-hub-actions">
          <UInput
            v-model="query"
            class="connection-hub-search"
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-lucide-search"
            :placeholder="t('sidebar.search')"
          />
          <UButton
            color="primary"
            variant="solid"
            size="sm"
            icon="i-lucide-plus"
            :label="t('workspace.addConnectionTitle')"
            @click="emit('createConnection')"
          />
        </div>
      </header>

      <div v-if="filteredGroups.length > 0" class="connection-hub-body">
        <section v-for="{ group, profiles } in filteredGroups" :key="group.id" class="connection-hub-group">
          <div class="connection-hub-group-title">
            <span>{{ groupDisplayName(group) }}</span>
            <small>{{ profiles.length }}</small>
          </div>

          <div class="connection-card-grid">
            <article
              v-for="profile in profiles"
              :key="profile.id"
              class="connection-card"
              :class="profileStatus(profile)"
            >
              <button type="button" class="connection-card-main" @click="emit('openProfile', profile)">
                <span class="connection-card-icon">
                  <UIcon name="i-lucide-server" />
                </span>

                <span class="connection-card-copy">
                  <strong>{{ profile.name }}</strong>
                  <small>{{ profile.username }}@{{ profile.host }}:{{ profile.port }}</small>
                  <em>{{ profile.defaultRemotePath || "/" }}</em>
                </span>

                <span class="connection-card-status">
                  <i></i>
                  <span>{{ profileStatusLabel(profile) }}</span>
                </span>
              </button>

              <UButton
                class="connection-card-edit"
                color="neutral"
                variant="ghost"
                size="xs"
                icon="i-lucide-pencil"
                :aria-label="t('sidebar.editConnection')"
                @click="emit('editProfile', profile.id)"
              />
            </article>
          </div>
        </section>
      </div>

      <div v-else class="connection-hub-no-results">
        <UIcon name="i-lucide-search-x" />
        <span>{{ t("sidebar.noMatches") }}</span>
      </div>
    </template>

    <div v-else class="connection-hub-empty">
      <div class="empty-icon">
        <UIcon name="i-lucide-plus" />
      </div>
      <h2>{{ t("workspace.addConnectionTitle") }}</h2>
      <UButton
        color="primary"
        variant="solid"
        size="lg"
        icon="i-lucide-plus"
        :label="t('workspace.addConnectionTitle')"
        @click="emit('createConnection')"
      />
    </div>
  </section>
</template>

<style scoped src="../../assets/css/workspace-hub.css"></style>
