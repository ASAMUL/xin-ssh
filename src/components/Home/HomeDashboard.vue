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
import HomeSessionCard from "./HomeSessionCard.vue";
import { useHomeSortable } from "../../composables/useHomeSortable";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { localizeError } from "../../utils/appError";
import { exportProfiles, readSshConfig } from "../../services/workspaceApi";

// 默认分组是「未分组」兜底位：不能改名、不能删，也不在卡片上显示分组标签
const DEFAULT_GROUP_ID = "group-default";
// USelect 的值不能为空，用哨兵代表「不按分组筛选」；分组 id 是 UUID，不会撞上
const ALL_GROUPS = "all";

const { t } = useI18n();
const toast = useToast();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();

const query = ref("");
const cardGrid = ref(null);
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

const activeNav = computed({
  get: () => uiStore.homeNav,
  set: (nav) => uiStore.setHomeNav(nav),
});
const activeGroupFilter = computed({
  get: () => uiStore.homeGroupFilter ?? ALL_GROUPS,
  set: (value) => uiStore.setHomeGroupFilter(value === ALL_GROUPS ? null : value),
});
const viewMode = computed(() => settingsStore.homeViewMode);
const hasProfiles = computed(() => profileStore.activeProfiles.length > 0);
const profileStatuses = computed(() => sessionStore.statusByProfile);
const normalizedQuery = computed(() => query.value.trim().toLowerCase());

const navItems = computed(() => [
  { value: "all", label: t("home.filter.all") },
  { value: "favorites", label: t("home.filter.favorites") },
  { value: "recents", label: t("home.filter.recents") },
]);

// profilesByGroup 已把分组被删的孤儿连接归进默认分组，筛选和标签都沿用这套归属
const profileGroupIds = computed(() => new Map(profileStore.profilesByGroup.flatMap(
  ({ group, profiles }) => profiles.map((profile) => [profile.id, group.id]),
)));

const groupNames = computed(() => Object.fromEntries(
  profileStore.profilesByGroup.map(({ group }) => [group.id, groupDisplayName(group)]),
));

const groupItems = computed(() => [
  { value: ALL_GROUPS, label: t("home.allGroups") },
  ...profileStore.profilesByGroup.map(({ group }) => ({ value: group.id, label: groupDisplayName(group) })),
]);

const filteredProfiles = computed(() => {
  const base = activeNav.value === "favorites" ? profileStore.favoriteProfiles
    : activeNav.value === "recents" ? profileStore.recentProfiles
      : profileStore.activeProfiles;
  const groupId = uiStore.homeGroupFilter;

  return base.filter((profile) => (!groupId || profileGroupIds.value.get(profile.id) === groupId)
    && (!normalizedQuery.value || matchesProfile(profile, normalizedQuery.value)));
});

// 当前会话卡只在它的连接落在筛选结果里时出现：筛到别的分组或搜别的机器时不硬塞进来
const featuredSession = computed(() => sessionStore.workspaceSession);
const featuredProfile = computed(() => {
  const session = featuredSession.value;
  return session ? filteredProfiles.value.find((profile) => profile.id === session.profileId) ?? null : null;
});
// 已经在墨色卡里展示的连接不在下方网格重复出现
const listedProfiles = computed(() => featuredProfile.value
  ? filteredProfiles.value.filter((profile) => profile.id !== featuredProfile.value.id)
  : filteredProfiles.value);

const profileMenus = computed(() =>
  Object.fromEntries(profileStore.activeProfiles.map((profile) => [profile.id, menuItemsFor(profile)])),
);

const tableRows = computed(() =>
  listedProfiles.value.map((profile) => ({
    profile,
    groupName: cardGroupName(profile),
    status: profileStatus(profile),
    menuItems: profileMenus.value[profile.id],
  })),
);

const emptyText = computed(() => {
  if (normalizedQuery.value) return t("sidebar.noMatches");
  if (activeNav.value === "favorites") return t("home.emptyFavorites");
  if (activeNav.value === "recents") return t("home.emptyRecents");
  return t("home.emptyGroup");
});

// 平铺网格里只有「全部 + 选中单个分组 + 未搜索」时顺序才等于该分组的真实顺序，
// 其余视图是跨组或筛过的子集，拖动结果无法落回某个分组，干脆不给拖。
const sortableGroupId = computed(() =>
  activeNav.value === "all" && !normalizedQuery.value ? uiStore.homeGroupFilter : null,
);

useHomeSortable(cardGrid, {
  draggable: ".connection-card",
  handle: ".connection-card-drag-handle",
  disabled: () => !sortableGroupId.value || profileStore.isSavingLayout,
  onDrop: ({ id, beforeId }) => moveProfile({ id, groupId: sortableGroupId.value, beforeId }),
});

const featuredMenuItems = computed(() => {
  const session = featuredSession.value;
  const profile = featuredProfile.value;
  if (!session || !profile) return [];
  return [
    [
      {
        label: t("home.newTerminal"),
        icon: "i-lucide-square-plus",
        onSelect: () => sessionStore.openProfileSession(profile, { forceNew: true }),
      },
      {
        label: t("sidebar.editConnection"),
        icon: "i-lucide-pencil",
        onSelect: () => uiStore.openConnectionForm(profile.id),
      },
    ],
    [
      {
        label: t("terminal.closeSession"),
        icon: "i-lucide-x",
        color: "error",
        onSelect: () => sessionStore.closeSession(session.id),
      },
    ],
  ];
});

const manageableGroups = computed(() => profileStore.activeGroups.filter((group) => group.id !== DEFAULT_GROUP_ID));

// 侧栏去掉后，分组管理、隧道和导入导出统一收进工具栏右端的「更多」菜单
const moreMenuItems = computed(() => [
  [
    {
      label: t("sidebar.addGroup"),
      icon: "i-lucide-folder-plus",
      onSelect: openCreateGroupDialog,
    },
    {
      label: t("home.manageGroups"),
      icon: "i-lucide-folder-cog",
      disabled: !manageableGroups.value.length,
      children: manageableGroups.value.map((group) => ({
        label: groupDisplayName(group),
        icon: "i-lucide-folder",
        children: groupMenuItems(group),
      })),
    },
  ],
  [
    {
      label: t("tunnel.title"),
      icon: "i-lucide-waypoints",
      onSelect: () => uiStore.openTunnelPanel(),
    },
  ],
  [
    {
      label: t("home.importSshConfig"),
      icon: "i-lucide-file-input",
      disabled: isImportingSshConfig.value,
      onSelect: importSshConfig,
    },
    {
      label: t("home.exportProfiles"),
      icon: "i-lucide-file-output",
      disabled: isExportingProfiles.value || !hasProfiles.value,
      onSelect: exportConnectionProfiles,
    },
  ],
]);

function matchesProfile(profile, keyword) {
  return [profile.name, profile.host, profile.username, profile.defaultRemotePath].some(
    (value) => String(value ?? "").toLowerCase().includes(keyword),
  );
}

function groupDisplayName(group) {
  return group.id === DEFAULT_GROUP_ID ? t("sidebar.ungrouped") : group.name;
}

function cardGroupName(profile) {
  const groupId = profileGroupIds.value.get(profile.id);
  return groupId && groupId !== DEFAULT_GROUP_ID ? groupNames.value[groupId] ?? null : null;
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

// 默认分组固定置顶，普通分组上移不能越过它（索引 1 即顶）
function groupMenuItems(group) {
  const groups = profileStore.activeGroups;
  const index = groups.findIndex((item) => item.id === group.id);
  const floor = groups[0]?.id === DEFAULT_GROUP_ID ? 1 : 0;
  return [
    [
      {
        label: t("sidebar.renameGroup"),
        icon: "i-lucide-pencil",
        onSelect: () => openRenameGroupDialog(group),
      },
    ],
    [
      {
        label: t("home.moveUp"),
        icon: "i-lucide-arrow-up",
        disabled: profileStore.isSavingLayout || index <= floor,
        onSelect: () => moveGroup(index, -1),
      },
      {
        label: t("home.moveDown"),
        icon: "i-lucide-arrow-down",
        disabled: profileStore.isSavingLayout || index === groups.length - 1,
        onSelect: () => moveGroup(index, 1),
      },
    ],
    [
      {
        label: t("sidebar.deleteGroup"),
        icon: "i-lucide-trash-2",
        color: "error",
        onSelect: () => openDeleteGroupDialog(group),
      },
    ],
  ];
}

function moveGroup(index, offset) {
  const ids = profileStore.activeGroups.map((group) => group.id);
  const [id] = ids.splice(index, 1);
  ids.splice(index + offset, 0, id);
  profileStore.reorderGroups(ids).catch(showError);
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
    <div class="home-sheet">
      <header class="home-header">
        <div class="home-title">
          <h1>{{ t("home.title") }}</h1>
          <span>{{ t("home.hostCount", { count: profileStore.activeProfiles.length }) }}</span>
        </div>
        <UButton
          v-liquid-glass="{ scale: 16 }"
          class="home-create liquid-glass liquid-glass--ink"
          color="primary"
          size="lg"
          icon="i-lucide-plus"
          :label="t('sidebar.add')"
          @click="uiStore.openConnectionForm()"
        />
      </header>

      <div class="home-scroll">
        <!-- 工具栏吸顶、浮在列表之上：卡片从玻璃底下滑过时，边缘的折射才看得见 -->
        <div class="home-toolbar">
          <UInput
            v-model="query"
            v-liquid-glass
            class="home-search liquid-glass"
            color="neutral"
            variant="none"
            size="lg"
            icon="i-lucide-search"
            :placeholder="t('home.searchPlaceholder')"
          />

          <UTabs
            v-model="activeNav"
            v-liquid-glass
            class="home-filter liquid-glass"
            variant="pill"
            :items="navItems"
            :content="false"
            :ui="{
              list: 'home-segment-list',
              indicator: 'home-segment-indicator',
              trigger: 'home-segment-trigger',
            }"
          />

          <!-- USelect 渲染的是片段（触发器 + 弹层 portal），指令挂不上去，玻璃做在外面这层 -->
          <div v-liquid-glass class="home-group-select liquid-glass">
            <USelect
              v-model="activeGroupFilter"
              class="home-group-trigger"
              color="neutral"
              variant="none"
              size="lg"
              icon="i-lucide-folder"
              :items="groupItems"
              :aria-label="t('home.groupFilter')"
              :content="{ align: 'start' }"
            />
          </div>

          <UFieldGroup
            v-liquid-glass
            class="home-view-toggle liquid-glass"
            role="group"
            :aria-label="t('home.viewMode')"
          >
            <UButton
              :class="{ active: viewMode === 'grid' }"
              color="neutral"
              variant="ghost"
              size="lg"
              square
              icon="i-lucide-layout-grid"
              :aria-pressed="viewMode === 'grid'"
              :aria-label="t('home.viewGrid')"
              :title="t('home.viewGrid')"
              @click="settingsStore.setHomeViewMode('grid')"
            />
            <UButton
              :class="{ active: viewMode === 'list' }"
              color="neutral"
              variant="ghost"
              size="lg"
              square
              icon="i-lucide-list"
              :aria-pressed="viewMode === 'list'"
              :aria-label="t('home.viewList')"
              :title="t('home.viewList')"
              @click="settingsStore.setHomeViewMode('list')"
            />
          </UFieldGroup>

          <UDropdownMenu :items="moreMenuItems" size="sm" :content="{ align: 'end' }">
            <UButton
              v-liquid-glass
              class="home-more liquid-glass"
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

        <div v-if="!hasProfiles" class="home-empty">
          <div class="home-empty-icon">
            <UIcon name="i-lucide-server" />
          </div>
          <h3>{{ t("home.emptyTitle") }}</h3>
          <p>{{ t("home.emptyDescription") }}</p>
          <UButton
            class="cta-button"
            color="primary"
            size="lg"
            icon="i-lucide-plus"
            :label="t('sidebar.add')"
            @click="uiStore.openConnectionForm()"
          />
        </div>

        <template v-else>
          <HomeSessionCard
            v-if="featuredSession && featuredProfile"
            :session="featuredSession"
            :profile="featuredProfile"
            :menu-items="featuredMenuItems"
            @resume="sessionStore.selectSession"
          />

          <p v-if="!listedProfiles.length && !featuredProfile" class="home-section-empty">{{ emptyText }}</p>

          <div
            v-else-if="viewMode === 'grid'"
            ref="cardGrid"
            class="home-card-grid"
            :data-group-id="sortableGroupId"
            :aria-busy="profileStore.isSavingLayout"
          >
            <ConnectionCard
              v-for="profile in listedProfiles"
              :key="profile.id"
              :data-sort-id="profile.id"
              :profile="profile"
              :status="profileStatus(profile)"
              :group-name="cardGroupName(profile)"
              :menu-items="profileMenus[profile.id]"
              :sortable="Boolean(sortableGroupId)"
              @open="openProfile"
              @toggle-favorite="toggleFavorite"
            />
          </div>

          <ConnectionTable
            v-else-if="tableRows.length"
            :rows="tableRows"
            @open="openProfile"
            @edit="uiStore.openConnectionForm"
            @toggle-favorite="toggleFavorite"
          />
        </template>
      </div>
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
