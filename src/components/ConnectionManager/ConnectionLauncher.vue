<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useUiStore } from "../../stores/uiStore";

const { t } = useI18n();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const uiStore = useUiStore();
const query = ref("");
const launcher = reactive({
  isOpen: false,
});

const activeSession = computed(() => sessionStore.activeSession);
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

function groupDisplayName(group) {
  return group.name === "Ungrouped" ? t("sidebar.ungrouped") : group.name;
}

function toggleLauncher() {
  launcher.isOpen = !launcher.isOpen;
}

function closeLauncher() {
  launcher.isOpen = false;
}

function openProfile(profile) {
  sessionStore.openProfileSession(profile);
  closeLauncher();
}

function editProfile(profile) {
  closeLauncher();
  uiStore.openConnectionForm(profile.id);
}

function openConnectionForm() {
  closeLauncher();
  uiStore.openConnectionForm();
}

function handleDocumentClick(event) {
  if (!event.target.closest?.(".connection-launcher")) {
    closeLauncher();
  }
}

onMounted(() => {
  document.addEventListener("click", handleDocumentClick);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", handleDocumentClick);
});
</script>

<template>
  <div class="connection-launcher">
    <button type="button" class="server-picker-button" @click.stop="toggleLauncher">
      <UIcon name="i-lucide-folder-open" />
      <span>
        <strong>{{ activeSession?.name ?? t("connectionLauncher.pickServer") }}</strong>
        <small>{{ activeSession ? `${activeSession.username}@${activeSession.host}` : t("connectionLauncher.noServer") }}</small>
      </span>
      <UIcon name="i-lucide-chevron-down" />
    </button>

    <section v-if="launcher.isOpen" class="server-picker-popover" @click.stop>
      <header>
        <label class="server-search">
          <UIcon name="i-lucide-search" />
          <input v-model="query" type="search" :placeholder="t('sidebar.search')" autofocus />
        </label>
        <button type="button" :aria-label="t('sidebar.add')" @click="openConnectionForm">
          <UIcon name="i-lucide-plus" />
        </button>
      </header>

      <div class="server-group-list">
        <section v-for="{ group, profiles } in filteredGroups" :key="group.id" class="server-group">
          <p class="server-group-title">
            <span>{{ groupDisplayName(group) }}</span>
            <small>{{ profiles.length }}</small>
          </p>

          <div
            v-for="profile in profiles"
            :key="profile.id"
            class="server-option"
            :class="{ active: activeSession?.profileId === profile.id }"
          >
            <button type="button" class="server-option-main" @click="openProfile(profile)">
              <span class="server-option-icon">
                <UIcon name="i-lucide-server" />
              </span>
              <span class="server-option-copy">
                <strong>{{ profile.name }}</strong>
                <small>{{ profile.username }}@{{ profile.host }}:{{ profile.port }}</small>
              </span>
            </button>
            <button
              type="button"
              class="server-option-edit"
              :aria-label="t('sidebar.editConnection')"
              @click="editProfile(profile)"
            >
              <UIcon name="i-lucide-pencil" />
            </button>
          </div>

          <p v-if="profiles.length === 0" class="server-empty">{{ t("sidebar.empty") }}</p>
        </section>
      </div>
    </section>
  </div>
</template>
