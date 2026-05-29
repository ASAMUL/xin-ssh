<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useProfileStore } from "../../stores/profileStore";
import { useUiStore } from "../../stores/uiStore";

const { t } = useI18n();
const profileStore = useProfileStore();
const uiStore = useUiStore();
const query = ref("");

const filteredGroups = computed(() => {
  const normalizedQuery = query.value.trim().toLowerCase();

  return profileStore.profilesByGroup
    .map(({ group, profiles }) => ({
      group,
      profiles: normalizedQuery
        ? profiles.filter((profile) =>
            [profile.name, profile.host, profile.username].some((value) =>
              value.toLowerCase().includes(normalizedQuery),
            ),
          )
        : profiles,
    }))
    .filter(({ profiles }) => !normalizedQuery || profiles.length > 0);
});
</script>

<template>
  <aside class="connection-sidebar">
    <div class="brand-row">
      <div class="brand-mark">
        <UIcon name="i-lucide-terminal" />
      </div>
      <div class="brand-title">
        <span>{{ t("app.name") }}</span>
        <small>{{ t("app.pro") }}</small>
      </div>
    </div>

    <label class="search-field">
      <UIcon name="i-lucide-search" />
      <input v-model="query" type="search" :placeholder="t('sidebar.search')" />
      <kbd>⌘ K</kbd>
    </label>

    <div class="section-heading">
      <span>{{ t("sidebar.connections") }}</span>
      <button type="button" :aria-label="t('sidebar.add')" @click="uiStore.openConnectionForm">
        <UIcon name="i-lucide-plus" />
      </button>
    </div>

    <div class="group-list">
      <section v-for="{ group, profiles } in filteredGroups" :key="group.id" class="connection-group">
        <button type="button" class="group-heading">
          <UIcon :name="group.collapsed ? 'i-lucide-chevron-right' : 'i-lucide-chevron-down'" />
          <span>{{ group.name === 'Ungrouped' ? t('sidebar.ungrouped') : group.name }}</span>
          <small>{{ profiles.length }}</small>
        </button>

        <div v-if="!group.collapsed" class="connection-list">
          <button v-for="profile in profiles" :key="profile.id" type="button" class="connection-item">
            <span class="status-dot disconnected"></span>
            <span class="connection-icon">
              <UIcon name="i-lucide-server" />
            </span>
            <span class="connection-copy">
              <strong>{{ profile.name }}</strong>
              <small>{{ profile.username }}@{{ profile.host }}</small>
            </span>
          </button>

          <p v-if="profiles.length === 0" class="empty-group">{{ t("sidebar.empty") }}</p>
        </div>
      </section>
    </div>

    <div class="sidebar-footer">
      <button type="button" :aria-label="t('app.settings')" @click="uiStore.openSettings">
        <UIcon name="i-lucide-settings" />
      </button>
    </div>
  </aside>
</template>
