import { defineStore } from "pinia";
import {
  deleteGroup,
  deleteProfile,
  markProfileConnected,
  moveProfile,
  reorderGroups,
  retryDeletedCredentialCleanup,
  saveGroup,
  saveProfile,
} from "../services/workspaceApi";

export const useProfileStore = defineStore("profiles", {
  state: () => ({
    groups: [],
    profiles: [],
    isSavingLayout: false,
    isCleaningCredentials: false,
  }),
  getters: {
    activeGroups: (state) =>
      state.groups
        .filter((group) => !group.deletedAt)
        .sort((left, right) => left.sortOrder - right.sortOrder),
    activeProfiles: (state) =>
      state.profiles
        .filter((profile) => !profile.deletedAt)
        .sort((left, right) => left.sortOrder - right.sortOrder),
    profilesAwaitingCredentialCleanup: (state) =>
      state.profiles.filter((profile) => profile.deletedAt && profile.credentialRef),
    favoriteProfiles() {
      return this.activeProfiles.filter((profile) => profile.favorite);
    },
    recentProfiles() {
      return this.activeProfiles
        .filter((profile) => Number(profile.lastConnectedAt) > 0)
        .sort((left, right) => Number(right.lastConnectedAt) - Number(left.lastConnectedAt));
    },
    profilesByGroup() {
      const fallbackGroup = { id: "group-default", name: "Ungrouped", sortOrder: 0 };
      const groups = this.activeGroups.length > 0 ? this.activeGroups : [fallbackGroup];
      const profiles = this.activeProfiles;
      const defaultGroup = groups.find((group) => group.id === "group-default") ?? groups[0] ?? null;
      const knownGroupIds = new Set(groups.map((group) => group.id));

      return groups.map((group) => ({
        group,
        profiles: profiles.filter((profile) => {
          if (profile.groupId === group.id) return true;

          return group.id === defaultGroup?.id && !knownGroupIds.has(profile.groupId);
        }),
      }));
    },
  },
  actions: {
    hydrate({ groups, profiles }) {
      this.groups = groups ?? [];
      this.profiles = profiles ?? [];
    },
    async saveProfile(profile) {
      const snapshot = await saveProfile(profile);
      this.hydrate(snapshot);
      return snapshot;
    },
    async deleteProfile(profileId) {
      const snapshot = await deleteProfile(profileId);
      this.hydrate(snapshot);
      return snapshot;
    },
    async toggleFavorite(profileId) {
      const profile = this.profiles.find((item) => item.id === profileId);
      if (!profile) return null;

      return this.saveProfile({ ...profile, favorite: !profile.favorite });
    },
    // 该连接专属的终端背景图；null = 取消覆盖，回到全局设置那张
    async setTerminalBackground(profileId, terminalBackground) {
      const profile = this.profiles.find((item) => item.id === profileId);
      if (!profile) return null;

      return this.saveProfile({ ...profile, terminalBackground });
    },
    // 连接成功后打点最近连接时间；失败静默（浏览器 dev 无后端）
    async markConnected(profileId) {
      try {
        const snapshot = await markProfileConnected(profileId);
        this.hydrate(snapshot);
        return snapshot;
      } catch (error) {
        console.warn("[Xin Terminal] mark profile connected failed", error);
        return null;
      }
    },
    async saveGroup(group) {
      const snapshot = await saveGroup(group);
      this.hydrate(snapshot);
      return snapshot;
    },
    // 新建分组：连接表单里的「就地新建」和首页侧栏共用同一条链路
    async createGroup(name) {
      const now = Date.now().toString();
      const groupId = crypto.randomUUID();

      await this.saveGroup({
        id: groupId,
        syncId: groupId,
        name,
        sortOrder: Math.max(-1, ...this.activeGroups.map((group) => group.sortOrder)) + 1,
        collapsed: false,
        createdAt: now,
        updatedAt: now,
        deletedAt: null,
      });
      return groupId;
    },
    async renameGroup(groupId, name) {
      const group = this.groups.find((item) => item.id === groupId);
      if (!group) return null;

      return this.saveGroup({ ...group, name });
    },
    // deleteProfiles = false：组内连接移回未分组；true：连接随分组一起删
    async deleteGroup(groupId, { deleteProfiles = false } = {}) {
      const snapshot = await deleteGroup(groupId, deleteProfiles);
      this.hydrate(snapshot);
      return snapshot;
    },
    async moveProfileToGroup(profileId, groupId, beforeProfileId = null) {
      if (this.isSavingLayout) return;
      this.isSavingLayout = true;
      try {
        this.hydrate(await moveProfile(profileId, groupId, beforeProfileId));
      } finally {
        this.isSavingLayout = false;
      }
    },
    async reorderGroups(groupIds) {
      if (this.isSavingLayout) return;
      this.isSavingLayout = true;
      try {
        this.hydrate(await reorderGroups(groupIds));
      } finally {
        this.isSavingLayout = false;
      }
    },
    async setGroupCollapsed(groupId, collapsed) {
      const group = this.activeGroups.find((item) => item.id === groupId);
      if (!group || group.collapsed === collapsed || this.isSavingLayout) return;
      this.isSavingLayout = true;
      try {
        await this.saveGroup({ ...group, collapsed });
      } finally {
        this.isSavingLayout = false;
      }
    },
    async retryCredentialCleanup() {
      if (this.isCleaningCredentials) return;
      this.isCleaningCredentials = true;
      try {
        this.hydrate(await retryDeletedCredentialCleanup());
      } finally {
        this.isCleaningCredentials = false;
      }
    },
    profilesInGroup(groupId) {
      return this.profilesByGroup.find(({ group }) => group.id === groupId)?.profiles ?? [];
    },
  },
});
