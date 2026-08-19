<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import ConnectionCard from "./ConnectionCard.vue";
import ConnectionTable from "./ConnectionTable.vue";
import HomeSidebar from "./HomeSidebar.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";

const RECENT_ROW_LIMIT = 5;

const { t } = useI18n();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();

const query = ref("");

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
    .filter(({ profiles }) => profiles.length > 0),
);

const tableRows = computed(() =>
  visibleProfiles.value.map((profile) => ({
    profile,
    groupName: groupNames.value[profile.groupId] ?? t("sidebar.ungrouped"),
    status: profileStatus(profile),
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
  return group.name === "Ungrouped" ? t("sidebar.ungrouped") : group.name;
}

function profileStatus(profile) {
  return profileStatuses.value[profile.id] ?? "notOpen";
}

function openProfile(profile) {
  sessionStore.openProfileSession(profile);
}

function toggleFavorite(profileId) {
  profileStore.toggleFavorite(profileId).catch((error) => uiStore.setError(error));
}
</script>

<template>
  <section class="home-dashboard">
    <HomeSidebar
      :nav-items="navItems"
      :groups="sidebarGroups"
      :active-nav="activeNav"
      :active-group-id="activeGroupId"
      @select-nav="uiStore.setHomeNav"
      @select-group="uiStore.setHomeGroupFilter"
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
              <div v-for="{ group, name, profiles } in visibleGroups" :key="group.id" class="home-group">
                <span class="home-group-title">
                  <span>{{ name }}</span>
                  <small>{{ profiles.length }}</small>
                </span>
                <div class="home-card-grid">
                  <ConnectionCard
                    v-for="profile in profiles"
                    :key="profile.id"
                    :profile="profile"
                    :status="profileStatus(profile)"
                    @open="openProfile"
                    @edit="uiStore.openConnectionForm"
                    @toggle-favorite="toggleFavorite"
                  />
                </div>
              </div>
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
  </section>
</template>

<style scoped src="../../assets/css/home-dashboard.css"></style>
