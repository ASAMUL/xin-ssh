<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { useI18n } from "vue-i18n";
import { useFileExplorerStore } from "../stores/fileExplorerStore";
import { useTransferStore } from "../stores/transferStore";
import { useUiStore } from "../stores/uiStore";
import { useSessionStore } from "../stores/sessionStore";
import { joinRemotePath, parentRemotePath } from "../utils/remotePath";

const props = defineProps({
  activeSession: {
    type: Object,
    default: null,
  },
});

const { t, locale } = useI18n();
const fileExplorerStore = useFileExplorerStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();
const sessionStore = useSessionStore();
const fileExplorerRoot = ref(null);
const fileTableWrap = ref(null);
const pathInput = ref("/");
const pathInputControl = ref(null);
const searchInputControl = ref(null);
const isDragOver = ref(false);
const lastSelectedPath = ref("");
const contextMenuEntry = ref(null);
const nameDialog = reactive({
  open: false,
  mode: "create",
  entry: null,
  value: "",
  error: "",
  isSubmitting: false,
});
const deleteDialog = reactive({
  open: false,
  entries: [],
  recursive: false,
  error: "",
  isSubmitting: false,
});
const conflictDialog = reactive({
  open: false,
  taskId: null,
  error: "",
  isSubmitting: false,
});
const fileSearchInputUi = {
  root: "file-search",
  base: "h-8 rounded-lg bg-default ps-8 pe-3 text-xs font-semibold shadow-[inset_0_1px_0_rgba(255,255,255,0.9)] placeholder:text-dimmed",
  leading: "ps-2.5",
  leadingIcon: "size-4 text-muted",
};
const fileContextMenuUi = {
  content: "min-w-40 rounded-lg border border-default bg-default shadow-xl backdrop-blur-xl",
  viewport: "divide-y divide-default",
  group: "p-1",
  separator: "my-1 bg-border",
  item: "min-h-8 items-center gap-2 rounded-md px-2 py-1.5 text-xs font-semibold leading-none",
  itemLeadingIcon: "size-4 w-4 shrink-0 text-muted",
  itemWrapper: "min-w-0 flex-1 justify-center",
  itemLabel: "truncate leading-4",
  itemTrailing: "ms-auto h-4 items-center",
  itemTrailingIcon: "size-4",
};

let unlistenDragDrop = null;

const fileState = computed(() => fileExplorerStore.getSessionState(props.activeSession?.id));
const currentPath = computed(() => fileState.value.currentPath || props.activeSession?.remotePath || "/");
const isConnected = computed(() => props.activeSession?.status === "connected");
const isConnecting = computed(() => props.activeSession?.status === "connecting");
const canNavigate = computed(() => isConnected.value && !fileState.value.isLoading);
const canTransfer = computed(() => Boolean(props.activeSession) && isConnected.value);
const canRefresh = computed(() => Boolean(props.activeSession) && !isConnecting.value && !fileState.value.isLoading);
const canGoBack = computed(() => fileState.value.historyIndex > 0);
const canGoForward = computed(() => fileState.value.historyIndex < fileState.value.history.length - 1);
const hasEntries = computed(() => fileState.value.entries.length > 0);
const query = computed({
  get: () => fileState.value.query,
  set: (value) => fileExplorerStore.setQuery(props.activeSession?.id, value),
});
const visibleEntries = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return fileState.value.entries;

  return fileState.value.entries.filter((entry) => entry.name.toLowerCase().includes(keyword));
});
// 行高需与 file-explorer.css 中 .file-row 的 min-height 保持一致
const FILE_ROW_HEIGHT = 32;
const rowVirtualizer = useVirtualizer(
  computed(() => ({
    count: visibleEntries.value.length,
    getScrollElement: () => fileTableWrap.value,
    estimateSize: () => FILE_ROW_HEIGHT,
    overscan: 12,
  })),
);
const virtualEntryRows = computed(() =>
  rowVirtualizer.value
    .getVirtualItems()
    .map((virtualRow) => ({ virtualRow, entry: visibleEntries.value[virtualRow.index] }))
    .filter((row) => row.entry),
);
const totalListHeight = computed(() => rowVirtualizer.value.getTotalSize());
const selectedPathSet = computed(() => new Set(fileState.value.selectedPaths));
const selectedEntries = computed(() =>
  fileState.value.entries.filter((entry) => selectedPathSet.value.has(entry.path)),
);
const canDownload = computed(() => canTransfer.value && selectedEntries.value.length > 0);
const canRename = computed(() => canTransfer.value && selectedEntries.value.length === 1);
const canDelete = computed(() => canTransfer.value && selectedEntries.value.length > 0);
const contextEntries = computed(() => {
  const entry = contextMenuEntry.value;
  if (!entry) return selectedEntries.value;

  return isSelected(entry) && selectedEntries.value.length ? selectedEntries.value : [entry];
});
const contextMenuItems = computed(() => {
  const entry = contextMenuEntry.value;
  const entries = contextEntries.value;

  return [
    [
      ...(entry?.isDirectory
        ? [
            {
              label: t("files.open"),
              icon: "i-lucide-folder-open",
              onSelect: openContextEntry,
            },
          ]
        : []),
      {
        label: t("files.download"),
        icon: "i-lucide-download",
        disabled: !canTransfer.value || !entries.length,
        onSelect: downloadContextEntries,
      },
      ...(entry
        ? [
            {
              label: t("files.copyPath"),
              icon: "i-lucide-copy",
              onSelect: copyContextPath,
            },
            {
              label: t("files.rename"),
              icon: "i-lucide-pencil",
              disabled: !canTransfer.value,
              onSelect: renameContextEntry,
            },
          ]
        : []),
    ],
    [
      {
        label: t("files.delete"),
        icon: "i-lucide-trash-2",
        color: "error",
        disabled: !canTransfer.value || !entries.length,
        onSelect: deleteContextEntries,
      },
    ],
    [
      {
        label: t("files.uploadFiles"),
        icon: "i-lucide-upload",
        disabled: !canTransfer.value,
        onSelect: chooseUploadFiles,
      },
      {
        label: t("files.uploadFolder"),
        icon: "i-lucide-folder-up",
        disabled: !canTransfer.value,
        onSelect: chooseUploadFolder,
      },
      {
        label: t("files.newFolder"),
        icon: "i-lucide-folder-plus",
        disabled: !canTransfer.value,
        onSelect: openCreateFolderDialog,
      },
      {
        label: t("files.refresh"),
        icon: "i-lucide-refresh-cw",
        disabled: !canRefresh.value,
        onSelect: refreshDirectory,
      },
    ],
  ];
});
const nameDialogTitle = computed(() =>
  t(nameDialog.mode === "create" ? "files.createFolderTitle" : "files.renameTitle"),
);
const nameDialogDescription = computed(() =>
  t(nameDialog.mode === "create" ? "files.createFolderDescription" : "files.renameDescription"),
);
const nameDialogSubmitLabel = computed(() =>
  t(nameDialog.mode === "create" ? "files.createFolder" : "files.rename"),
);
const deleteDialogTitle = computed(() =>
  t("files.deleteTitle", { count: deleteDialog.entries.length }),
);
const deletePreviewNames = computed(() =>
  deleteDialog.entries.slice(0, 5).map((entry) => entry.name).join(", "),
);
const deleteHasDirectory = computed(() =>
  deleteDialog.entries.some((entry) => entry.isDirectory),
);
const conflictTask = computed(() => transferStore.findTask(conflictDialog.taskId));
const conflictDialogTitle = computed(() =>
  t(conflictTask.value?.direction === "download" ? "files.downloadConflictTitle" : "files.uploadConflictTitle"),
);
const conflictDialogDescription = computed(() => conflictTask.value?.error || "");
const emptyState = computed(() => {
  if (!isConnected.value) {
    return {
      icon: isConnecting.value ? "i-lucide-loader-circle" : "i-lucide-unplug",
      title: t("files.waitingTitle"),
      description: isConnecting.value
        ? t("files.reconnectingDescription")
        : t("files.waitingDescription"),
      spinning: isConnecting.value,
    };
  }

  if (fileState.value.isLoading) {
    return {
      icon: "i-lucide-loader-circle",
      title: t("files.loadingTitle"),
      description: t("files.loadingDescription", { path: fileState.value.requestedPath }),
      spinning: true,
    };
  }

  if (fileState.value.error) {
    return {
      icon: "i-lucide-circle-alert",
      title: t("files.errorTitle"),
      description: fileState.value.error,
      isError: true,
    };
  }

  if (query.value.trim()) {
    return {
      icon: "i-lucide-search-x",
      title: t("files.noMatchesTitle"),
      description: t("files.noMatchesDescription"),
    };
  }

  return {
    icon: "i-lucide-folder-open",
    title: t("files.emptyTitle"),
    description: t("files.emptyDescription"),
  };
});

watch(
  () => [props.activeSession?.id, props.activeSession?.status],
  () => {
    const session = props.activeSession;
    if (!session) return;

    const state = fileExplorerStore.ensureSessionState(session);
    pathInput.value = state.currentPath;

    if (session.status === "connected" && !state.hasLoaded && !state.isLoading) {
      fileExplorerStore.loadDirectory(session, state.currentPath, { pushHistory: false });
    }
  },
  { immediate: true },
);

watch(
  currentPath,
  (path) => {
    pathInput.value = path;
    rowVirtualizer.value.scrollToOffset(0);
  },
  { immediate: true },
);

watch(
  () =>
    transferStore.tasks
      .filter(
        (task) =>
          task.sessionId === props.activeSession?.id &&
          task.direction === "upload" &&
          task.status === "completed",
      )
      .map((task) => `${task.id}:${task.updatedAt}`)
      .join("|"),
  (marker, previousMarker) => {
    if (!marker || marker === previousMarker || !props.activeSession || !isConnected.value) return;

    fileExplorerStore.refreshDirectory(props.activeSession);
  },
);

watch(
  () =>
    transferStore.tasks
      .filter(
        (task) =>
          task.sessionId === props.activeSession?.id &&
          task.status === "failed" &&
          !task.conflictPrompted &&
          isTransferConflictError(task.error),
      )
      .map((task) => `${task.id}:${task.updatedAt}`)
      .join("|"),
  () => {
    if (conflictDialog.open) return;

    const task = transferStore.tasks.find(
      (task) =>
        task.sessionId === props.activeSession?.id &&
        task.status === "failed" &&
        !task.conflictPrompted &&
        isTransferConflictError(task.error),
    );

    if (task) {
      openTransferConflictDialog(task);
    }
  },
);

onMounted(() => {
  window.addEventListener("keydown", handleWindowKeydown);
  registerDragDrop();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWindowKeydown);
  unlistenDragDrop?.();
});

function submitPath() {
  if (!props.activeSession || !canNavigate.value) return;

  fileExplorerStore.loadDirectory(props.activeSession, pathInput.value);
}

function refreshDirectory() {
  closeContextMenu();
  if (!props.activeSession || !canRefresh.value) return;

  if (!isConnected.value) {
    sessionStore.reconnectSession(props.activeSession.id);
    return;
  }

  fileExplorerStore.refreshDirectory(props.activeSession);
}

function focusPathInput() {
  pathInputControl.value?.focus();
  pathInputControl.value?.select();
}

function focusSearchInput() {
  const input = searchInputControl.value?.inputRef;

  input?.focus();
  input?.select();
}

function goBack() {
  if (!props.activeSession || !canNavigate.value || !canGoBack.value) return;

  fileExplorerStore.goBack(props.activeSession);
}

function goForward() {
  if (!props.activeSession || !canNavigate.value || !canGoForward.value) return;

  fileExplorerStore.goForward(props.activeSession);
}

function goToParent() {
  if (!props.activeSession || !canNavigate.value || currentPath.value === "/") return;

  fileExplorerStore.goToParent(props.activeSession);
}

function selectEntry(entry, event) {
  const sessionId = props.activeSession?.id;
  if (!sessionId) return;

  if (event?.shiftKey && lastSelectedPath.value) {
    fileExplorerStore.setSelection(sessionId, entryRangePaths(entry.path));
  } else if (event?.ctrlKey || event?.metaKey) {
    fileExplorerStore.toggleEntrySelection(sessionId, entry.path);
  } else {
    fileExplorerStore.selectEntry(sessionId, entry.path);
  }

  lastSelectedPath.value = entry.path;
}

function openEntry(entry) {
  const sessionId = props.activeSession?.id;
  if (sessionId) {
    fileExplorerStore.selectEntry(sessionId, entry.path);
    lastSelectedPath.value = entry.path;
  }

  closeContextMenu();
  fileExplorerStore.openEntry(props.activeSession, entry);
}

function isSelected(entry) {
  return selectedPathSet.value.has(entry.path);
}

async function chooseUploadFiles() {
  closeContextMenu();
  if (!canTransfer.value) return;

  try {
    const selected = await open({
      multiple: true,
      directory: false,
      title: t("files.chooseUploadFiles"),
    });

    await startUploadPaths(normalizeDialogSelection(selected));
  } catch (error) {
    uiStore.setError(error);
  }
}

async function chooseUploadFolder() {
  closeContextMenu();
  if (!canTransfer.value) return;

  try {
    const selected = await open({
      multiple: true,
      directory: true,
      title: t("files.chooseUploadFolder"),
    });

    await startUploadPaths(normalizeDialogSelection(selected));
  } catch (error) {
    uiStore.setError(error);
  }
}

async function downloadSelectedEntries() {
  await downloadEntries(selectedEntries.value);
}

async function downloadContextEntries() {
  await downloadEntries(contextEntries.value);
}

async function downloadEntries(entries) {
  const remoteEntries = entries.filter(Boolean);
  closeContextMenu();
  if (!canTransfer.value || !remoteEntries.length) return;

  try {
    const selected = await open({
      multiple: false,
      directory: true,
      title: t("files.chooseDownloadDir"),
    });
    const [localDir] = normalizeDialogSelection(selected);

    if (!localDir) return;

    await transferStore.downloadEntries(props.activeSession, remoteEntries, localDir);
  } catch (error) {
    uiStore.setError(error);
  }
}

async function startUploadPaths(paths) {
  const localPaths = normalizeDialogSelection(paths);
  if (!canTransfer.value || !localPaths.length) return;

  await transferStore.uploadPaths(props.activeSession, localPaths, currentPath.value);
}

function openCreateFolderDialog() {
  closeContextMenu();
  if (!canTransfer.value) return;

  nameDialog.open = true;
  nameDialog.mode = "create";
  nameDialog.entry = null;
  nameDialog.value = "";
  nameDialog.error = "";
}

function openRenameDialog(entry = selectedEntries.value[0]) {
  closeContextMenu();
  if (!canTransfer.value || !entry) return;

  nameDialog.open = true;
  nameDialog.mode = "rename";
  nameDialog.entry = entry;
  nameDialog.value = entry.name;
  nameDialog.error = "";
}

function closeNameDialog() {
  if (nameDialog.isSubmitting) return;

  nameDialog.open = false;
  nameDialog.entry = null;
  nameDialog.value = "";
  nameDialog.error = "";
}

async function submitNameDialog() {
  if (!props.activeSession || nameDialog.isSubmitting) return;

  const name = nameDialog.value.trim();
  if (!name) {
    nameDialog.error = t("files.nameRequired");
    return;
  }

  if (name.includes("/") || name.includes("\\")) {
    nameDialog.error = t("files.nameCannotContainSlash");
    return;
  }

  nameDialog.isSubmitting = true;
  nameDialog.error = "";

  try {
    if (nameDialog.mode === "create") {
      await fileExplorerStore.createDirectory(
        props.activeSession,
        joinRemotePath(currentPath.value, name),
      );
    } else if (nameDialog.entry) {
      await fileExplorerStore.renameEntry(
        props.activeSession,
        nameDialog.entry.path,
        joinRemotePath(parentRemotePath(nameDialog.entry.path), name),
      );
    }

    nameDialog.open = false;
    nameDialog.entry = null;
    nameDialog.value = "";
    nameDialog.error = "";
  } catch (error) {
    nameDialog.error = String(error);
  } finally {
    nameDialog.isSubmitting = false;
  }
}

function openDeleteDialog(entries = selectedEntries.value) {
  const deletableEntries = entries.filter(Boolean);
  closeContextMenu();
  if (!canTransfer.value || !deletableEntries.length) return;

  deleteDialog.open = true;
  deleteDialog.entries = deletableEntries;
  deleteDialog.recursive = false;
  deleteDialog.error = "";
}

function closeDeleteDialog() {
  if (deleteDialog.isSubmitting) return;

  deleteDialog.open = false;
  deleteDialog.entries = [];
  deleteDialog.recursive = false;
  deleteDialog.error = "";
}

async function submitDeleteDialog() {
  if (!props.activeSession || deleteDialog.isSubmitting || !deleteDialog.entries.length) return;

  deleteDialog.isSubmitting = true;
  deleteDialog.error = "";

  try {
    await fileExplorerStore.deleteEntries(
      props.activeSession,
      deleteDialog.entries,
      deleteDialog.recursive,
    );
    deleteDialog.open = false;
    deleteDialog.entries = [];
    deleteDialog.recursive = false;
    deleteDialog.error = "";
  } catch (error) {
    deleteDialog.error = String(error);
  } finally {
    deleteDialog.isSubmitting = false;
  }
}

function openTransferConflictDialog(task) {
  transferStore.markConflictPrompted(task.id);
  conflictDialog.open = true;
  conflictDialog.taskId = task.id;
  conflictDialog.error = "";
}

function closeConflictDialog() {
  if (conflictDialog.isSubmitting) return;

  conflictDialog.open = false;
  conflictDialog.taskId = null;
  conflictDialog.error = "";
}

async function resolveTransferConflict(policy) {
  const task = conflictTask.value;
  if (!task || conflictDialog.isSubmitting) return;

  if (policy === "cancel") {
    transferStore.markTaskCanceled(task.id);
    closeConflictDialog();
    return;
  }

  conflictDialog.isSubmitting = true;
  conflictDialog.error = "";

  try {
    await transferStore.retryTaskWithConflictPolicy(task.id, policy);
    conflictDialog.open = false;
    conflictDialog.taskId = null;
    conflictDialog.error = "";
  } catch (error) {
    conflictDialog.error = String(error);
  } finally {
    conflictDialog.isSubmitting = false;
  }
}

function closeContextMenu() {
  contextMenuEntry.value = null;
}

function prepareContextMenu(event) {
  if (!isConnected.value) {
    closeContextMenu();
    return;
  }

  const entry = contextEntryFromEvent(event);
  contextMenuEntry.value = entry;

  if (entry && !isSelected(entry)) {
    selectEntry(entry);
  }
}

function openContextEntry() {
  const entry = contextMenuEntry.value;
  if (!entry) return;

  openEntry(entry);
}

function renameContextEntry() {
  openRenameDialog(contextMenuEntry.value);
}

function deleteContextEntries() {
  openDeleteDialog(contextEntries.value);
}

async function copyContextPath() {
  const path = contextMenuEntry.value?.path;
  closeContextMenu();
  if (!path) return;

  try {
    await navigator.clipboard?.writeText(path);
  } catch (error) {
    uiStore.setError(error);
  }
}

function handleWindowKeydown(event) {
  if (event.key === "Escape") {
    if (nameDialog.open) closeNameDialog();
    else if (deleteDialog.open) closeDeleteDialog();
    else if (conflictDialog.open) closeConflictDialog();
    else fileExplorerStore.clearSelection(props.activeSession?.id);

    isDragOver.value = false;
    return;
  }

  if (!shouldHandleFileShortcut(event)) return;

  const key = event.key.toLowerCase();
  const isCommand = event.ctrlKey || event.metaKey;

  if (isCommand && key === "l") {
    event.preventDefault();
    focusPathInput();
    return;
  }

  if (isCommand && key === "f") {
    event.preventDefault();
    focusSearchInput();
    return;
  }

  if (isCommand && key === "r") {
    event.preventDefault();
    refreshDirectory();
    return;
  }

  if (isCommand && key === "u") {
    event.preventDefault();
    chooseUploadFiles();
    return;
  }

  if (isCommand && key === ",") {
    event.preventDefault();
    uiStore.openSettings();
    return;
  }

  if (isEditableElement(event.target)) return;

  if (event.key === "F2" && canRename.value) {
    event.preventDefault();
    openRenameDialog();
    return;
  }

  if (event.key === "Delete" && canDelete.value) {
    event.preventDefault();
    openDeleteDialog();
  }
}

async function registerDragDrop() {
  try {
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      handleDragDropPayload(event.payload);
    });
  } catch (error) {
    console.warn("[Xin Terminal] drag-drop registration failed", error);
  }
}

function handleDragDropPayload(payload) {
  if (!payload) return;

  if (payload.type === "enter" || payload.type === "over") {
    isDragOver.value = canTransfer.value;
    return;
  }

  if (payload.type === "drop") {
    isDragOver.value = false;
    startUploadPaths(payload.paths || []);
    return;
  }

  if (payload.type === "leave") {
    isDragOver.value = false;
  }
}

function entryRangePaths(targetPath) {
  const paths = visibleEntries.value.map((entry) => entry.path);
  const startIndex = paths.indexOf(lastSelectedPath.value);
  const endIndex = paths.indexOf(targetPath);

  if (startIndex < 0 || endIndex < 0) return [targetPath];

  return paths.slice(Math.min(startIndex, endIndex), Math.max(startIndex, endIndex) + 1);
}

function normalizeDialogSelection(selection) {
  if (!selection) return [];

  const values = Array.isArray(selection) ? selection : [selection];
  return values.map((value) => String(value || "").trim()).filter(Boolean);
}

function shouldHandleFileShortcut() {
  const activeElement = document.activeElement;
  if (!activeElement || activeElement === document.body) return true;

  return Boolean(fileExplorerRoot.value?.contains(activeElement));
}

function isEditableElement(target) {
  const element = target instanceof HTMLElement ? target : null;
  if (!element) return false;

  return Boolean(element.closest("input, textarea, select, [contenteditable='true']"));
}

function isTransferConflictError(error) {
  const message = String(error || "");
  return (
    message.includes("Remote target already exists") ||
    message.includes("Local target already exists") ||
    message.includes("Duplicate download target")
  );
}

function contextEntryFromEvent(event) {
  const target = event.target instanceof HTMLElement ? event.target : null;
  const row = target?.closest("[data-file-path]");
  const filePath = row?.dataset.filePath;

  if (!filePath) return null;

  return visibleEntries.value.find((entry) => entry.path === filePath) ?? null;
}

function formatSize(entry) {
  if (entry.isDirectory) return "--";
  if (entry.size == null) return "--";

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = Number(entry.size);
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const digits = unitIndex === 0 || value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unitIndex]}`;
}

// 虚拟滚动时每帧都要格式化可见行时间，缓存格式化器避免反复创建 Intl.DateTimeFormat
const modifiedFormatter = computed(
  () =>
    new Intl.DateTimeFormat(locale.value, {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }),
);

function formatModified(entry) {
  if (!entry.modifiedAt) return "--";

  return modifiedFormatter.value.format(new Date(entry.modifiedAt * 1000));
}

function fileTypeLabel(entry) {
  return t(`files.kinds.${entry.kind || "other"}`);
}
</script>

<template>
  <section ref="fileExplorerRoot" class="file-explorer" aria-label="Remote file explorer">
    <header class="file-panel-tabs">
      <div class="file-tab-group">
        <button type="button" class="file-panel-tab active">
          <UIcon name="i-lucide-folder" />
          <span>{{ t("files.fileTab") }}</span>
        </button>
        <button type="button" class="file-panel-tab">
          <UIcon name="i-lucide-terminal-square" />
          <span>{{ t("files.commandTab") }}</span>
        </button>
      </div>

      <div class="file-panel-actions">
        <UInput
          ref="searchInputControl"
          v-model="query"
          color="neutral"
          variant="outline"
          size="sm"
          type="search"
          icon="i-lucide-search"
          :placeholder="t('files.search')"
          :ui="fileSearchInputUi"
        />
        <span class="file-count">{{ t("files.itemCount", { count: visibleEntries.length }) }}</span>
        <button type="button" :aria-label="t('files.history')" :title="t('files.history')" :disabled="!hasEntries">
          <UIcon name="i-lucide-history" />
        </button>
        <button
          type="button"
          :aria-label="t('files.refresh')"
          :title="t('files.refresh')"
          :disabled="!canRefresh"
          @click="refreshDirectory"
        >
          <UIcon name="i-lucide-refresh-cw" :class="{ 'spin-icon': fileState.isLoading || isConnecting }" />
        </button>
        <button
          type="button"
          :aria-label="t('files.download')"
          :title="t('files.download')"
          :disabled="!canDownload"
          @click="downloadSelectedEntries"
        >
          <UIcon name="i-lucide-download" />
        </button>
        <button
          type="button"
          :aria-label="t('files.rename')"
          :title="t('files.rename')"
          :disabled="!canRename"
          @click="openRenameDialog()"
        >
          <UIcon name="i-lucide-pencil" />
        </button>
        <button
          type="button"
          :aria-label="t('files.delete')"
          :title="t('files.delete')"
          :disabled="!canDelete"
          @click="openDeleteDialog()"
        >
          <UIcon name="i-lucide-trash-2" />
        </button>
        <button
          type="button"
          :aria-label="t('files.uploadFiles')"
          :title="t('files.uploadFiles')"
          :disabled="!canTransfer"
          @click="chooseUploadFiles"
        >
          <UIcon name="i-lucide-upload" />
        </button>
        <button
          type="button"
          :aria-label="t('files.uploadFolder')"
          :title="t('files.uploadFolder')"
          :disabled="!canTransfer"
          @click="chooseUploadFolder"
        >
          <UIcon name="i-lucide-folder-up" />
        </button>
        <button
          type="button"
          :aria-label="t('files.newFolder')"
          :title="t('files.newFolder')"
          :disabled="!canTransfer"
          @click="openCreateFolderDialog"
        >
          <UIcon name="i-lucide-folder-plus" />
        </button>
      </div>
    </header>

    <div class="file-path-row">
      <div class="navigation-tools">
        <button type="button" :aria-label="t('files.back')" :disabled="!canNavigate || !canGoBack" @click="goBack">
          <UIcon name="i-lucide-arrow-left" />
        </button>
        <button
          type="button"
          :aria-label="t('files.forward')"
          :disabled="!canNavigate || !canGoForward"
          @click="goForward"
        >
          <UIcon name="i-lucide-arrow-right" />
        </button>
        <button
          type="button"
          :aria-label="t('files.parent')"
          :disabled="!canNavigate || currentPath === '/'"
          @click="goToParent"
        >
          <UIcon name="i-lucide-arrow-up" />
        </button>
      </div>

      <form class="path-bar" aria-label="Current remote path" @submit.prevent="submitPath">
        <UIcon name="i-lucide-map" />
        <input ref="pathInputControl" v-model="pathInput" type="text" :disabled="!isConnected" spellcheck="false" />
      </form>
    </div>

    <UContextMenu :items="contextMenuItems" size="xs" :ui="fileContextMenuUi">
      <div class="file-workbench" :class="{ 'drag-over': isDragOver }" @contextmenu="prepareContextMenu">
        <section ref="fileTableWrap" class="file-table-wrap">
          <div class="file-row file-heading">
            <span class="name-column">{{ t("files.name") }}</span>
            <span>{{ t("files.size") }}</span>
            <span>{{ t("files.type") }}</span>
            <span>{{ t("files.modified") }}</span>
            <span>{{ t("files.permissions") }}</span>
          </div>

          <div v-if="!visibleEntries.length" class="file-empty-state" :class="{ error: emptyState.isError }">
            <UIcon :name="emptyState.icon" :class="{ 'spin-icon': emptyState.spinning }" />
            <strong>{{ emptyState.title }}</strong>
            <span>{{ emptyState.description }}</span>
            <button v-if="emptyState.isError" type="button" class="retry-button" @click="refreshDirectory">
              <UIcon name="i-lucide-refresh-cw" />
              <span>{{ t("files.retry") }}</span>
            </button>
          </div>

          <div
            v-if="visibleEntries.length"
            class="file-virtual-body"
            :style="{ height: `${totalListHeight}px` }"
          >
            <button
              v-for="{ virtualRow, entry: fileEntry } in virtualEntryRows"
              :key="fileEntry.id"
              type="button"
              class="file-row file-entry file-virtual-row"
              :class="{ selected: isSelected(fileEntry), directory: fileEntry.isDirectory }"
              :style="{ height: `${virtualRow.size}px`, transform: `translateY(${virtualRow.start}px)` }"
              :data-file-path="fileEntry.path"
              @click="selectEntry(fileEntry, $event)"
              @dblclick="openEntry(fileEntry)"
              @keydown.enter.prevent="openEntry(fileEntry)"
            >
              <span class="name-column">
                <span class="file-icon" :class="{ directory: fileEntry.isDirectory, symlink: fileEntry.isSymlink }">
                  <UIcon :name="fileEntry.icon" />
                </span>
                <span class="file-name">{{ fileEntry.name }}</span>
              </span>
              <span>{{ formatSize(fileEntry) }}</span>
              <span>{{ fileTypeLabel(fileEntry) }}</span>
              <span>{{ formatModified(fileEntry) }}</span>
              <span>{{ fileEntry.permissions || "--" }}</span>
            </button>
          </div>
        </section>

        <div v-if="isDragOver" class="file-drop-overlay">
          <UIcon name="i-lucide-cloud-upload" />
          <strong>{{ t("files.dropUploadTitle") }}</strong>
          <span>{{ t("files.dropUploadDescription", { path: currentPath }) }}</span>
        </div>
      </div>
    </UContextMenu>

    <UModal
      v-model:open="nameDialog.open"
      :title="nameDialogTitle"
      :description="nameDialogDescription"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <form class="file-dialog-form" @submit.prevent="submitNameDialog">
          <UInput
            v-model="nameDialog.value"
            color="neutral"
            variant="outline"
            :placeholder="t('files.namePlaceholder')"
            autofocus
          />
          <UAlert
            v-if="nameDialog.error"
            color="error"
            variant="soft"
            icon="i-lucide-circle-alert"
            :title="nameDialog.error"
          />
        </form>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" :label="t('common.cancel')" @click="closeNameDialog" />
        <UButton
          color="primary"
          icon="i-lucide-check"
          :loading="nameDialog.isSubmitting"
          :label="nameDialogSubmitLabel"
          @click="submitNameDialog"
        />
      </template>
    </UModal>

    <UModal
      v-model:open="deleteDialog.open"
      :title="deleteDialogTitle"
      :description="t('files.deleteDescription')"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <div class="file-dialog-form">
          <div class="delete-preview">
            <UIcon name="i-lucide-trash-2" />
            <span>{{ deletePreviewNames }}</span>
          </div>
          <UCheckbox
            v-if="deleteHasDirectory"
            v-model="deleteDialog.recursive"
            :label="t('files.recursiveDelete')"
            :description="t('files.recursiveDeleteDescription')"
          />
          <UAlert
            v-if="deleteDialog.error"
            color="error"
            variant="soft"
            icon="i-lucide-circle-alert"
            :title="deleteDialog.error"
          />
        </div>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" :label="t('common.cancel')" @click="closeDeleteDialog" />
        <UButton
          color="error"
          icon="i-lucide-trash-2"
          :loading="deleteDialog.isSubmitting"
          :label="t('files.delete')"
          @click="submitDeleteDialog"
        />
      </template>
    </UModal>

    <UModal
      v-model:open="conflictDialog.open"
      :title="conflictDialogTitle"
      :description="conflictDialogDescription"
      :ui="{ footer: 'justify-end' }"
    >
      <template #body>
        <div class="file-dialog-form">
          <UAlert
            color="warning"
            variant="soft"
            icon="i-lucide-copy-x"
            :title="t('files.conflictSummary')"
            :description="t('files.conflictDescription')"
          />
          <UAlert
            v-if="conflictDialog.error"
            color="error"
            variant="soft"
            icon="i-lucide-circle-alert"
            :title="conflictDialog.error"
          />
        </div>
      </template>
      <template #footer>
        <UButton
          color="neutral"
          variant="outline"
          :disabled="conflictDialog.isSubmitting"
          :label="t('files.cancelTask')"
          @click="resolveTransferConflict('cancel')"
        />
        <UButton
          color="neutral"
          variant="soft"
          :disabled="conflictDialog.isSubmitting"
          :label="t('files.skipConflicts')"
          @click="resolveTransferConflict('skip')"
        />
        <UButton
          color="neutral"
          variant="soft"
          :disabled="conflictDialog.isSubmitting"
          :label="t('files.renameConflicts')"
          @click="resolveTransferConflict('rename')"
        />
        <UButton
          color="primary"
          icon="i-lucide-copy-check"
          :loading="conflictDialog.isSubmitting"
          :label="t('files.overwriteConflicts')"
          @click="resolveTransferConflict('overwrite')"
        />
      </template>
    </UModal>
  </section>
</template>

<style scoped src="../assets/css/file-explorer.css"></style>
