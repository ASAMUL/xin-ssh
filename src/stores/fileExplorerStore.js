import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import {
  createRemoteDirectory,
  createRemoteFile,
  deleteRemoteEntries,
  listRemoteDirectory,
  renameRemotePath,
  streamRemoteDirectory,
} from "../services/workspaceApi";
import {
  DEFAULT_REMOTE_PATH,
  normalizeRemotePath,
  parentRemotePath,
} from "../utils/remotePath";
import { useSessionStore } from "./sessionStore";

const DIRECTORY_EVENT_NAME = "directory-event";
const EMPTY_FILE_STATE = createFileState(DEFAULT_REMOTE_PATH);

export const useFileExplorerStore = defineStore("fileExplorer", {
  state: () => ({
    sessionStates: {},
    unlistenDirectory: null,
  }),
  getters: {
    getSessionState: (state) => (sessionId) => {
      if (!sessionId) return EMPTY_FILE_STATE;

      return state.sessionStates[sessionId] ?? EMPTY_FILE_STATE;
    },
  },
  actions: {
    async startListening() {
      if (this.unlistenDirectory) return;

      try {
        this.unlistenDirectory = await listen(DIRECTORY_EVENT_NAME, (event) => {
          this.handleDirectoryEvent(event.payload);
        });
      } catch (error) {
        console.warn("[Xin Terminal] directory event listener unavailable", error);
      }
    },
    ensureSessionState(session) {
      if (!session) return EMPTY_FILE_STATE;

      if (!this.sessionStates[session.id]) {
        this.sessionStates[session.id] = createFileState(session.remotePath || DEFAULT_REMOTE_PATH);
      }

      return this.sessionStates[session.id];
    },
    setQuery(sessionId, query) {
      const state = this.sessionStates[sessionId];
      if (!state) return;

      state.query = query;
    },
    clearSelection(sessionId) {
      const state = this.sessionStates[sessionId];
      if (!state) return;

      state.selectedPaths = [];
    },
    setSelection(sessionId, entryPaths) {
      const state = this.sessionStates[sessionId];
      if (!state) return;

      state.selectedPaths = Array.from(new Set(entryPaths.filter(Boolean)));
    },
    selectEntry(sessionId, entryPath) {
      const state = this.sessionStates[sessionId];
      if (!state) return;

      state.selectedPaths = entryPath ? [entryPath] : [];
    },
    toggleEntrySelection(sessionId, entryPath) {
      const state = this.sessionStates[sessionId];
      if (!state || !entryPath) return;

      if (state.selectedPaths.includes(entryPath)) {
        state.selectedPaths = state.selectedPaths.filter((path) => path !== entryPath);
      } else {
        state.selectedPaths = [...state.selectedPaths, entryPath];
      }
    },
    async loadDirectory(session, path, options = {}) {
      if (!session) return;

      const state = this.ensureSessionState(session);
      if (session.status !== "connected") {
        state.entries = [];
        state.error = null;
        state.isLoading = false;
        state.hasLoaded = false;
        return;
      }

      const targetPath = normalizeRemotePath(path || state.currentPath || session.remotePath);
      const requestId = crypto.randomUUID();
      state.requestToken = requestId;
      state.requestedPath = targetPath;
      state.pushHistoryOnLoad = options.pushHistory !== false;
      state.isLoading = true;
      state.error = null;
      state.entries = [];
      state.selectedPaths = [];

      try {
        await streamRemoteDirectory({
          sessionId: session.id,
          requestId,
          path: targetPath,
        });
      } catch (error) {
        if (state.requestToken !== requestId) return;

        state.isLoading = false;
        state.entries = [];
        state.error = String(error);
        state.hasLoaded = false;
        useSessionStore().handleConnectionOperationError(session.id, error);
      }
    },
    handleDirectoryEvent(event) {
      const state = this.sessionStates[event.sessionId];
      if (!state || state.requestToken !== event.requestId) return;

      if (event.kind === "started") {
        const listedPath = normalizeRemotePath(event.path);
        state.currentPath = listedPath;
        state.entries = [];
        state.selectedPaths = [];

        if (state.pushHistoryOnLoad) {
          pushHistory(state, listedPath);
        }

        useSessionStore().updateSessionRemotePath(event.sessionId, listedPath);
        return;
      }

      if (event.kind === "chunk") {
        state.entries.push(...(event.entries || []).map(normalizeRemoteEntry));
        return;
      }

      if (event.kind === "completed") {
        state.entries = sortRemoteEntries([...state.entries]);
        state.isLoading = false;
        state.hasLoaded = true;
        state.error = null;
        return;
      }

      if (event.kind === "canceled") {
        state.isLoading = false;
        return;
      }

      if (event.kind === "failed") {
        state.entries = [];
        state.error = event.error || "Directory listing failed";
        state.isLoading = false;
        state.hasLoaded = false;
        useSessionStore().handleConnectionOperationError(event.sessionId, state.error);
      }
    },
    refreshDirectory(session) {
      const state = this.ensureSessionState(session);
      return this.loadDirectory(session, state.currentPath, { pushHistory: false });
    },
    openEntry(session, entry) {
      if (!entry?.isDirectory) return;

      return this.loadDirectory(session, entry.path);
    },
    goToParent(session) {
      const state = this.ensureSessionState(session);
      const parentPath = parentRemotePath(state.currentPath);

      if (parentPath === state.currentPath) return;

      return this.loadDirectory(session, parentPath);
    },
    goBack(session) {
      const state = this.ensureSessionState(session);
      if (state.historyIndex <= 0) return;

      state.historyIndex -= 1;
      return this.loadDirectory(session, state.history[state.historyIndex], { pushHistory: false });
    },
    goForward(session) {
      const state = this.ensureSessionState(session);
      if (state.historyIndex >= state.history.length - 1) return;

      state.historyIndex += 1;
      return this.loadDirectory(session, state.history[state.historyIndex], { pushHistory: false });
    },
    async createDirectory(session, directoryPath) {
      if (!session || session.status !== "connected") return;

      await createRemoteDirectory({
        sessionId: session.id,
        path: directoryPath,
      });
      await this.refreshDirectory(session);
      this.refreshTreeChildren(session, parentRemotePath(directoryPath));
    },
    async createFile(session, filePath) {
      if (!session || session.status !== "connected") return;

      await createRemoteFile({
        sessionId: session.id,
        path: filePath,
      });
      await this.refreshDirectory(session);
    },
    async renameEntry(session, fromPath, toPath) {
      if (!session || session.status !== "connected") return;

      await renameRemotePath({
        sessionId: session.id,
        fromPath,
        toPath,
      });
      await this.refreshDirectory(session);
      this.refreshTreeChildren(session, parentRemotePath(fromPath));
      this.refreshTreeChildren(session, parentRemotePath(toPath));
    },
    async deleteEntries(session, entries, recursive = false) {
      if (!session || session.status !== "connected" || !entries?.length) return;

      await deleteRemoteEntries({
        sessionId: session.id,
        entries: entries.map((entry) => ({
          path: entry.path,
          name: entry.name,
          isDirectory: entry.isDirectory,
          size: entry.size,
        })),
        recursive,
      });
      await this.refreshDirectory(session);

      const parentPaths = new Set(entries.map((entry) => parentRemotePath(entry.path)));
      for (const parentPath of parentPaths) {
        this.refreshTreeChildren(session, parentPath);
      }
    },
    // ---- 目录树（懒加载，仅目录；与面板流式列表互不影响） ----
    async loadTreeChildren(session, path, options = {}) {
      if (!session || session.status !== "connected") return;

      const state = this.ensureSessionState(session);
      const node = ensureTreeNode(state, path);
      if (node.loading || (node.loaded && !options.force)) return;

      node.loading = true;
      node.error = null;

      try {
        const entries = await listRemoteDirectory({
          sessionId: session.id,
          path: node.path,
        });
        node.childDirs = entries
          .filter((entry) => entry.isDirectory)
          .sort((left, right) => left.name.toLowerCase().localeCompare(right.name.toLowerCase()))
          .map((entry) => ({
            path: normalizeRemotePath(entry.path),
            name: entry.name,
            isHidden: Boolean(entry.isHidden),
          }));

        for (const child of node.childDirs) {
          ensureTreeNode(state, child.path, child.name);
        }

        node.loaded = true;
      } catch (error) {
        node.error = String(error);
        useSessionStore().handleConnectionOperationError(session.id, error);
      } finally {
        node.loading = false;
      }
    },
    async toggleTreeNode(session, path) {
      const state = this.ensureSessionState(session);
      const node = ensureTreeNode(state, path);

      node.expanded = !node.expanded;
      if (node.expanded && !node.loaded) {
        await this.loadTreeChildren(session, node.path);
      }
    },
    // 面板路径变化时展开目录树上的祖先链，让当前目录在树里可见
    async revealTreePath(session, targetPath) {
      if (!session || session.status !== "connected") return;

      const state = this.ensureSessionState(session);
      const path = normalizeRemotePath(targetPath);
      const ancestorPaths = ["/"];
      const segments = path === "/" ? [] : path.slice(1).split("/");
      let currentPath = "";

      for (const segment of segments.slice(0, -1)) {
        currentPath = `${currentPath}/${segment}`;
        ancestorPaths.push(currentPath);
      }

      for (const ancestorPath of ancestorPaths) {
        const node = ensureTreeNode(state, ancestorPath);
        node.expanded = true;
        if (!node.loaded) {
          await this.loadTreeChildren(session, ancestorPath);
        }
      }
    },
    refreshTreeChildren(session, parentPath) {
      const state = this.sessionStates[session?.id];
      if (!state) return;

      const node = state.treeNodes[normalizeRemotePath(parentPath)];
      if (!node?.loaded) return;

      return this.loadTreeChildren(session, node.path, { force: true });
    },
  },
});

function createFileState(path) {
  const currentPath = normalizeRemotePath(path);
  const state = {
    entries: [],
    currentPath,
    requestedPath: currentPath,
    query: "",
    isLoading: false,
    hasLoaded: false,
    error: null,
    selectedPaths: [],
    history: [currentPath],
    historyIndex: 0,
    requestToken: null,
    pushHistoryOnLoad: true,
    treeNodes: {},
  };

  ensureTreeNode(state, DEFAULT_REMOTE_PATH);
  return state;
}

function ensureTreeNode(state, path, name = "") {
  const normalized = normalizeRemotePath(path);

  if (!state.treeNodes[normalized]) {
    state.treeNodes[normalized] = {
      path: normalized,
      name: name || (normalized === "/" ? "/" : normalized.split("/").pop()),
      expanded: normalized === "/",
      loading: false,
      loaded: false,
      error: null,
      childDirs: [],
    };
  }

  return state.treeNodes[normalized];
}

function normalizeRemoteEntry(entry) {
  return {
    ...entry,
    path: normalizeRemotePath(entry.path),
    icon: fileIcon(entry),
  };
}

function sortRemoteEntries(entries) {
  return entries.sort((left, right) => {
    if (left.isDirectory !== right.isDirectory) {
      return left.isDirectory ? -1 : 1;
    }

    return left.name.toLowerCase().localeCompare(right.name.toLowerCase());
  });
}

function fileIcon(entry) {
  if (entry.isSymlink) return "i-lucide-file-symlink";
  if (entry.isDirectory) return "i-lucide-folder";

  return "i-lucide-file";
}

function pushHistory(state, path) {
  if (state.history[state.historyIndex] === path) return;

  state.history = state.history.slice(0, state.historyIndex + 1);
  state.history.push(path);
  state.historyIndex = state.history.length - 1;
}
