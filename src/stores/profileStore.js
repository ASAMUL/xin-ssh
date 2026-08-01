import { defineStore } from "pinia";
import {
  deleteGroup,
  deleteProfile,
  markProfileConnected,
  saveGroup,
  saveProfile,
} from "../services/workspaceApi";

export const useProfileStore = defineStore("profiles", {
  state: () => ({
    groups: [],
    profiles: [],
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
    async deleteGroup(groupId) {
      const snapshot = await deleteGroup(groupId);
      this.hydrate(snapshot);
      return snapshot;
    },
  },
});
