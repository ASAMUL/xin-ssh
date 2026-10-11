import { defineStore } from "pinia";
import {
  deleteGroup,
  deleteProfile,
  markProfileConnected,
  moveProfile,
  reorderGroups,
  reorderProfiles,
  retryDeletedCredentialCleanup,
  saveGroup,
  saveProfile,
  setProfileBackground,
  setProfileDetectedOs,
} from "../services/workspaceApi";
import { normalizeTerminalBackground } from "../services/backgroundApi";
import { createSerialQueue } from "../utils/serialQueue";

const enqueueProfileWrite = createSerialQueue();

export const useProfileStore = defineStore("profiles", {
  state: () => ({
    groups: [],
    profiles: [],
    workspaceDir: null,
    backgroundDrafts: {},
    backgroundRevision: 0,
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
      state.profiles.filter((profile) => (profile.deletedAt || ["agent", "keyboardInteractive"].includes(profile.authMethod))
        && profile.credentialRef),
    favoriteProfiles() {
      return this.activeProfiles.filter((profile) => profile.favorite);
    },
    // 生产机标记：会话、标签、终端都按连接 id 现查，改了标记开着的会话立刻跟着变
    isProductionProfile: (state) => (profileId) =>
      Boolean(profileId && state.profiles.find((profile) => profile.id === profileId)?.production),
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
    // 后台打点和表单保存都会返回整份快照，未落盘的背景预览必须覆盖其旧值。
    hydrate({ groups, profiles, workspaceDir = this.workspaceDir }) {
      if (workspaceDir !== this.workspaceDir) this.backgroundDrafts = {};
      this.workspaceDir = workspaceDir;
      this.groups = groups ?? [];
      this.profiles = (profiles ?? []).map((profile) => {
        const draft = this.backgroundDrafts[profile.id];
        return draft && !profile.deletedAt ? { ...profile, terminalBackground: draft.background } : profile;
      });
      for (const id of Object.keys(this.backgroundDrafts)) {
        if (!this.activeProfiles.some((profile) => profile.id === id)) delete this.backgroundDrafts[id];
      }
    },
    // 写请求和快照消费一起串行化，连接打点返回的旧快照不能盖住后一次编辑。
    persistChange(operation, onSaved = () => {}) {
      const workspaceDir = this.workspaceDir;
      return enqueueProfileWrite(async () => {
        if (workspaceDir !== this.workspaceDir) {
          throw { code: "workspace.changed", message: "Workspace changed before saving" };
        }
        const snapshot = await operation();
        if (workspaceDir === this.workspaceDir) {
          onSaved();
          this.hydrate(snapshot);
        }
        return snapshot;
      });
    },
    async saveProfile(profile) {
      const patch = { ...profile };
      return this.persistChange(() => saveProfile({
        ...this.profiles.find((item) => item.id === patch.id),
        ...patch,
      }));
    },
    // 导入只创建连接引用，不触碰凭据；逐条复用现有串行保存可让失败项明确停在当前条目。
    async importSshConfigEntries(entries) {
      const groupId = this.activeGroups.find((group) => group.id === "group-default")?.id
        ?? this.activeGroups[0]?.id ?? "group-default";
      const signatures = new Set(this.activeProfiles.map((profile) => profileSignature(profile)));
      let imported = 0;
      for (const entry of entries) {
        const signature = profileSignature({
          name: entry.alias,
          host: entry.host,
          port: entry.port || 22,
          username: entry.username,
        });
        if (signatures.has(signature)) continue;
        signatures.add(signature);
        const id = crypto.randomUUID();
        const now = Date.now().toString();
        await this.saveProfile({
          id,
          syncId: id,
          groupId,
          name: entry.alias,
          host: entry.host,
          port: entry.port || 22,
          username: entry.username,
          authMethod: entry.privateKeyPath ? "privateKey" : "password",
          privateKeyPath: entry.privateKeyPath ?? null,
          agent: null,
          credentialRef: null,
          defaultRemotePath: "/",
          sortOrder: Math.max(-1, ...this.activeProfiles.map((profile) => profile.sortOrder)) + 1,
          favorite: false,
          lastConnectedAt: null,
          createdAt: now,
          updatedAt: now,
          deletedAt: null,
        });
        imported += 1;
      }
      return imported;
    },
    async deleteProfile(profileId) {
      return this.persistChange(() => deleteProfile(profileId));
    },
    async toggleFavorite(profileId) {
      return this.persistChange(() => {
        const profile = this.activeProfiles.find((item) => item.id === profileId);
        if (!profile) throw { code: "profile.notFound", message: "Connection profile not found" };
        return saveProfile({ ...profile, favorite: !profile.favorite });
      });
    },
    previewTerminalBackground(profileId, terminalBackground) {
      const profile = this.activeProfiles.find((item) => item.id === profileId);
      if (!profile) throw { code: "profile.notFound", message: "Connection profile not found" };
      const draft = {
        background: normalizeTerminalBackground(terminalBackground),
        revision: ++this.backgroundRevision,
      };
      this.backgroundDrafts[profileId] = draft;
      profile.terminalBackground = draft.background;
      return draft;
    },
    // 该连接专属的终端背景图；null = 取消覆盖，回到全局设置那张
    async setTerminalBackground(profileId, terminalBackground, workspaceDir = this.workspaceDir) {
      if (workspaceDir !== this.workspaceDir) {
        throw { code: "workspace.changed", message: "Workspace changed before saving" };
      }
      const draft = this.previewTerminalBackground(profileId, terminalBackground);
      return this.persistChange(
        () => setProfileBackground(profileId, draft.background, workspaceDir),
        () => {
          // 等待保存时可能已拖出新预览，只清掉本次确实保存过的那一版。
          if (this.backgroundDrafts[profileId]?.revision === draft.revision) {
            delete this.backgroundDrafts[profileId];
          }
        },
      );
    },
    // 连接成功后打点最近连接时间；失败静默（浏览器 dev 无后端）
    async markConnected(profileId) {
      try {
        return await this.persistChange(() => markProfileConnected(profileId));
      } catch (error) {
        console.warn("[Xin SSH] mark profile connected failed", error);
        return null;
      }
    },
    // 识别结果只是图标提示，失败不打扰用户；结果没变就不写盘，避免每轮采样都落一次 profiles.json
    async recordDetectedOs(profileId, detectedOs) {
      const profile = this.activeProfiles.find((item) => item.id === profileId);
      if (!profile || !detectedOs || profile.detectedOs === detectedOs) return null;
      try {
        return await this.persistChange(() => setProfileDetectedOs(profileId, detectedOs));
      } catch (error) {
        console.warn("[Xin SSH] record detected os failed", error);
        return null;
      }
    },
    async saveGroup(group) {
      const patch = { ...group };
      return this.persistChange(() => saveGroup({
        ...this.groups.find((item) => item.id === patch.id),
        ...patch,
      }));
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

      return this.saveGroup({ id: groupId, name });
    },
    // deleteProfiles = false：组内连接移回未分组；true：连接随分组一起删
    async deleteGroup(groupId, { deleteProfiles = false } = {}) {
      return this.persistChange(() => deleteGroup(groupId, deleteProfiles));
    },
    async moveProfileToGroup(profileId, groupId) {
      if (this.isSavingLayout) return;
      this.isSavingLayout = true;
      try {
        await this.persistChange(() => moveProfile(profileId, groupId));
      } finally {
        this.isSavingLayout = false;
      }
    },
    // visibleIds 是拖放后屏幕上的顺序，可能只是筛选出的子集：
    // 只让被拖的连接贴到它的可见邻居旁边，被筛掉的连接保持原位。
    async placeProfile(profileId, visibleIds) {
      const ids = this.activeProfiles.map((profile) => profile.id).filter((id) => id !== profileId);
      const position = visibleIds.indexOf(profileId);
      const previousId = visibleIds[position - 1];
      const index = previousId ? ids.indexOf(previousId) + 1 : ids.indexOf(visibleIds[position + 1]);
      ids.splice(index < 0 ? ids.length : index, 0, profileId);
      return this.reorderProfiles(ids);
    },
    async reorderProfiles(profileIds) {
      if (this.isSavingLayout) return;
      this.isSavingLayout = true;
      // 松手时 SortableJS 已把 DOM 还原，等落盘回来再重排会先闪回原位，所以先在本地落位
      const previousOrders = new Map(this.profiles.map((profile) => [profile.id, profile.sortOrder]));
      const nextOrders = new Map(profileIds.map((id, index) => [id, index]));
      this.profiles = this.profiles.map((profile) => nextOrders.has(profile.id)
        ? { ...profile, sortOrder: nextOrders.get(profile.id) }
        : profile);
      try {
        await this.persistChange(() => reorderProfiles(profileIds));
      } catch (error) {
        this.profiles = this.profiles.map((profile) => previousOrders.has(profile.id)
          ? { ...profile, sortOrder: previousOrders.get(profile.id) }
          : profile);
        throw error;
      } finally {
        this.isSavingLayout = false;
      }
    },
    async reorderGroups(groupIds) {
      if (this.isSavingLayout) return;
      this.isSavingLayout = true;
      try {
        await this.persistChange(() => reorderGroups([...groupIds]));
      } finally {
        this.isSavingLayout = false;
      }
    },
    async setGroupCollapsed(groupId, collapsed) {
      const group = this.activeGroups.find((item) => item.id === groupId);
      if (!group || group.collapsed === collapsed || this.isSavingLayout) return;
      this.isSavingLayout = true;
      try {
        await this.saveGroup({ id: groupId, collapsed });
      } finally {
        this.isSavingLayout = false;
      }
    },
    async retryCredentialCleanup() {
      if (this.isCleaningCredentials) return;
      this.isCleaningCredentials = true;
      try {
        await this.persistChange(() => retryDeletedCredentialCleanup());
      } finally {
        this.isCleaningCredentials = false;
      }
    },
    profilesInGroup(groupId) {
      return this.profilesByGroup.find(({ group }) => group.id === groupId)?.profiles ?? [];
    },
  },
});

function profileSignature(profile) {
  return [profile.name, profile.host, profile.port || 22, profile.username]
    .map((value) => String(value ?? "").trim().toLowerCase())
    .join("\u0000");
}
