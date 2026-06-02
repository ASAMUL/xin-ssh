<script setup>
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useProfileStore } from "../../stores/profileStore";
import { useUiStore } from "../../stores/uiStore";

const { t } = useI18n();
const profileStore = useProfileStore();
const uiStore = useUiStore();
const query = ref("");
const newGroupName = ref("");
const isGroupCreatorOpen = ref(false);
const savingGroupId = ref(null);
const isSavingGroup = ref(false);
const deletingProfileId = ref(null);
const profileMenu = reactive({
  isOpen: false,
  x: 0,
  y: 0,
  profile: null,
});

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

function groupToggleLabel(group) {
  return t(group.collapsed ? "sidebar.expandGroup" : "sidebar.collapseGroup", {
    name: groupDisplayName(group),
  });
}

function groupIconName(group) {
  if (savingGroupId.value === group.id) {
    return "i-lucide-loader-circle";
  }

  return group.collapsed ? "i-lucide-chevron-right" : "i-lucide-chevron-down";
}

function openGroupCreator() {
  closeProfileMenu();
  isGroupCreatorOpen.value = true;
}

function closeGroupCreator() {
  isGroupCreatorOpen.value = false;
  newGroupName.value = "";
}

async function createGroup() {
  const name = newGroupName.value.trim();
  if (!name || isSavingGroup.value) {
    return;
  }

  const now = Date.now().toString();
  const groupId = crypto.randomUUID();

  isSavingGroup.value = true;
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
    isSavingGroup.value = false;
  }
}

async function toggleGroupCollapsed(group) {
  if (savingGroupId.value === group.id) return;

  savingGroupId.value = group.id;
  uiStore.clearError();
  try {
    await profileStore.saveGroup({
      ...group,
      collapsed: !group.collapsed,
    });
  } catch (error) {
    uiStore.setError(`${t("sidebar.updateGroupFailed")}: ${String(error)}`);
  } finally {
    savingGroupId.value = null;
  }
}

function openProfileMenu(event, profile) {
  isGroupCreatorOpen.value = false;
  profileMenu.isOpen = true;
  profileMenu.x = Math.max(8, Math.min(event.clientX, window.innerWidth - 184));
  profileMenu.y = Math.max(8, Math.min(event.clientY, window.innerHeight - 88));
  profileMenu.profile = profile;
}

function closeProfileMenu() {
  profileMenu.isOpen = false;
  profileMenu.profile = null;
}

function editProfileFromMenu() {
  if (!profileMenu.profile) return;

  const profileId = profileMenu.profile.id;
  closeProfileMenu();
  uiStore.openConnectionForm(profileId);
}

async function deleteProfileFromMenu() {
  const profile = profileMenu.profile;
  if (!profile || deletingProfileId.value) return;

  if (!window.confirm(t("sidebar.deleteConnectionConfirm", { name: profile.name }))) {
    closeProfileMenu();
    return;
  }

  deletingProfileId.value = profile.id;
  uiStore.clearError();
  try {
    await profileStore.deleteProfile(profile.id);
    closeProfileMenu();
  } catch (error) {
    uiStore.setError(`${t("sidebar.deleteConnectionFailed")}: ${String(error)}`);
  } finally {
    deletingProfileId.value = null;
  }
}
</script>

<template>
  <aside class="connection-sidebar" @click="closeProfileMenu">
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
      <div class="section-actions">
        <button type="button" :aria-label="t('sidebar.addGroup')" @click="openGroupCreator">
          <UIcon name="i-lucide-folder-plus" />
        </button>
        <button type="button" :aria-label="t('sidebar.add')" @click="uiStore.openConnectionForm()">
          <UIcon name="i-lucide-plus" />
        </button>
      </div>
    </div>

    <div class="group-list">
      <form v-if="isGroupCreatorOpen" class="group-creator" @submit.prevent="createGroup" @click.stop>
        <input
          v-model="newGroupName"
          type="text"
          :placeholder="t('sidebar.groupName')"
          autofocus
        />
        <button type="submit" :aria-label="t('sidebar.saveGroup')" :disabled="!newGroupName.trim() || isSavingGroup">
          <UIcon
            :name="isSavingGroup ? 'i-lucide-loader-circle' : 'i-lucide-check'"
            :class="{ 'spin-icon': isSavingGroup }"
          />
        </button>
        <button type="button" :aria-label="t('sidebar.cancelGroup')" @click="closeGroupCreator">
          <UIcon name="i-lucide-x" />
        </button>
      </form>

      <section v-for="{ group, profiles } in filteredGroups" :key="group.id" class="connection-group">
        <button
          type="button"
          class="group-heading"
          :aria-label="groupToggleLabel(group)"
          :disabled="savingGroupId === group.id"
          @click="toggleGroupCollapsed(group)"
        >
          <UIcon
            :name="groupIconName(group)"
            :class="{ 'spin-icon': savingGroupId === group.id }"
          />
          <span>{{ groupDisplayName(group) }}</span>
          <small>{{ profiles.length }}</small>
        </button>

        <div v-if="!group.collapsed" class="connection-list">
          <button
            v-for="profile in profiles"
            :key="profile.id"
            type="button"
            class="connection-item"
            :class="{ deleting: deletingProfileId === profile.id }"
            @contextmenu.prevent="openProfileMenu($event, profile)"
          >
            <span class="status-dot disconnected"></span>
            <span class="connection-icon">
              <UIcon
                :name="deletingProfileId === profile.id ? 'i-lucide-loader-circle' : 'i-lucide-server'"
                :class="{ 'spin-icon': deletingProfileId === profile.id }"
              />
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

    <div
      v-if="profileMenu.isOpen"
      class="connection-context-menu"
      :style="{ left: `${profileMenu.x}px`, top: `${profileMenu.y}px` }"
      @click.stop
    >
      <button type="button" @click="editProfileFromMenu">
        <UIcon name="i-lucide-pencil" />
        <span>{{ t("sidebar.editConnection") }}</span>
      </button>
      <button type="button" class="danger" :disabled="Boolean(deletingProfileId)" @click="deleteProfileFromMenu">
        <UIcon name="i-lucide-trash-2" />
        <span>{{ t("sidebar.deleteConnection") }}</span>
      </button>
    </div>
  </aside>
</template>
