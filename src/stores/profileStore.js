import { defineStore } from "pinia";
import { deleteGroup, deleteProfile, saveGroup, saveProfile } from "../services/workspaceApi";

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
