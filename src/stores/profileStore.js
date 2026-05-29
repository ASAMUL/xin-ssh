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
      return this.activeGroups.map((group) => ({
        group,
        profiles: this.activeProfiles.filter((profile) => profile.groupId === group.id),
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
