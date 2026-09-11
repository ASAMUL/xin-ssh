import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import {
  createRemoteDirectory,
  createRemoteFile,
  copyRemotePath,
  chmodRemotePath,
  listRemoteDirectory,
  renameRemotePath,
  streamRemoteDirectory,
} from "../services/workspaceApi";
import {
  DEFAULT_REMOTE_PATH,
  normalizeRemotePath,
  parentRemotePath,
} from "../utils/remotePath";
import { useEditorStore } from "./editorStore";
import { useSessionStore } from "./sessionStore";
import { localizeError } from "../utils/appError";

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
        console.warn("[Xin SSH] directory event listener unavailable", error);
      }
    },
    ensureSessionState(session) {
      const current = useSessionStore().findSession(session?.id);
      if (!current || current.isClosing || session.currentAttemptId !== current.currentAttemptId) return null;

      if (!this.sessionStates[session.id]) {
        this.sessionStates[session.id] = createFileState(session.remotePath || DEFAULT_REMOTE_PATH, current.currentAttemptId);
      } else if (this.sessionStates[session.id].attemptId !== current.currentAttemptId) {
        this.invalidateSessionState(session.id, current.currentAttemptId);
      }

      return this.sessionStates[session.id];
    },
    // 重连保留浏览位置和历史，但远端内容必须重新读取，旧请求持有的状态对象随之失效。
    invalidateSessionState(sessionId, attemptId) {
      const state = this.sessionStates[sessionId];
      if (!state) return;
      this.sessionStates[sessionId] = {
        ...createFileState(state.currentPath, attemptId),
        query: state.query,
        history: [...state.history],
        historyIndex: state.historyIndex,
      };
    },
    removeSessionState(sessionId) {
      delete this.sessionStates[sessionId];
    },
    isCurrentState(sessionId, state) {
      const session = useSessionStore().findSession(sessionId);
      return Boolean(state && session && !session.isClosing && session.status === "connected"
        && session.currentAttemptId === state.attemptId && this.sessionStates[sessionId] === state);
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
    // 流式结果同时绑定请求与连接尝试，重连后旧批次不得写回或触发自动重连。
    async loadDirectory(session, path, options = {}) {
      if (!session) return;

      const state = this.ensureSessionState(session);
      if (!state) return;
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
        if (!this.isCurrentState(session.id, state) || state.requestToken !== requestId) return;

        state.requestToken = null;
        state.isLoading = false;
        state.entries = [];
        state.error = localizeError(error);
        state.hasLoaded = false;
        useSessionStore().handleConnectionOperationError(session.id, error);
      }
    },
    handleDirectoryEvent(event) {
      const state = this.sessionStates[event.sessionId];
      if (!this.isCurrentState(event.sessionId, state) || state.requestToken !== event.requestId) return;

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
        state.requestToken = null;
        state.entries = sortRemoteEntries([...state.entries]);
        state.isLoading = false;
        state.hasLoaded = true;
        state.error = null;
        return;
      }

      if (event.kind === "canceled") {
        state.requestToken = null;
        state.isLoading = false;
        return;
      }

      if (event.kind === "failed") {
        state.requestToken = null;
        state.entries = [];
        state.error = localizeError(event.error);
        state.isLoading = false;
        state.hasLoaded = false;
        useSessionStore().handleConnectionOperationError(event.sessionId, event.error);
      }
    },
    refreshDirectory(session) {
      const state = this.ensureSessionState(session);
      if (!state) return;
      return this.loadDirectory(session, state.currentPath, { pushHistory: false });
    },
    openEntry(session, entry) {
      if (!entry) return;
      if (entry.isDirectory) return this.loadDirectory(session, entry.path);

      // 双击文件：打开内置编辑器（软链接指向目录时由编辑器回退为导航）
      return useEditorStore().openFile(session, entry);
    },
    goToParent(session) {
      const state = this.ensureSessionState(session);
      if (!state) return;
      const parentPath = parentRemotePath(state.currentPath);

      if (parentPath === state.currentPath) return;

      return this.loadDirectory(session, parentPath);
    },
    goBack(session) {
      const state = this.ensureSessionState(session);
      if (!state || state.historyIndex <= 0) return;

      state.historyIndex -= 1;
      return this.loadDirectory(session, state.history[state.historyIndex], { pushHistory: false });
    },
    goForward(session) {
      const state = this.ensureSessionState(session);
      if (!state || state.historyIndex >= state.history.length - 1) return;

      state.historyIndex += 1;
      return this.loadDirectory(session, state.history[state.historyIndex], { pushHistory: false });
    },
    async createDirectory(session, directoryPath) {
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;

      await createRemoteDirectory({
        sessionId: session.id,
        path: directoryPath,
      });
      if (!this.isCurrentState(session.id, state)) return;
      await this.refreshDirectory(session);
      if (this.isCurrentState(session.id, state)) {
        this.refreshTreeChildren(session, parentRemotePath(directoryPath));
      }
    },
    async createFile(session, filePath) {
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;

      await createRemoteFile({
        sessionId: session.id,
        path: filePath,
      });
      if (!this.isCurrentState(session.id, state)) return;
      await this.refreshDirectory(session);
    },
    async renameEntry(session, fromPath, toPath) {
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;

      await renameRemotePath({
        sessionId: session.id,
        fromPath,
        toPath,
      });
      if (!this.isCurrentState(session.id, state)) return;
      await this.refreshDirectory(session);
      if (!this.isCurrentState(session.id, state)) return;
      this.refreshTreeChildren(session, parentRemotePath(fromPath));
      this.refreshTreeChildren(session, parentRemotePath(toPath));
    },
    async copyEntry(session, fromPath, toPath, recursive) {
      // 复制完成后同时刷新列表和目标父目录，避免目录树显示旧的远端状态。
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;

      await copyRemotePath({ sessionId: session.id, fromPath, toPath, recursive });
      if (!this.isCurrentState(session.id, state)) return;
      await this.refreshDirectory(session);
      if (!this.isCurrentState(session.id, state)) return;
      this.refreshTreeChildren(session, parentRemotePath(toPath));
    },
    async chmodEntry(session, path, mode) {
      // 权限变化只影响元数据，但列表与目录树都依赖权限字段，所以统一刷新来源。
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;

      await chmodRemotePath({ sessionId: session.id, path, mode });
      if (!this.isCurrentState(session.id, state)) return;
      await this.refreshDirectory(session);
      if (this.isCurrentState(session.id, state)) this.refreshTreeChildren(session, parentRemotePath(path));
    },
    async refreshAfterDeletion(session, paths) {
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;
      await this.refreshDirectory(session);
      if (!this.isCurrentState(session.id, state)) return;

      const parentPaths = new Set(paths.map(parentRemotePath));
      for (const parentPath of parentPaths) {
        this.refreshTreeChildren(session, parentPath);
      }
    },
    // 目录树独立加载，慢请求只更新自己发起时的缓存，避免填回重连后的新树。
    async loadTreeChildren(session, path, options = {}) {
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;
      const node = ensureTreeNode(state, path);
      if (node.loading || (node.loaded && !options.force)) return;

      node.loading = true;
      node.error = null;

      try {
        const entries = await listRemoteDirectory({
          sessionId: session.id,
          path: node.path,
        });
        if (!this.isCurrentState(session.id, state)) return;
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
        if (!this.isCurrentState(session.id, state)) return;
        node.error = localizeError(error);
        useSessionStore().handleConnectionOperationError(session.id, error);
      } finally {
        if (this.isCurrentState(session.id, state)) node.loading = false;
      }
    },
    async toggleTreeNode(session, path) {
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;
      const node = ensureTreeNode(state, path);

      node.expanded = !node.expanded;
      if (node.expanded && !node.loaded) {
        await this.loadTreeChildren(session, node.path);
      }
    },
    // 面板路径变化时展开目录树上的祖先链，让当前目录在树里可见
    async revealTreePath(session, targetPath) {
      const state = this.ensureSessionState(session);
      if (!this.isCurrentState(session?.id, state)) return;
      const path = normalizeRemotePath(targetPath);
      const ancestorPaths = ["/"];
      const segments = path === "/" ? [] : path.slice(1).split("/");
      let currentPath = "";

      for (const segment of segments.slice(0, -1)) {
        currentPath = `${currentPath}/${segment}`;
        ancestorPaths.push(currentPath);
      }

      for (const ancestorPath of ancestorPaths) {
        if (!this.isCurrentState(session.id, state)) return;
        const node = ensureTreeNode(state, ancestorPath);
        node.expanded = true;
        if (!node.loaded) {
          await this.loadTreeChildren(session, ancestorPath);
        }
      }
    },
    refreshTreeChildren(session, parentPath) {
      const state = this.sessionStates[session?.id];
      if (!this.isCurrentState(session?.id, state)) return;

      const node = state.treeNodes[normalizeRemotePath(parentPath)];
      if (!node?.loaded) return;

      return this.loadTreeChildren(session, node.path, { force: true });
    },
  },
});

function createFileState(path, attemptId = null) {
  const currentPath = normalizeRemotePath(path);
  const state = {
    attemptId,
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
