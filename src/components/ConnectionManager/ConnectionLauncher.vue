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
const DEFAULT_GROUP_ID = "group-default";
const query = ref("");
const launcher = reactive({
  isOpen: false,
});
const groupCreator = reactive({
  isOpen: false,
  name: "",
  isSaving: false,
});
const groupEditor = reactive({
  id: null,
  name: "",
  isSaving: false,
});
const deletingGroupId = ref(null);

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
    .filter(({ group, profiles }) => {
      if (!normalizedQuery) return true;

      return groupDisplayName(group).toLowerCase().includes(normalizedQuery) || profiles.length > 0;
    });
});

function groupDisplayName(group) {
  return group.name === "Ungrouped" ? t("sidebar.ungrouped") : group.name;
}

function toggleLauncher() {
  launcher.isOpen = !launcher.isOpen;
}

function closeLauncher() {
  launcher.isOpen = false;
  closeGroupCreator();
  closeGroupEditor();
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

function canManageGroup(group) {
  return group.id !== DEFAULT_GROUP_ID;
}

function openGroupCreator() {
  closeGroupEditor();
  groupCreator.isOpen = true;
}

function closeGroupCreator() {
  groupCreator.isOpen = false;
  groupCreator.name = "";
}

async function createGroup() {
  const name = groupCreator.name.trim();
  if (!name || groupCreator.isSaving) return;

  const now = Date.now().toString();
  const groupId = crypto.randomUUID();
  groupCreator.isSaving = true;
  uiStore.clearError();

  try {
    await profileStore.saveGroup({
      id: groupId,
      syncId: groupId,
      name,
      sortOrder: profileStore.activeGroups.length,
      collapsed: false,
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    });
    closeGroupCreator();
  } catch (error) {
    uiStore.setError(`${t("sidebar.createGroupFailed")}: ${String(error)}`);
  } finally {
    groupCreator.isSaving = false;
  }
}

function openGroupEditor(group) {
  if (!canManageGroup(group)) return;

  closeGroupCreator();
  groupEditor.id = group.id;
  groupEditor.name = group.name;
}

function closeGroupEditor() {
  groupEditor.id = null;
  groupEditor.name = "";
}

async function saveGroupEditor(group) {
  const name = groupEditor.name.trim();
  if (!name || groupEditor.isSaving) return;

  groupEditor.isSaving = true;
  uiStore.clearError();
  try {
    await profileStore.saveGroup({
      ...group,
      name,
    });
    closeGroupEditor();
  } catch (error) {
    uiStore.setError(`${t("sidebar.updateGroupFailed")}: ${String(error)}`);
  } finally {
    groupEditor.isSaving = false;
  }
}

async function removeGroup(group) {
  if (!canManageGroup(group) || deletingGroupId.value) return;

  if (!window.confirm(t("sidebar.deleteGroupConfirm", { name: groupDisplayName(group) }))) {
    return;
  }

  deletingGroupId.value = group.id;
  uiStore.clearError();
  try {
    await profileStore.deleteGroup(group.id);
  } catch (error) {
    uiStore.setError(`${t("sidebar.deleteGroupFailed")}: ${String(error)}`);
  } finally {
    deletingGroupId.value = null;
  }
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
        <button type="button" :aria-label="t('sidebar.addGroup')" @click="openGroupCreator">
          <UIcon name="i-lucide-folder-plus" />
        </button>
        <button type="button" :aria-label="t('sidebar.add')" @click="openConnectionForm">
          <UIcon name="i-lucide-plus" />
        </button>
      </header>

      <div class="server-group-list">
        <form v-if="groupCreator.isOpen" class="server-group-creator" @submit.prevent="createGroup">
          <UIcon name="i-lucide-folder-plus" />
          <input v-model="groupCreator.name" type="text" :placeholder="t('sidebar.groupName')" autofocus />
          <button type="submit" :disabled="!groupCreator.name.trim() || groupCreator.isSaving">
            <UIcon
              :name="groupCreator.isSaving ? 'i-lucide-loader-circle' : 'i-lucide-check'"
              :class="{ 'spin-icon': groupCreator.isSaving }"
            />
          </button>
          <button type="button" @click="closeGroupCreator">
            <UIcon name="i-lucide-x" />
          </button>
        </form>

        <section v-for="{ group, profiles } in filteredGroups" :key="group.id" class="server-group">
          <form
            v-if="groupEditor.id === group.id"
            class="server-group-editor"
            @submit.prevent="saveGroupEditor(group)"
          >
            <input v-model="groupEditor.name" type="text" autofocus />
            <button type="submit" :disabled="!groupEditor.name.trim() || groupEditor.isSaving">
              <UIcon
                :name="groupEditor.isSaving ? 'i-lucide-loader-circle' : 'i-lucide-check'"
                :class="{ 'spin-icon': groupEditor.isSaving }"
              />
            </button>
            <button type="button" @click="closeGroupEditor">
              <UIcon name="i-lucide-x" />
            </button>
          </form>
          <div v-else class="server-group-title">
            <span>{{ groupDisplayName(group) }}</span>
            <small>{{ profiles.length }}</small>
            <span v-if="canManageGroup(group)" class="server-group-actions">
              <button type="button" :aria-label="t('sidebar.renameGroup')" @click="openGroupEditor(group)">
                <UIcon name="i-lucide-pencil" />
              </button>
              <button
                type="button"
                :aria-label="t('sidebar.deleteGroup')"
                :disabled="deletingGroupId === group.id"
                @click="removeGroup(group)"
              >
                <UIcon
                  :name="deletingGroupId === group.id ? 'i-lucide-loader-circle' : 'i-lucide-trash-2'"
                  :class="{ 'spin-icon': deletingGroupId === group.id }"
                />
              </button>
            </span>
          </div>

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

        <p v-if="filteredGroups.length === 0" class="server-empty">{{ t("sidebar.noMatches") }}</p>
      </div>
    </section>
  </div>
</template>
