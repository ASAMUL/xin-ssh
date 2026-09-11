<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@nuxt/ui/composables";
import { open, save } from "@tauri-apps/plugin-dialog";
import ConnectionCard from "./ConnectionCard.vue";
import ConnectionTable from "./ConnectionTable.vue";
import DeleteConnectionDialog from "./DeleteConnectionDialog.vue";
import DeleteGroupDialog from "./DeleteGroupDialog.vue";
import GroupNameDialog from "./GroupNameDialog.vue";
import HomeSidebar from "./HomeSidebar.vue";
import HomeConnectionGroup from "./HomeConnectionGroup.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { localizeError } from "../../utils/appError";
import { exportProfiles, readSshConfig } from "../../services/workspaceApi";

const RECENT_ROW_LIMIT = 5;

const { t } = useI18n();
const toast = useToast();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();

const query = ref("");
const isDraggingProfiles = ref(false);
const groupDialogOpen = ref(false);
const groupDialogMode = ref("create");
const groupDialogInitialName = ref("");
const pendingGroupId = ref(null);

const deleteConnectionDialogOpen = ref(false);
const pendingDeleteProfile = ref(null);

const deleteGroupDialogOpen = ref(false);
const pendingDeleteGroup = ref(null);
const isImportingSshConfig = ref(false);
const isExportingProfiles = ref(false);

const greeting = computed(() => {
  const hour = new Date().getHours();
  if (hour >= 5 && hour < 12) return t("home.greeting.morning");
  if (hour >= 12 && hour < 18) return t("home.greeting.afternoon");
  return t("home.greeting.evening");
});

const profileStatuses = computed(() => sessionStore.statusByProfile);

const activeNav = computed(() => uiStore.homeNav);
const activeGroupId = computed(() => uiStore.homeGroupFilter);
const viewMode = computed(() => settingsStore.homeViewMode);
const hasProfiles = computed(() => profileStore.activeProfiles.length > 0);

const navItems = computed(() => [
  { id: "home", icon: "i-lucide-house", label: t("home.nav.home") },
  {
    id: "all",
    icon: "i-lucide-folder-open",
    label: t("home.nav.all"),
    count: profileStore.activeProfiles.length,
  },
  {
    id: "favorites",
    icon: "i-lucide-star",
    label: t("home.nav.favorites"),
    count: profileStore.favoriteProfiles.length,
  },
  {
    id: "recents",
    icon: "i-lucide-clock",
    label: t("home.nav.recents"),
    count: profileStore.recentProfiles.length,
  },
]);

const sidebarGroups = computed(() =>
  profileStore.profilesByGroup.map(({ group, profiles }) => ({
    group,
    name: groupDisplayName(group),
    count: profiles.length,
  })),
);

const groupNames = computed(() =>
  Object.fromEntries(sidebarGroups.value.map(({ group, name }) => [group.id, name])),
);

const normalizedQuery = computed(() => query.value.trim().toLowerCase());

// 当前导航下的扁平连接列表（收藏 / 最近直接用对应 getter，主页与全部再叠加分组筛选）
const visibleProfiles = computed(() => {
  let profiles;
  if (activeNav.value === "favorites") {
    profiles = profileStore.favoriteProfiles;
  } else if (activeNav.value === "recents") {
    profiles = profileStore.recentProfiles;
  } else {
    profiles = profileStore.activeProfiles;
    if (activeGroupId.value) {
      const grouped = profileStore.profilesByGroup.find(
        ({ group }) => group.id === activeGroupId.value,
      );
      profiles = grouped?.profiles ?? [];
    }
  }

  return normalizedQuery.value
    ? profiles.filter((profile) => matchesProfile(profile, normalizedQuery.value))
    : profiles;
});

// 卡片视图在主页/全部按分组分区展示；收藏、最近为扁平网格
const groupedView = computed(() => ["home", "all"].includes(activeNav.value));
const visibleGroups = computed(() =>
  profileStore.profilesByGroup
    .filter(({ group }) => !activeGroupId.value || group.id === activeGroupId.value)
    .map(({ group, profiles }) => ({
      group,
      name: groupDisplayName(group),
      profiles: normalizedQuery.value
        ? profiles.filter((profile) => matchesProfile(profile, normalizedQuery.value))
        : profiles,
    }))
    .filter(({ profiles }) => !normalizedQuery.value || profiles.length > 0),
);

const profileMenus = computed(() =>
  Object.fromEntries(profileStore.activeProfiles.map((profile) => [profile.id, menuItemsFor(profile)])),
);

const tableRows = computed(() =>
  visibleProfiles.value.map((profile) => ({
    profile,
    groupName: groupNames.value[profile.groupId] ?? t("sidebar.ungrouped"),
    status: profileStatus(profile),
    menuItems: profileMenus.value[profile.id],
  })),
);

const recentRow = computed(() => profileStore.recentProfiles.slice(0, RECENT_ROW_LIMIT));
const showHomeSections = computed(() => activeNav.value === "home" && !normalizedQuery.value);

const sectionTitle = computed(() => {
  if (activeNav.value === "favorites") return t("home.nav.favorites");
  if (activeNav.value === "recents") return t("home.nav.recents");
  return t("home.nav.all");
});

const sectionEmptyText = computed(() => {
  if (!hasProfiles.value || normalizedQuery.value) return t("sidebar.noMatches");
  if (activeNav.value === "favorites") return t("home.emptyFavorites");
  if (activeNav.value === "recents") return t("home.emptyRecents");
  return t("sidebar.noMatches");
});

const quickActions = computed(() => [
  {
    id: "new",
    icon: "i-lucide-plus",
    tone: "blue",
    title: t("home.quick.newConnection"),
    description: t("home.quick.newConnectionDesc"),
    disabled: false,
    run: () => uiStore.openConnectionForm(),
  },
  {
    id: "tunnel",
    icon: "i-lucide-waypoints",
    tone: "green",
    title: t("home.quick.tunnel"),
    description: t("home.quick.tunnelDesc"),
    disabled: false,
    run: () => uiStore.openTunnelPanel(),
  },
  {
    id: "favorites",
    icon: "i-lucide-star",
    tone: "amber",
    title: t("home.quick.favorites"),
    description: t("home.quick.favoritesDesc", {
      count: profileStore.favoriteProfiles.length,
    }),
    disabled: false,
    run: () => uiStore.setHomeNav("favorites"),
  },
  {
    id: "settings",
    icon: "i-lucide-settings",
    tone: "purple",
    title: t("home.quick.settings"),
    description: t("home.quick.settingsDesc"),
    disabled: false,
    run: () => uiStore.openSettings(),
  },
]);

function matchesProfile(profile, keyword) {
  return [profile.name, profile.host, profile.username, profile.defaultRemotePath].some(
    (value) => String(value ?? "").toLowerCase().includes(keyword),
  );
}

function groupDisplayName(group) {
  return group.id === "group-default" ? t("sidebar.ungrouped") : group.name;
}

function profileStatus(profile) {
  return profileStatuses.value[profile.id] ?? "notOpen";
}

// 卡片、表格和右键共用操作定义，避免某个入口遗漏分组移动或删除确认。
function menuItemsFor(profile) {
  const siblings = profileStore.profilesInGroup(profile.groupId);
  const index = siblings.findIndex((item) => item.id === profile.id);
  return [
    [
      {
        label: t("sidebar.editConnection"),
        icon: "i-lucide-pencil",
        onSelect: () => uiStore.openConnectionForm(profile.id),
      },
      {
        label: t("home.moveToGroup"),
        icon: "i-lucide-folder-input",
        disabled: profileStore.isSavingLayout,
        children: profileStore.activeGroups.map((group) => ({
          label: groupDisplayName(group),
          icon: group.id === profile.groupId ? "i-lucide-check" : "i-lucide-folder",
          disabled: group.id === profile.groupId,
          onSelect: () => moveProfile({ id: profile.id, groupId: group.id }),
        })),
      },
    ],
    [
      {
        label: t("home.moveUp"),
        icon: "i-lucide-arrow-up",
        disabled: profileStore.isSavingLayout || index <= 0,
        onSelect: () => moveProfile({
          id: profile.id,
          groupId: profile.groupId,
          beforeId: siblings[index - 1].id,
        }),
      },
      {
        label: t("home.moveDown"),
        icon: "i-lucide-arrow-down",
        disabled: profileStore.isSavingLayout || index < 0 || index === siblings.length - 1,
        onSelect: () => moveProfile({
          id: profile.id,
          groupId: profile.groupId,
          beforeId: siblings[index + 2]?.id,
        }),
      },
    ],
    [
      {
        label: t("sidebar.deleteConnection"),
        icon: "i-lucide-trash-2",
        color: "error",
        onSelect: () => openDeleteConnectionDialog(profile),
      },
    ],
  ];
}

function openProfile(profile) {
  sessionStore.openProfileSession(profile);
}

async function importSshConfig() {
  if (isImportingSshConfig.value) return;
  const selected = await open({
    multiple: false,
    directory: false,
    title: t("home.chooseSshConfig"),
    filters: [{ name: "SSH config", extensions: ["config", "conf", "txt"] }],
  });
  if (typeof selected !== "string") return;

  isImportingSshConfig.value = true;
  try {
    const entries = await readSshConfig(selected);
    if (!entries.length) {
      toast.add({ title: t("home.sshConfigEmpty"), color: "warning" });
      return;
    }
    const imported = await profileStore.importSshConfigEntries(entries);
    toast.add({
      title: t("home.sshConfigImported", { count: imported }),
      icon: "i-lucide-check",
      color: "success",
    });
  } catch (error) {
    toast.add({
      title: t("home.sshConfigImportFailed"),
      description: localizeError(error),
      color: "error",
    });
  } finally {
    isImportingSshConfig.value = false;
  }
}

async function exportConnectionProfiles() {
  if (isExportingProfiles.value) return;
  const profiles = profileStore.activeProfiles;
  if (!profiles.length) {
    toast.add({ title: t("home.noProfilesToExport"), color: "warning" });
    return;
  }

  const selected = await save({
    defaultPath: "xin-ssh-connections.json",
    title: t("home.exportProfilesTitle"),
    filters: [{ name: "Xin SSH connections", extensions: ["json"] }],
  });
  if (typeof selected !== "string") return;

  isExportingProfiles.value = true;
  try {
    await exportProfiles(selected, profiles.map((profile) => ({
      name: profile.name,
      host: profile.host,
      port: profile.port,
      username: profile.username,
      authMethod: profile.authMethod,
      privateKeyPath: profile.privateKeyPath ?? null,
      defaultRemotePath: profile.defaultRemotePath || "/",
    })));
    toast.add({
      title: t("home.profilesExported", { count: profiles.length }),
      icon: "i-lucide-check",
      color: "success",
    });
  } catch (error) {
    toast.add({
      title: t("home.profileExportFailed"),
      description: localizeError(error),
      color: "error",
    });
  } finally {
    isExportingProfiles.value = false;
  }
}

function toggleFavorite(profileId) {
  profileStore.toggleFavorite(profileId).catch(showError);
}

function moveProfile({ id, groupId, beforeId = null }) {
  profileStore.moveProfileToGroup(id, groupId, beforeId).catch(showError);
}

function reorderGroups(groupIds) {
  profileStore.reorderGroups(groupIds).catch(showError);
}

function setGroupCollapsed(groupId, collapsed) {
  profileStore.setGroupCollapsed(groupId, collapsed).catch(showError);
}

// ---- 删除连接 ----
function openDeleteConnectionDialog(profile) {
  pendingDeleteProfile.value = profile;
  deleteConnectionDialogOpen.value = true;
}

// 关闭请求失败时保留配置和确认窗口，避免删除后留下无法管理的后台会话。
async function confirmDeleteConnection(profile, { done, fail }) {
  try {
    await sessionStore.closeSessionsForProfile(profile.id);
    await profileStore.deleteProfile(profile.id);
    toast.add({
      title: t("home.connectionDeleted"),
      description: profile.name,
      icon: "i-lucide-check",
      color: "neutral",
      duration: 3000,
    });
    done();
  } catch (deleteError) {
    fail(deleteError);
  }
}

// ---- 分组管理 ----
function openCreateGroupDialog() {
  groupDialogMode.value = "create";
  groupDialogInitialName.value = "";
  pendingGroupId.value = null;
  groupDialogOpen.value = true;
}

function openRenameGroupDialog(group) {
  groupDialogMode.value = "rename";
  groupDialogInitialName.value = groupDisplayName(group);
  pendingGroupId.value = group.id;
  groupDialogOpen.value = true;
}

// 只在持久化成功后关窗，创建和重命名失败时都保留用户输入。
async function submitGroupDialog(name, { done, fail }) {
  try {
    if (groupDialogMode.value === "rename" && pendingGroupId.value) {
      await profileStore.renameGroup(pendingGroupId.value, name);
    } else {
      await profileStore.createGroup(name);
    }
    toast.add({
      title: t(groupDialogMode.value === "rename" ? "home.groupRenamed" : "home.groupCreated"),
      description: name,
      color: "success",
    });
    done();
  } catch (submitError) {
    fail(submitError);
  }
}

// ---- 删除分组 ----
function openDeleteGroupDialog(group) {
  pendingDeleteGroup.value = group;
  deleteGroupDialogOpen.value = true;
}

// 仅在用户选择一并删除连接时关闭会话，普通移组不能打断正在使用的终端。
async function confirmDeleteGroup({ deleteProfiles }, { done, fail }) {
  const group = pendingDeleteGroup.value;
  if (!group) return;

  try {
    if (deleteProfiles) {
      const profileIds = profileStore.profilesInGroup(group.id).map((profile) => profile.id);
      for (const profileId of profileIds) {
        await sessionStore.closeSessionsForProfile(profileId);
      }
    }
    await profileStore.deleteGroup(group.id, { deleteProfiles });
    if (uiStore.homeGroupFilter === group.id) {
      uiStore.setHomeGroupFilter(null);
    }
    toast.add({ title: t("home.groupDeleted"), description: group.name, color: "success" });
    done();
  } catch (deleteError) {
    fail(deleteError);
  }
}

function showError(error) {
  uiStore.setError(error);
}
</script>

<template>
  <section class="home-dashboard">
    <HomeSidebar
      :nav-items="navItems"
      :groups="sidebarGroups"
      :active-nav="activeNav"
      :active-group-id="activeGroupId"
      :busy="profileStore.isSavingLayout"
      @select-nav="uiStore.setHomeNav"
      @select-group="uiStore.setHomeGroupFilter"
      @create-group="openCreateGroupDialog"
      @rename-group="openRenameGroupDialog"
      @delete-group="openDeleteGroupDialog"
      @reorder-groups="reorderGroups"
      @open-settings="uiStore.openSettings"
      @open-tunnels="uiStore.openTunnelPanel"
    />

    <div class="home-main">
      <header class="home-hero">
        <div class="home-hero-copy">
          <h1>
            <span class="home-hero-wave">👋</span>
            {{ greeting }}
          </h1>
          <p>{{ t("home.subtitle") }}</p>
        </div>

        <div class="home-hero-actions">
          <UInput
            v-model="query"
            class="home-search"
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-lucide-search"
            :placeholder="t('sidebar.search')"
          />
          <UButton
            class="cta-button"
            color="primary"
            variant="solid"
            size="sm"
            icon="i-lucide-plus"
            :label="t('workspace.addConnectionTitle')"
            @click="uiStore.openConnectionForm()"
          />
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-lucide-file-input"
            :label="t('home.importSshConfig')"
            :loading="isImportingSshConfig"
            @click="importSshConfig"
          />
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            icon="i-lucide-file-output"
            :label="t('home.exportProfiles')"
            :loading="isExportingProfiles"
            @click="exportConnectionProfiles"
          />
        </div>
      </header>

      <section v-if="showHomeSections" class="home-quick-grid">
        <button
          v-for="action in quickActions"
          :key="action.id"
          type="button"
          class="home-quick-card"
          :disabled="action.disabled"
          @click="action.run"
        >
          <span class="home-quick-icon" :class="action.tone">
            <UIcon :name="action.icon" />
          </span>
          <span class="home-quick-copy">
            <strong>{{ action.title }}</strong>
            <small>{{ action.description }}</small>
          </span>
        </button>
      </section>

      <section v-if="showHomeSections && recentRow.length > 0" class="home-section">
        <header class="home-section-header">
          <h2>{{ t("home.nav.recents") }}</h2>
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            trailing-icon="i-lucide-chevron-right"
            :label="t('monitor.viewAll')"
            @click="uiStore.setHomeNav('recents')"
          />
        </header>
        <div class="home-recent-grid">
          <ConnectionCard
            v-for="profile in recentRow"
            :key="profile.id"
            :profile="profile"
            :status="profileStatus(profile)"
            :menu-items="profileMenus[profile.id]"
            @open="openProfile"
            @edit="uiStore.openConnectionForm"
            @toggle-favorite="toggleFavorite"
          />
        </div>
      </section>

      <section class="home-section">
        <header class="home-section-header">
          <h2>{{ sectionTitle }}</h2>
          <small>{{ t("workspace.connectionCount", { count: visibleProfiles.length }) }}</small>

          <span class="home-view-toggle">
            <UButton
              :class="{ active: viewMode === 'grid' }"
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-lucide-layout-grid"
              :aria-label="t('home.viewGrid')"
              :title="t('home.viewGrid')"
              @click="settingsStore.setHomeViewMode('grid')"
            />
            <UButton
              :class="{ active: viewMode === 'list' }"
              color="neutral"
              variant="ghost"
              size="xs"
              icon="i-lucide-list"
              :aria-label="t('home.viewList')"
              :title="t('home.viewList')"
              @click="settingsStore.setHomeViewMode('list')"
            />
          </span>
        </header>

        <div v-if="!hasProfiles" class="home-empty">
          <div class="home-empty-icon">
            <UIcon name="i-lucide-plus" />
          </div>
          <h3>{{ t("workspace.addConnectionTitle") }}</h3>
          <UButton
            class="cta-button"
            color="primary"
            variant="solid"
            size="lg"
            icon="i-lucide-plus"
            :label="t('workspace.addConnectionTitle')"
            @click="uiStore.openConnectionForm()"
          />
        </div>

        <template v-else-if="viewMode === 'grid'">
          <template v-if="groupedView">
            <template v-if="visibleGroups.length > 0">
              <HomeConnectionGroup
                v-for="{ group, name, profiles } in visibleGroups"
                :key="group.id"
                :group="group"
                :name="name"
                :profiles="profiles"
                :statuses="profileStatuses"
                :menu-items="profileMenus"
                :disabled="profileStore.isSavingLayout || Boolean(normalizedQuery)"
                :force-expanded="isDraggingProfiles || Boolean(normalizedQuery)"
                @open="openProfile"
                @edit="uiStore.openConnectionForm"
                @toggle-favorite="toggleFavorite"
                @collapse="setGroupCollapsed"
                @move="moveProfile"
                @drag-start="isDraggingProfiles = true"
                @drag-end="isDraggingProfiles = false"
              />
            </template>
            <p v-else class="home-section-empty">{{ sectionEmptyText }}</p>
          </template>

          <template v-else>
            <div v-if="visibleProfiles.length > 0" class="home-card-grid">
              <ConnectionCard
                v-for="profile in visibleProfiles"
                :key="profile.id"
                :profile="profile"
                :status="profileStatus(profile)"
                :menu-items="profileMenus[profile.id]"
                @open="openProfile"
                @edit="uiStore.openConnectionForm"
                @toggle-favorite="toggleFavorite"
              />
            </div>
            <p v-else class="home-section-empty">{{ sectionEmptyText }}</p>
          </template>
        </template>

        <template v-else>
          <ConnectionTable
            v-if="tableRows.length > 0"
            :rows="tableRows"
            @open="openProfile"
            @edit="uiStore.openConnectionForm"
            @toggle-favorite="toggleFavorite"
          />
          <p v-else class="home-section-empty">{{ sectionEmptyText }}</p>
        </template>
      </section>
    </div>

    <GroupNameDialog
      :open="groupDialogOpen"
      :mode="groupDialogMode"
      :initial-name="groupDialogInitialName"
      @close="groupDialogOpen = false"
      @submit="submitGroupDialog"
    />

    <DeleteConnectionDialog
      :open="deleteConnectionDialogOpen"
      :profile="pendingDeleteProfile"
      @close="deleteConnectionDialogOpen = false"
      @confirm="confirmDeleteConnection"
    />

    <DeleteGroupDialog
      :open="deleteGroupDialogOpen"
      :group="pendingDeleteGroup"
      @close="deleteGroupDialogOpen = false"
      @confirm="confirmDeleteGroup"
    />
  </section>
</template>

<style scoped src="../../assets/css/home-dashboard.css"></style>
