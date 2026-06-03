import { defineStore } from "pinia";
import { listRemoteDirectory } from "../services/workspaceApi";
import { useSessionStore } from "./sessionStore";

const DEFAULT_REMOTE_PATH = "/";
const EMPTY_FILE_STATE = createFileState(DEFAULT_REMOTE_PATH);

export const useFileExplorerStore = defineStore("fileExplorer", {
  state: () => ({
    sessionStates: {},
  }),
  getters: {
    getSessionState: (state) => (sessionId) => {
      if (!sessionId) return EMPTY_FILE_STATE;

      return state.sessionStates[sessionId] ?? EMPTY_FILE_STATE;
    },
  },
  actions: {
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
      const requestToken = `${Date.now()}-${Math.random()}`;
      state.requestToken = requestToken;
      state.requestedPath = targetPath;
      state.isLoading = true;
      state.error = null;

      try {
        const listing = await listRemoteDirectory({
          sessionId: session.id,
          path: targetPath,
        });

        if (state.requestToken !== requestToken) return;

        const listedPath = normalizeRemotePath(listing.path || targetPath);
        state.currentPath = listedPath;
        state.entries = (listing.entries || []).map(normalizeRemoteEntry);
        state.selectedPaths = [];
        state.hasLoaded = true;

        if (options.pushHistory !== false) {
          pushHistory(state, listedPath);
        }

        useSessionStore().updateSessionRemotePath(session.id, listedPath);
      } catch (error) {
        if (state.requestToken !== requestToken) return;

        state.entries = [];
        state.error = String(error);
        state.hasLoaded = false;
      } finally {
        if (state.requestToken === requestToken) {
          state.isLoading = false;
        }
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
  },
});

function createFileState(path) {
  const currentPath = normalizeRemotePath(path);

  return {
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
  };
}

function normalizeRemoteEntry(entry) {
  return {
    ...entry,
    path: normalizeRemotePath(entry.path),
    icon: fileIcon(entry),
  };
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

function normalizeRemotePath(path = DEFAULT_REMOTE_PATH) {
  const cleanPath = String(path || DEFAULT_REMOTE_PATH).trim().replaceAll("\\", "/");
  const parts = [];

  for (const part of cleanPath.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      parts.pop();
      continue;
    }
    parts.push(part);
  }

  return parts.length ? `/${parts.join("/")}` : DEFAULT_REMOTE_PATH;
}

function parentRemotePath(path) {
  const currentPath = normalizeRemotePath(path);
  if (currentPath === DEFAULT_REMOTE_PATH) return currentPath;

  const parts = currentPath.split("/").filter(Boolean);
  parts.pop();

  return parts.length ? `/${parts.join("/")}` : DEFAULT_REMOTE_PATH;
}
