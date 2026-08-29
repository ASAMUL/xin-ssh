import { defineStore } from "pinia";
import { readRemoteFileText, writeRemoteFileText } from "../services/workspaceApi";
import { ERROR_CODES, errorCode } from "../utils/appError";
import { useFileExplorerStore } from "./fileExplorerStore";
import { useSessionStore } from "./sessionStore";

// 与后端 MAX_EDITABLE_FILE_BYTES 保持一致
export const EDITOR_MAX_FILE_BYTES = 5 * 1024 * 1024;

// 常见二进制扩展名：双击时直接提示，避免整份拉取后才失败。
// 这里构造的错误与后端 AppError 同形（code/message/params），组件按同一套逻辑本地化。
const BINARY_FILE_EXTENSIONS = new Set([
  "png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "svgz", "heic",
  "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
  "zip", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar", "tar", "jar", "war",
  "so", "dylib", "dll", "exe", "bin", "iso", "img", "deb", "rpm", "apk",
  "mp3", "mp4", "mkv", "avi", "mov", "flac", "ogg", "wav", "webm",
  "woff", "woff2", "ttf", "otf", "eot",
  "db", "sqlite", "sqlite3", "pyc", "class", "o", "a", "wasm",
]);

export const useEditorStore = defineStore("editor", {
  state: () => ({
    isOpen: false,
    sessionId: null,
    path: "",
    name: "",
    size: 0,
    initialContent: "",
    isLoading: false,
    isSaving: false,
    isDirty: false,
    confirmingClose: false,
    loadError: null,
    saveError: null,
    lastSavedAt: null,
    requestToken: null,
  }),
  getters: {
    session(state) {
      return useSessionStore().findSession(state.sessionId);
    },
    hasDocument(state) {
      return state.isOpen && !state.isLoading && !state.loadError;
    },
  },
  actions: {
    async openFile(session, entry) {
      if (!session || session.status !== "connected" || !entry || entry.isDirectory) return;

      const token = crypto.randomUUID();
      this.requestToken = token;
      this.isOpen = true;
      this.sessionId = session.id;
      this.path = entry.path;
      this.name = entry.name;
      this.size = Number(entry.size ?? 0);
      this.initialContent = "";
      this.isDirty = false;
      this.confirmingClose = false;
      this.loadError = null;
      this.saveError = null;
      this.lastSavedAt = null;

      const guardError = quickEditGuardError(entry);
      if (guardError) {
        this.isLoading = false;
        this.loadError = guardError;
        return;
      }

      this.isLoading = true;

      try {
        const result = await readRemoteFileText({
          sessionId: session.id,
          path: entry.path,
        });
        if (this.requestToken !== token) return;

        this.initialContent = result?.content ?? "";
        this.size = Number(result?.size ?? 0);
        this.isLoading = false;
      } catch (error) {
        if (this.requestToken !== token) return;

        // 双击到指向目录的软链接：退回为目录导航
        if (errorCode(error) === ERROR_CODES.remoteIsDirectory) {
          this.close();
          useFileExplorerStore().loadDirectory(session, entry.path);
          return;
        }

        this.loadError = error;
        this.isLoading = false;
        useSessionStore().handleConnectionOperationError(session.id, error);
      }
    },
    retryLoad() {
      const session = this.session;
      if (!session || !this.path) return;

      return this.openFile(session, {
        path: this.path,
        name: this.name,
        size: this.size,
        isDirectory: false,
      });
    },
    async saveFile(content, options = {}) {
      if (!this.hasDocument || this.isSaving || typeof content !== "string") return;

      const session = this.session;
      if (!session || session.status !== "connected") {
        this.saveError = { code: ERROR_CODES.sessionNotRunning, message: "", params: {} };
        return;
      }

      this.isSaving = true;
      this.saveError = null;

      try {
        await writeRemoteFileText({
          sessionId: this.sessionId,
          path: this.path,
          content,
        });

        this.initialContent = content;
        this.size = content.length;
        this.isDirty = false;
        this.lastSavedAt = Date.now();

        // 刷新文件列表里的大小/修改时间；失败不影响保存结果
        useFileExplorerStore().refreshDirectory(session);

        if (options.closeAfter) {
          this.close();
        }
      } catch (error) {
        this.saveError = error;
        useSessionStore().handleConnectionOperationError(this.sessionId, error);
      } finally {
        this.isSaving = false;
      }
    },
    markDirty() {
      if (!this.isDirty) this.isDirty = true;
      if (this.saveError) this.saveError = null;
    },
    requestClose() {
      if (this.isSaving) return;

      if (this.isDirty && this.hasDocument) {
        this.confirmingClose = true;
        return;
      }

      this.close();
    },
    cancelCloseConfirm() {
      this.confirmingClose = false;
    },
    discardAndClose() {
      if (this.isSaving) return;

      this.isDirty = false;
      this.close();
    },
    close() {
      this.requestToken = null;
      this.isOpen = false;
      this.isLoading = false;
      this.confirmingClose = false;
    },
  },
});

function quickEditGuardError(entry) {
  if (Number(entry.size ?? 0) > EDITOR_MAX_FILE_BYTES) {
    return {
      code: ERROR_CODES.fileTooLarge,
      message: "",
      params: { limitMb: String(EDITOR_MAX_FILE_BYTES / (1024 * 1024)) },
    };
  }

  const name = String(entry.name || "");
  const dotIndex = name.lastIndexOf(".");
  const extension = dotIndex > 0 ? name.slice(dotIndex + 1).toLowerCase() : "";
  if (extension && BINARY_FILE_EXTENSIONS.has(extension)) {
    return { code: ERROR_CODES.fileBinary, message: "", params: {} };
  }

  return null;
}
