<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { Pane, Splitpanes } from "splitpanes";
import { useI18n } from "vue-i18n";
import FilePermissionsEditor from "./FilePermissionsEditor.vue";
import { useFileExplorerStore } from "../stores/fileExplorerStore";
import { useFileOperationStore } from "../stores/fileOperationStore";
import { useEditorStore } from "../stores/editorStore";
import { useSettingsStore } from "../stores/settingsStore";
import { useTransferStore } from "../stores/transferStore";
import { useUiStore } from "../stores/uiStore";
import { useSessionStore } from "../stores/sessionStore";
import { TRANSFER_CONFLICT_CODES, hasErrorCode, localizeError } from "../utils/appError";
import { joinRemotePath, parentRemotePath, quoteRemotePathForShell } from "../utils/remotePath";

const props = defineProps({
  activeSession: {
    type: Object,
    default: null,
  },
});

const { t, locale } = useI18n();
const fileExplorerStore = useFileExplorerStore();
const fileOperationStore = useFileOperationStore();
const editorStore = useEditorStore();
const settingsStore = useSettingsStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();
const sessionStore = useSessionStore();
const fileExplorerRoot = ref(null);
const fileTableWrap = ref(null);
const pathInput = ref("/");
const pathInputControl = ref(null);
const isEditingPath = ref(false);
const searchInputControl = ref(null);
const isDragOver = ref(false);
const lastSelectedPath = ref("");
const contextMenuEntry = ref(null);
const treeContextNode = ref(null);
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
const remoteOperationDialog = reactive({
  open: false,
  mode: "copy",
  entry: null,
  targetPath: "",
  permissions: "",
  error: "",
  isSubmitting: false,
});
let showOperationsAfterDelete = false;
const conflictDialog = reactive({
  open: false,
  taskId: null,
  error: "",
  isSubmitting: false,
});
// 右侧留出 pe-10 给当前目录的项数角标
const fileSearchInputUi = {
  root: "file-search",
  base: "h-8 rounded-full bg-default ps-8 pe-10 text-xs font-semibold placeholder:text-dimmed",
  leading: "ps-3",
  leadingIcon: "size-4 text-muted",
  trailing: "pe-2",
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
const query = computed({
  get: () => fileState.value.query,
  set: (value) => fileExplorerStore.setQuery(props.activeSession?.id, value),
});
const sortKey = ref("name");
const sortDirection = ref("asc");
const entryCollator = computed(() => new Intl.Collator(locale.value, {
  numeric: true,
  sensitivity: "base",
}));
const visibleEntries = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  const showHidden = settingsStore.showHiddenFiles;

  const filteredEntries = fileState.value.entries.filter((entry) => {
    if (!showHidden && entry.isHidden) return false;
    if (!keyword) return true;

    return entry.name.toLowerCase().includes(keyword);
  });

  return sortEntries(filteredEntries, sortKey.value, sortDirection.value, entryCollator.value);
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
const canRemoteOperate = computed(() => canTransfer.value && selectedEntries.value.length === 1);
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
            {
              label: t("files.cdTerminal"),
              icon: "i-lucide-square-terminal",
              disabled: !canTransfer.value,
              onSelect: () => cdTerminalTo(entry.path),
            },
          ]
        : []),
      ...(entry && !entry.isDirectory
        ? [
            {
              label: t("files.edit"),
              icon: "i-lucide-file-pen-line",
              disabled: !canTransfer.value,
              onSelect: editContextEntry,
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
            {
              label: t("files.copyTo"),
              icon: "i-lucide-copy-plus",
              disabled: !canRemoteOperate.value || entries.length !== 1,
              onSelect: () => openRemoteOperationDialog("copy", entry),
            },
            {
              label: t("files.moveTo"),
              icon: "i-lucide-folder-input",
              disabled: !canRemoteOperate.value || entries.length !== 1,
              onSelect: () => openRemoteOperationDialog("move", entry),
            },
            {
              label: t("files.changePermissions"),
              icon: "i-lucide-shield-check",
              disabled: !canRemoteOperate.value || entries.length !== 1,
              onSelect: () => openRemoteOperationDialog("chmod", entry),
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
        label: t("files.newFile"),
        icon: "i-lucide-file-plus",
        disabled: !canTransfer.value,
        onSelect: openCreateFileDialog,
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
// 目录树右键菜单（独立于文件列表菜单）
const treeContextMenuItems = computed(() => {
  const node = treeContextNode.value;
  if (!node) return [];

  return [
    [
      {
        label: t("files.open"),
        icon: "i-lucide-folder-open",
        disabled: !canNavigate.value,
        onSelect: () => openTreeDirectory(node),
      },
      {
        label: t("files.cdTerminal"),
        icon: "i-lucide-square-terminal",
        disabled: !canTransfer.value,
        onSelect: () => cdTerminalTo(node.path),
      },
      {
        label: t("files.copyPath"),
        icon: "i-lucide-copy",
        onSelect: () => copyPathToClipboard(node.path),
      },
    ],
    [
      {
        label: t("files.refreshChildren"),
        icon: "i-lucide-refresh-cw",
        disabled: !canTransfer.value,
        onSelect: () => refreshTreeNode(node),
      },
    ],
  ];
});
// 目录树扁平化渲染行：只含目录，跟随隐藏文件开关
const treeRows = computed(() => {
  const nodes = fileState.value.treeNodes || {};
  const rootNode = nodes["/"];
  if (!rootNode) return [];

  const showHidden = settingsStore.showHiddenFiles;
  const rows = [];
  const visit = (node, depth) => {
    rows.push({ node, depth });
    if (!node.expanded) return;

    for (const child of node.childDirs) {
      if (!showHidden && child.isHidden) continue;

      const childNode = nodes[child.path];
      if (childNode) visit(childNode, depth + 1);
    }
  };

  visit(rootNode, 0);
  return rows;
});
const historyMenuItems = computed(() => {
  const seenPaths = new Set();
  const items = [];

  for (let index = fileState.value.history.length - 1; index >= 0; index -= 1) {
    const path = fileState.value.history[index];
    if (seenPaths.has(path)) continue;

    seenPaths.add(path);
    items.push({
      label: path,
      icon: path === currentPath.value ? "i-lucide-check" : "i-lucide-folder",
      disabled: !canNavigate.value,
      onSelect: () => fileExplorerStore.loadDirectory(props.activeSession, path),
    });

    if (items.length >= 12) break;
  }

  return [items];
});
const hasHistory = computed(() => historyMenuItems.value[0].length > 0);

// 路径太深时只留根、省略号和最后三级，工具栏一行放得下
const PATH_CRUMB_TAIL = 3;
const pathCrumbs = computed(() => {
  const segments = currentPath.value.split("/").filter(Boolean);
  const crumbs = segments.map((segment, index) => ({
    label: segment,
    path: `/${segments.slice(0, index + 1).join("/")}`,
  }));
  const tail = crumbs.length > PATH_CRUMB_TAIL + 1
    ? [{ label: "…", path: `ellipsis:${currentPath.value}`, ellipsis: true }, ...crumbs.slice(-PATH_CRUMB_TAIL)]
    : crumbs;
  return [{ label: "/", path: "/" }, ...tail];
});

const createMenuItems = computed(() => [[
  { label: t("files.newFolder"), icon: "i-lucide-folder-plus", disabled: !canTransfer.value, onSelect: openCreateFolderDialog },
  { label: t("files.newFile"), icon: "i-lucide-file-plus", disabled: !canTransfer.value, onSelect: openCreateFileDialog },
]]);

const moreMenuItems = computed(() => [
  [
    { label: t("files.uploadFolder"), icon: "i-lucide-folder-up", disabled: !canTransfer.value, onSelect: chooseUploadFolder },
    { label: t("files.download"), icon: "i-lucide-download", disabled: !canDownload.value, onSelect: downloadSelectedEntries },
    { label: t("files.rename"), icon: "i-lucide-pencil", disabled: !canRename.value, onSelect: () => openRenameDialog() },
    {
      label: t("files.delete"),
      icon: "i-lucide-trash-2",
      color: "error",
      disabled: !canDelete.value,
      onSelect: () => openDeleteDialog(),
    },
  ],
  [
    { label: t("files.refresh"), icon: "i-lucide-refresh-cw", disabled: !canRefresh.value, onSelect: refreshDirectory },
    { label: t("files.history"), icon: "i-lucide-history", disabled: !hasHistory.value, children: historyMenuItems.value },
    {
      label: t(settingsStore.showHiddenFiles ? "files.hideHidden" : "files.showHidden"),
      icon: settingsStore.showHiddenFiles ? "i-lucide-eye-off" : "i-lucide-eye",
      onSelect: () => settingsStore.toggleHiddenFiles(),
    },
  ],
]);
const nameDialogTitle = computed(() => {
  if (nameDialog.mode === "create") return t("files.createFolderTitle");
  if (nameDialog.mode === "createFile") return t("files.createFileTitle");

  return t("files.renameTitle");
});
const nameDialogDescription = computed(() => {
  if (nameDialog.mode === "create") return t("files.createFolderDescription");
  if (nameDialog.mode === "createFile") return t("files.createFileDescription");

  return t("files.renameDescription");
});
const nameDialogSubmitLabel = computed(() => {
  if (nameDialog.mode === "create") return t("files.createFolder");
  if (nameDialog.mode === "createFile") return t("files.createFile");

  return t("files.rename");
});
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
const conflictDialogDescription = computed(() => localizeError(conflictTask.value?.error));
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

// 工作区保活后弹窗仍会传送到 body，切换目标时收起，避免在另一台主机上误提交。
watch(() => [props.activeSession?.id, props.activeSession?.currentAttemptId], () => {
  nameDialog.open = false;
  deleteDialog.open = false;
  closeRemoteOperationDialog(true);
  conflictDialog.open = false;
  contextMenuEntry.value = null;
  treeContextNode.value = null;
  isDragOver.value = false;
});

watch(
  () => [props.activeSession?.id, props.activeSession?.currentAttemptId, props.activeSession?.status],
  () => {
    const session = props.activeSession;
    if (!session) return;

    const state = fileExplorerStore.ensureSessionState(session);
    if (!state) return;
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
    fileExplorerStore.refreshTreeChildren(props.activeSession, currentPath.value);
  },
);

// 当前路径变化时让目录树展开到对应位置（树可见且已连接才加载）
watch(
  () => [props.activeSession?.id, currentPath.value, isConnected.value, settingsStore.showFileTree],
  ([, path, connected, treeVisible]) => {
    if (!props.activeSession || !connected || !treeVisible) return;

    fileExplorerStore.revealTreePath(props.activeSession, path);
  },
  { immediate: true },
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

  isEditingPath.value = false;
  fileExplorerStore.loadDirectory(props.activeSession, pathInput.value);
}

function navigateToPath(path) {
  if (!props.activeSession || !canNavigate.value || path === currentPath.value) return;

  fileExplorerStore.loadDirectory(props.activeSession, path);
}

// 点面包屑本身是跳转，点它旁边的空白才进入手输
function startPathEdit(event) {
  if (isEditingPath.value || !isConnected.value || event.target.closest(".path-crumb")) return;
  focusPathInput();
}

function cancelPathEdit() {
  isEditingPath.value = false;
  pathInput.value = currentPath.value;
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

async function focusPathInput() {
  isEditingPath.value = true;
  await nextTick();
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

function openCreateFileDialog() {
  closeContextMenu();
  if (!canTransfer.value) return;

  nameDialog.open = true;
  nameDialog.mode = "createFile";
  nameDialog.entry = null;
  nameDialog.value = "";
  nameDialog.error = "";
}

// ---- 目录树 ----
function toggleTreeRow(node) {
  if (!isConnected.value) return;

  fileExplorerStore.toggleTreeNode(props.activeSession, node.path);
}

function openTreeDirectory(node) {
  closeTreeContextMenu();
  if (!canNavigate.value) return;

  fileExplorerStore.loadDirectory(props.activeSession, node.path);
  if (!node.expanded) {
    fileExplorerStore.toggleTreeNode(props.activeSession, node.path);
  }
}

function refreshTreeNode(node) {
  closeTreeContextMenu();
  if (!isConnected.value) return;

  fileExplorerStore.loadTreeChildren(props.activeSession, node.path, { force: true });
}

function prepareTreeContextMenu(event) {
  if (!isConnected.value) {
    closeTreeContextMenu();
    return;
  }

  const target = event.target instanceof HTMLElement ? event.target : null;
  const row = target?.closest("[data-tree-path]");
  const treePath = row?.dataset.treePath;

  treeContextNode.value = treePath
    ? fileState.value.treeNodes[treePath] ?? null
    : null;
}

function closeTreeContextMenu() {
  treeContextNode.value = null;
}

// 向当前会话的终端发送 cd，让终端与文件管理器进入同一目录
async function cdTerminalTo(path) {
  closeContextMenu();
  closeTreeContextMenu();
  if (!isConnected.value || !path) return;

  try {
    await sessionStore.sendActiveTerminalInput(`cd ${quoteRemotePathForShell(path)}\n`);
  } catch (error) {
    uiStore.setError(error);
  }
}

async function copyPathToClipboard(path) {
  closeTreeContextMenu();
  if (!path) return;

  try {
    await navigator.clipboard?.writeText(path);
  } catch (error) {
    uiStore.setError(error);
  }
}

// 树/列表分割拖拽结束后持久化占比
function persistTreeSize(event) {
  const panes = Array.isArray(event) ? event : event?.panes ?? [];
  if (!settingsStore.showFileTree || panes.length < 2) return;

  settingsStore.applySettingsPatch({ fileTreeSize: panes[0].size });
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

const remoteOperationTitle = computed(() => {
  if (remoteOperationDialog.mode === "copy") return t("files.copyToTitle");
  if (remoteOperationDialog.mode === "move") return t("files.moveToTitle");
  return t("files.changePermissionsTitle");
});
const remoteOperationDescription = computed(() => {
  if (remoteOperationDialog.mode === "chmod") return remoteOperationDialog.entry?.path ?? "";
  return t("files.remoteTargetDescription");
});
const remoteOperationSubmitLabel = computed(() => {
  if (remoteOperationDialog.mode === "copy") return t("files.copyTo");
  if (remoteOperationDialog.mode === "move") return t("files.moveTo");
  return t("files.changePermissions");
});

function openRemoteOperationDialog(mode, entry = selectedEntries.value[0]) {
  closeContextMenu();
  if (!canTransfer.value || !entry || selectedEntries.value.length > 1) return;

  remoteOperationDialog.open = true;
  remoteOperationDialog.mode = mode;
  remoteOperationDialog.entry = entry;
  remoteOperationDialog.targetPath = "";
  remoteOperationDialog.permissions = entry.permissionsMode == null
    ? ""
    : entry.permissionsMode.toString(8).padStart(3, "0");
  remoteOperationDialog.error = "";
}

function closeRemoteOperationDialog(force = false) {
  if (remoteOperationDialog.isSubmitting && !force) return;
  remoteOperationDialog.open = false;
  remoteOperationDialog.entry = null;
  remoteOperationDialog.targetPath = "";
  remoteOperationDialog.permissions = "";
  remoteOperationDialog.error = "";
}

async function submitRemoteOperation() {
  // 先在界面拦截明显无效的路径和权限值，避免把可预见的输入错误送到远端。
  const entry = remoteOperationDialog.entry;
  if (!props.activeSession || !entry || remoteOperationDialog.isSubmitting) return;

  const mode = remoteOperationDialog.mode;
  if (mode === "chmod") {
    if (!/^[0-7]{3}$/.test(remoteOperationDialog.permissions)) {
      remoteOperationDialog.error = t("files.permissionsInvalid");
      return;
    }
  } else if (!remoteOperationDialog.targetPath.startsWith("/") || remoteOperationDialog.targetPath.includes("\0")) {
    remoteOperationDialog.error = t("files.remotePathRequired");
    return;
  }

  remoteOperationDialog.isSubmitting = true;
  remoteOperationDialog.error = "";
  try {
    if (mode === "copy") {
      await fileExplorerStore.copyEntry(props.activeSession, entry.path, remoteOperationDialog.targetPath, entry.isDirectory);
    } else if (mode === "move") {
      await fileExplorerStore.renameEntry(props.activeSession, entry.path, remoteOperationDialog.targetPath);
    } else {
      await fileExplorerStore.chmodEntry(props.activeSession, entry.path, Number.parseInt(remoteOperationDialog.permissions, 8));
    }
    closeRemoteOperationDialog(true);
  } catch (error) {
    remoteOperationDialog.error = localizeError(error);
  } finally {
    remoteOperationDialog.isSubmitting = false;
  }
}

async function submitNameDialog() {
  if (!props.activeSession || nameDialog.isSubmitting) return;

  const name = nameDialog.value;
  if (!name) {
    nameDialog.error = t("files.nameRequired");
    return;
  }

  if (name.includes("/") || name.includes("\0") || name === "." || name === "..") {
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
    } else if (nameDialog.mode === "createFile") {
      await fileExplorerStore.createFile(
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
    nameDialog.error = localizeError(error);
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
    await fileOperationStore.startDelete(
      props.activeSession,
      deleteDialog.entries,
      deleteDialog.recursive,
    );
    showOperationsAfterDelete = true;
    deleteDialog.open = false;
    deleteDialog.entries = [];
    deleteDialog.recursive = false;
    deleteDialog.error = "";
  } catch (error) {
    deleteDialog.error = localizeError(error);
  } finally {
    deleteDialog.isSubmitting = false;
  }
}

function handleDeleteDialogLeave() {
  if (!showOperationsAfterDelete) return;
  showOperationsAfterDelete = false;
  // 退场动画完成后焦点锁才释放，DOM 更新完成并不代表浮层已经退出。
  fileOperationStore.panelOpen = true;
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
    conflictDialog.error = localizeError(error);
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

function editContextEntry() {
  const entry = contextMenuEntry.value;
  closeContextMenu();
  if (!entry || !canTransfer.value) return;

  editorStore.openFile(props.activeSession, entry);
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
  if (!props.activeSession || uiStore.isWindowCloseOpen || event.defaultPrevented) return;
  if (event.key === "Escape") {
    if (nameDialog.open) closeNameDialog();
    else if (deleteDialog.open) closeDeleteDialog();
    else if (remoteOperationDialog.open) closeRemoteOperationDialog();
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
    console.warn("[Xin SSH] drag-drop registration failed", error);
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
  return values.map((value) => String(value || "")).filter(Boolean);
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
  return hasErrorCode(error, TRANSFER_CONFLICT_CODES);
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

// 目录始终置顶，用户切换列时仍能先定位可展开的远程路径；缺失元数据统一排在该列末尾。
function sortEntries(entries, key, direction, collator) {
  const multiplier = direction === "desc" ? -1 : 1;
  return [...entries].sort((left, right) => {
    if (left.isDirectory !== right.isDirectory) return left.isDirectory ? -1 : 1;

    const leftValue = entrySortValue(left, key);
    const rightValue = entrySortValue(right, key);
    const leftMissing = leftValue == null;
    const rightMissing = rightValue == null;
    if (leftMissing !== rightMissing) return leftMissing ? 1 : -1;

    const comparison = leftMissing
      ? 0
      : typeof leftValue === "string"
        ? collator.compare(leftValue, rightValue)
        : leftValue - rightValue;
    if (comparison !== 0) return comparison * multiplier;

    const nameComparison = collator.compare(left.name, right.name);
    return nameComparison || collator.compare(left.path, right.path);
  });
}

function entrySortValue(entry, key) {
  if (key === "size") {
    if (entry.size == null) return null;
    const size = Number(entry.size);
    return Number.isFinite(size) ? size : null;
  }
  if (key === "modified") {
    if (entry.modifiedAt == null) return null;
    const modifiedAt = Number(entry.modifiedAt);
    return Number.isFinite(modifiedAt) ? modifiedAt : null;
  }
  if (key === "type") return entry.kind || "other";
  return entry.name;
}

function toggleSort(nextKey) {
  if (sortKey.value === nextKey) {
    sortDirection.value = sortDirection.value === "asc" ? "desc" : "asc";
    return;
  }

  sortKey.value = nextKey;
  sortDirection.value = "asc";
}

function sortIcon(key) {
  if (sortKey.value !== key) return "i-lucide-arrow-up-down";
  return sortDirection.value === "asc" ? "i-lucide-arrow-up" : "i-lucide-arrow-down";
}

function sortAriaSort(key) {
  if (sortKey.value !== key) return "none";
  return sortDirection.value === "asc" ? "ascending" : "descending";
}

function sortAriaLabel(key) {
  const field = t(`files.${key === "modified" ? "modified" : key}`);
  const direction = sortKey.value === key && sortDirection.value === "desc"
    ? t("files.sortDescending") : t("files.sortAscending");
  return t("files.sortBy", { field, direction });
}
</script>

<template>
  <section ref="fileExplorerRoot" class="file-explorer liquid-rim" aria-label="Remote file explorer">
    <!-- 一行工具栏：标题 · 目录树开关 · 前进后退 · 面包屑路径 ‖ 筛选 · 上传 · 新建 · 更多。
         低频操作（上传文件夹、下载、重命名、删除、刷新、历史、隐藏文件）收进「更多」，右键菜单保留全部 -->
    <header class="file-toolbar">
      <h3 class="file-toolbar-title">{{ t("files.fileTab") }}</h3>

      <UButton
        class="file-tool"
        :class="{ active: settingsStore.showFileTree }"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        icon="i-lucide-panel-left"
        :aria-pressed="settingsStore.showFileTree"
        :aria-label="t(settingsStore.showFileTree ? 'files.hideTree' : 'files.showTree')"
        :title="t(settingsStore.showFileTree ? 'files.hideTree' : 'files.showTree')"
        @click="settingsStore.toggleFileTree()"
      />

      <div class="navigation-tools">
        <UButton
          class="file-tool"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          icon="i-lucide-chevron-left"
          :aria-label="t('files.back')"
          :title="t('files.back')"
          :disabled="!canNavigate || !canGoBack"
          @click="goBack"
        />
        <UButton
          class="file-tool"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          icon="i-lucide-chevron-right"
          :aria-label="t('files.forward')"
          :title="t('files.forward')"
          :disabled="!canNavigate || !canGoForward"
          @click="goForward"
        />
        <UButton
          class="file-tool"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          icon="i-lucide-arrow-up"
          :aria-label="t('files.parent')"
          :title="t('files.parent')"
          :disabled="!canNavigate || currentPath === '/'"
          @click="goToParent"
        />
      </div>

      <!-- 平时是可点的面包屑；点空白处 / Ctrl+L 进入输入模式，回车跳转，Esc 或失焦退出 -->
      <form
        class="path-bar"
        :class="{ editing: isEditingPath }"
        aria-label="Current remote path"
        @submit.prevent="submitPath"
        @click="startPathEdit"
      >
        <input
          v-if="isEditingPath"
          ref="pathInputControl"
          v-model="pathInput"
          type="text"
          spellcheck="false"
          :disabled="!isConnected"
          @blur="cancelPathEdit"
          @keydown.esc.prevent="cancelPathEdit"
        />
        <nav v-else class="path-crumbs" :aria-label="t('files.pathCrumbs')">
          <template v-for="(crumb, index) in pathCrumbs" :key="crumb.path">
            <span v-if="index > 1" class="path-separator" aria-hidden="true">/</span>
            <button
              type="button"
              class="path-crumb"
              :class="{ current: crumb.path === currentPath, ellipsis: crumb.ellipsis }"
              :disabled="!canNavigate || crumb.ellipsis"
              :title="crumb.ellipsis ? currentPath : crumb.path"
              @click.stop="navigateToPath(crumb.path)"
            >
              {{ crumb.label }}
            </button>
          </template>
        </nav>
      </form>

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
      >
        <template #trailing>
          <span class="file-count" :title="t('files.itemCount', { count: visibleEntries.length })">
            {{ visibleEntries.length }}
          </span>
        </template>
      </UInput>

      <UButton
        v-liquid-glass="{ scale: 12 }"
        class="file-upload liquid-glass liquid-glass--ink"
        color="primary"
        size="sm"
        icon="i-lucide-upload"
        :label="t('files.upload')"
        :title="t('files.uploadFiles')"
        :disabled="!canTransfer"
        @click="chooseUploadFiles"
      />

      <UDropdownMenu :items="createMenuItems" size="sm" :content="{ align: 'end' }">
        <UButton
          class="file-create"
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-lucide-plus"
          trailing-icon="i-lucide-chevron-down"
          :label="t('files.create')"
          :disabled="!canTransfer"
        />
      </UDropdownMenu>

      <UDropdownMenu :items="moreMenuItems" size="sm" :content="{ align: 'end' }">
        <UButton
          class="file-tool"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          icon="i-lucide-ellipsis"
          :aria-label="t('files.moreActions')"
          :title="t('files.moreActions')"
        />
      </UDropdownMenu>
    </header>

    <Splitpanes class="xin-split explorer-split" @resized="persistTreeSize">
      <Pane
        v-if="settingsStore.showFileTree"
        :size="settingsStore.fileTreeSize"
        min-size="12"
        max-size="46"
      >
        <UContextMenu :items="treeContextMenuItems" size="xs" :ui="fileContextMenuUi">
          <aside class="file-tree" @contextmenu="prepareTreeContextMenu">
            <template v-if="isConnected">
              <button
                v-for="row in treeRows"
                :key="row.node.path"
                type="button"
                class="tree-row"
                :class="{ active: row.node.path === currentPath }"
                :style="{ '--tree-depth': row.depth }"
                :data-tree-path="row.node.path"
                :title="row.node.path"
                @click="openTreeDirectory(row.node)"
              >
                <span
                  class="tree-caret"
                  :class="{
                    expanded: row.node.expanded,
                    leaf: row.node.loaded && !row.node.childDirs.length,
                  }"
                  @click.stop="toggleTreeRow(row.node)"
                >
                  <UIcon
                    :name="row.node.loading ? 'i-lucide-loader-circle' : 'i-lucide-chevron-right'"
                    :class="{ 'spin-icon': row.node.loading }"
                  />
                </span>
                <UIcon
                  class="tree-folder-icon"
                  :name="row.node.expanded ? 'i-lucide-folder-open' : 'i-lucide-folder'"
                />
                <span class="tree-name">{{ row.node.name }}</span>
              </button>
            </template>

            <div v-else class="tree-empty">
              <UIcon name="i-lucide-list-tree" />
              <span>{{ t("files.treeEmpty") }}</span>
            </div>
          </aside>
        </UContextMenu>
      </Pane>

      <Pane :size="settingsStore.showFileTree ? 100 - settingsStore.fileTreeSize : 100">
        <UContextMenu :items="contextMenuItems" size="xs" :ui="fileContextMenuUi">
          <div class="file-workbench" :class="{ 'drag-over': isDragOver }" @contextmenu="prepareContextMenu">
        <section ref="fileTableWrap" class="file-table-wrap">
          <div class="file-row file-heading">
            <button
              type="button"
              class="file-sort-button name-column"
              :aria-label="sortAriaLabel('name')"
              :aria-sort="sortAriaSort('name')"
              @click="toggleSort('name')"
            >
              <span>{{ t("files.name") }}</span>
              <UIcon :name="sortIcon('name')" />
            </button>
            <button
              type="button"
              class="file-sort-button"
              :aria-label="sortAriaLabel('size')"
              :aria-sort="sortAriaSort('size')"
              @click="toggleSort('size')"
            >
              <span>{{ t("files.size") }}</span>
              <UIcon :name="sortIcon('size')" />
            </button>
            <button
              type="button"
              class="file-sort-button"
              :aria-label="sortAriaLabel('type')"
              :aria-sort="sortAriaSort('type')"
              @click="toggleSort('type')"
            >
              <span>{{ t("files.type") }}</span>
              <UIcon :name="sortIcon('type')" />
            </button>
            <button
              type="button"
              class="file-sort-button"
              :aria-label="sortAriaLabel('modified')"
              :aria-sort="sortAriaSort('modified')"
              @click="toggleSort('modified')"
            >
              <span>{{ t("files.modified") }}</span>
              <UIcon :name="sortIcon('modified')" />
            </button>
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
      </Pane>
    </Splitpanes>

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
      @after:leave="handleDeleteDialogLeave"
      :dismissible="!deleteDialog.isSubmitting"
      :close="!deleteDialog.isSubmitting"
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
            :disabled="deleteDialog.isSubmitting"
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
        <UButton color="neutral" variant="outline" :label="t('common.cancel')" :disabled="deleteDialog.isSubmitting" @click="closeDeleteDialog" />
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
      v-model:open="remoteOperationDialog.open"
      :title="remoteOperationTitle"
      :description="remoteOperationDescription"
      :dismissible="!remoteOperationDialog.isSubmitting"
      :close="!remoteOperationDialog.isSubmitting"
      :ui="{ description: 'break-all', footer: 'justify-end' }"
    >
      <template #body>
        <form class="file-dialog-form" @submit.prevent="submitRemoteOperation">
          <UInput
            v-if="remoteOperationDialog.mode !== 'chmod'"
            v-model="remoteOperationDialog.targetPath"
            color="neutral"
            variant="outline"
            :placeholder="t('files.remotePathPlaceholder')"
            autofocus
          />
          <FilePermissionsEditor
            v-else
            :key="remoteOperationDialog.entry?.path"
            v-model="remoteOperationDialog.permissions"
            :is-directory="remoteOperationDialog.entry?.isDirectory"
            :disabled="remoteOperationDialog.isSubmitting"
            @update:model-value="remoteOperationDialog.error = ''"
          />
          <UAlert
            v-if="remoteOperationDialog.error"
            color="error"
            variant="soft"
            icon="i-lucide-circle-alert"
            :title="remoteOperationDialog.error"
          />
        </form>
      </template>
      <template #footer>
        <UButton color="neutral" variant="outline" :label="t('common.cancel')" :disabled="remoteOperationDialog.isSubmitting" @click="closeRemoteOperationDialog" />
        <UButton
          color="primary"
          icon="i-lucide-check"
          :loading="remoteOperationDialog.isSubmitting"
          :label="remoteOperationSubmitLabel"
          @click="submitRemoteOperation"
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
          variant="soft"
          icon="i-lucide-play"
          :disabled="conflictDialog.isSubmitting"
          :label="t('files.resumeConflicts')"
          @click="resolveTransferConflict('resume')"
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
