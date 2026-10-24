<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { Pane, Splitpanes } from "splitpanes";
import { useI18n } from "vue-i18n";
import { useToast } from "@nuxt/ui/composables";
import FilePermissionsEditor from "./FilePermissionsEditor.vue";
import ClipboardPasteDialog from "./FileExplorer/ClipboardPasteDialog.vue";
import ContainerPicker from "./FileExplorer/ContainerPicker.vue";
import DiskUsageSummary from "./FileExplorer/DiskUsageSummary.vue";
import { useDiskUsageStore } from "../stores/diskUsageStore";
import { useFileExplorerStore } from "../stores/fileExplorerStore";
import { useFileOperationStore } from "../stores/fileOperationStore";
import { useEditorStore } from "../stores/editorStore";
import { useSettingsStore } from "../stores/settingsStore";
import { useTransferStore } from "../stores/transferStore";
import { useUiStore } from "../stores/uiStore";
import { useSessionStore } from "../stores/sessionStore";
import { readClipboard } from "../services/workspaceApi";
import { TRANSFER_CONFLICT_CODES, hasErrorCode, localizeError } from "../utils/appError";
import {
  containerInnerPath,
  formatPathsForShell,
  joinRemotePath,
  parentRemotePath,
  processRootOf,
  quoteRemotePathForShell,
} from "../utils/remotePath";
import { TERMINAL_CWD_STATUS } from "../utils/terminalCwd";
import { compactMenuUi } from "../config/menuUi";
import { formatBytes } from "../utils/formatMetrics";
import { describePermissions, summarizePermissions } from "../utils/filePermissions";
import { COMPRESS_FORMATS, archiveStem, defaultArchiveName, isSingleFileGzip } from "../utils/archive";
import {
  highlightTerminalDrop,
  terminalDropTargetAt,
  useFileDragToTerminal,
} from "../composables/useFileDragToTerminal";

const props = defineProps({
  activeSession: {
    type: Object,
    default: null,
  },
});
// 要插到终端命令行的文字（拖到终端上的路径）和它属于哪个会话，由 TerminalWorkspace 交给那个会话的终端
const emit = defineEmits(["insert-terminal-text"]);

const { t, locale } = useI18n();
const toast = useToast();
const fileExplorerStore = useFileExplorerStore();
const diskUsageStore = useDiskUsageStore();
const fileOperationStore = useFileOperationStore();
const editorStore = useEditorStore();
const settingsStore = useSettingsStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();
const sessionStore = useSessionStore();
const fileExplorerRoot = ref(null);
const fileTableWrap = ref(null);
const fileTableHead = ref(null);
const pathInput = ref("/");
const pathInputControl = ref(null);
const isEditingPath = ref(false);
const searchInputControl = ref(null);
const isDragOver = ref(false);
const lastSelectedPath = ref("");
const contextMenuEntry = ref(null);
const treeContextNode = ref(null);
const containerPickerOpen = ref(false);
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
// Ctrl+V 粘贴截图 / 文字时的保存弹窗；复制的文件不进弹窗，直接上传
const pasteDialog = reactive({
  open: false,
  content: null,
  directory: "/",
});
const conflictDialog = reactive({
  open: false,
  taskId: null,
  error: "",
  isSubmitting: false,
});
// 右侧留出 pe-10 给当前目录的项数角标
const fileSearchInputUi = {
  root: "file-search",
  base: "h-full rounded-full bg-transparent ps-9 pe-11 text-xs font-semibold placeholder:text-dimmed",
  leading: "ps-3.5",
  leadingIcon: "size-4 text-muted",
  trailing: "pe-2.5",
};

let unlistenDragDrop = null;

const fileState = computed(() => fileExplorerStore.getSessionState(props.activeSession?.id));
const currentPath = computed(() => fileState.value.currentPath || props.activeSession?.remotePath || "/");
const isConnected = computed(() => props.activeSession?.status === "connected");
const isConnecting = computed(() => props.activeSession?.status === "connecting");
const canNavigate = computed(() => isConnected.value && !fileState.value.isLoading);
const canTransfer = computed(() => Boolean(props.activeSession) && isConnected.value);
// 路径在 /proc/<PID>/root 下就是在逛容器。没经容器列表进来（手输的路径）也一样只读，名字用 PID 顶上
const activeContainer = computed(() => {
  const root = processRootOf(currentPath.value);
  if (!root) return null;
  return fileState.value.containers?.[root]
    ?? { root, name: t("docker.unknownContainer", { pid: root.split("/")[2] }), image: "" };
});
// 容器里只读：可以浏览、查看、下载，不能上传、新建、改名、删除、压缩解压
const canModify = computed(() => canTransfer.value && !activeContainer.value);
// 拖到终端上的本地文件传到哪：终端当前所在的目录；认不出终端在哪时退回文件面板的目录
const terminalUploadDir = computed(() => {
  const cwd = props.activeSession?.terminalCwd;
  return cwd?.status === TERMINAL_CWD_STATUS.following ? cwd.path : currentPath.value;
});
const fileDrag = useFileDragToTerminal({
  resolve: describeDraggedEntries,
  onDrop: (paths) => emit("insert-terminal-text", formatPathsForShell(paths), props.activeSession?.id),
});
const canRefresh = computed(() => Boolean(props.activeSession) && !isConnecting.value && !fileState.value.isLoading);
const canGoBack = computed(() => fileState.value.historyIndex > 0);
// 跟随终端目录：开关是全局偏好，跟不跟得上看这个会话的终端（terminalCwd.js 推断，存在会话上）
const followTerminal = computed(() => settingsStore.followTerminalCwd);
const terminalCwd = computed(() => props.activeSession?.terminalCwd ?? null);
const canFollowTerminal = computed(() => terminalCwd.value?.status === "following");
const followTerminalTitle = computed(() => {
  if (!followTerminal.value) return t("files.followTerminalOff");
  if (terminalCwd.value?.status === "elsewhere") return t("files.followTerminalElsewhere");
  if (!canFollowTerminal.value) return t("files.followTerminalUnknown");
  return t("files.followTerminalOn", { path: terminalCwd.value.path });
});
const canGoForward = computed(() => fileState.value.historyIndex < fileState.value.history.length - 1);
const query = computed({
  get: () => fileState.value.query,
  set: (value) => fileExplorerStore.setQuery(props.activeSession?.id, value),
});
const sortKey = ref("name");
const sortDirection = ref("asc");
// 占用分析：开着就统计当前目录每一项占了多少盘，列表按占用排序
const diskUsageEnabled = computed(() => diskUsageStore.isEnabled(props.activeSession?.id));
const diskUsage = computed(() =>
  diskUsageEnabled.value ? diskUsageStore.resultFor(props.activeSession?.id, currentPath.value) : null,
);
// 总数要等 du 算完才知道，算的过程中先拿已算出的部分当分母，占比会随之回落
const diskUsageBase = computed(() => diskUsage.value?.totalBytes || diskUsage.value?.scannedBytes || 0);
// 排序键是组件级的，切到没开分析的标签时按名称排
const effectiveSortKey = computed(() => (sortKey.value === "usage" && !diskUsage.value ? "name" : sortKey.value));
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

  return sortEntries(filteredEntries, effectiveSortKey.value, sortDirection.value, entryCollator.value, diskUsage.value?.sizes);
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

// 表头不在滚动区里（原因见 file-explorer.css 的 .file-table-head），横向滚动时让它跟着列表走
function syncTableHeadScroll(event) {
  if (fileTableHead.value) fileTableHead.value.scrollLeft = event.target.scrollLeft;
}

// 表头和工具栏那一截也不在滚动区里：滚轮落在这儿照样滚列表，和以前表头还在滚动区里时一样
function scrollTableFromChrome(event) {
  const wrap = fileTableWrap.value;
  if (!wrap || event.ctrlKey || wrap.contains(event.target)) return;
  wrap.scrollBy({ left: event.deltaX, top: event.deltaY });
}
const selectedPathSet = computed(() => new Set(fileState.value.selectedPaths));
const selectedEntries = computed(() =>
  fileState.value.entries.filter((entry) => selectedPathSet.value.has(entry.path)),
);
const canDownload = computed(() => canTransfer.value && selectedEntries.value.length > 0);
const canRename = computed(() => canModify.value && selectedEntries.value.length === 1);
const canRemoteOperate = computed(() => canModify.value && selectedEntries.value.length === 1);
const canDelete = computed(() => canModify.value && selectedEntries.value.length > 0);
const contextEntries = computed(() => {
  const entry = contextMenuEntry.value;
  if (!entry) return selectedEntries.value;

  return isSelected(entry) && selectedEntries.value.length ? selectedEntries.value : [entry];
});
// 压缩 / 解压：选中项右键直接在服务器上打包，压缩包右键直接解开
const archiveMenuItems = computed(() => {
  const entry = contextMenuEntry.value;
  const entries = contextEntries.value;
  const items = [];
  if (!entries.length) return items;

  const archiveName = defaultArchiveName(entries, currentPath.value);
  items.push({
    label: t("archive.compress"),
    icon: "i-lucide-file-archive",
    disabled: !canModify.value,
    children: COMPRESS_FORMATS.map(({ format, extension }) => ({
      label: t("archive.compressAs", { name: `${archiveName}${extension}` }),
      onSelect: () => compressEntries(entries, format),
    })),
  });

  const stem = entry && !entry.isDirectory && entries.length === 1 ? archiveStem(entry.name) : null;
  if (!stem) return items;

  if (isSingleFileGzip(entry.name)) {
    items.push({
      label: t("archive.extractGzip", { name: stem }),
      icon: "i-lucide-package-open",
      disabled: !canModify.value,
      onSelect: () => extractEntry(entry, false),
    });
    return items;
  }

  items.push(
    {
      label: t("archive.extractToFolder", { name: stem }),
      icon: "i-lucide-package-open",
      disabled: !canModify.value,
      onSelect: () => extractEntry(entry, true),
    },
    {
      label: t("archive.extractHere"),
      icon: "i-lucide-folder-down",
      disabled: !canModify.value,
      onSelect: () => extractEntry(entry, false),
    },
  );
  return items;
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
              label: t(activeContainer.value ? "files.view" : "files.edit"),
              icon: activeContainer.value ? "i-lucide-file-search" : "i-lucide-file-pen-line",
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
              disabled: !canModify.value,
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
    archiveMenuItems.value,
    [
      {
        label: t("files.delete"),
        icon: "i-lucide-trash-2",
        color: "error",
        disabled: !canModify.value || !entries.length,
        onSelect: deleteContextEntries,
      },
    ],
    [
      {
        label: t("files.uploadFiles"),
        icon: "i-lucide-upload",
        disabled: !canModify.value,
        onSelect: chooseUploadFiles,
      },
      {
        label: t("files.uploadFolder"),
        icon: "i-lucide-folder-up",
        disabled: !canModify.value,
        onSelect: chooseUploadFolder,
      },
      {
        label: t("paste.action"),
        icon: "i-lucide-clipboard-paste",
        disabled: !canModify.value,
        onSelect: pasteFromClipboard,
      },
      {
        label: t("files.newFolder"),
        icon: "i-lucide-folder-plus",
        disabled: !canModify.value,
        onSelect: openCreateFolderDialog,
      },
      {
        label: t("files.newFile"),
        icon: "i-lucide-file-plus",
        disabled: !canModify.value,
        onSelect: openCreateFileDialog,
      },
      {
        label: t("files.refresh"),
        icon: "i-lucide-refresh-cw",
        disabled: !canRefresh.value,
        onSelect: refreshDirectory,
      },
    ],
  ].filter((group) => group.length);
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
  // 在容器里时树从容器根开始
  const rootNode = nodes[activeContainer.value?.root ?? "/"];
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
// 在容器里时第一段是容器名，后面是容器自己看到的路径
const pathCrumbs = computed(() => {
  const container = activeContainer.value;
  const base = container?.root ?? "";
  const segments = (container ? containerInnerPath(currentPath.value, base) : currentPath.value).split("/").filter(Boolean);
  const crumbs = segments.map((segment, index) => ({
    label: segment,
    path: `${base}/${segments.slice(0, index + 1).join("/")}`,
  }));
  const tail = crumbs.length > PATH_CRUMB_TAIL + 1
    ? [{ label: "…", path: `ellipsis:${currentPath.value}`, ellipsis: true }, ...crumbs.slice(-PATH_CRUMB_TAIL)]
    : crumbs;
  const rootCrumb = container
    ? { label: container.name, path: container.root, icon: "i-lucide-container" }
    : { label: "/", path: "/" };
  return [rootCrumb, ...tail];
});
// 路径栏手输时显示的路径：容器里只显示容器内的那一段
const editablePath = computed(() => {
  const container = activeContainer.value;
  return container ? containerInnerPath(currentPath.value, container.root) : currentPath.value;
});

const createMenuItems = computed(() => [[
  { label: t("files.newFolder"), icon: "i-lucide-folder-plus", disabled: !canModify.value, onSelect: openCreateFolderDialog },
  { label: t("files.newFile"), icon: "i-lucide-file-plus", disabled: !canModify.value, onSelect: openCreateFileDialog },
]]);

const moreMenuItems = computed(() => [
  [
    { label: t("files.uploadFolder"), icon: "i-lucide-folder-up", disabled: !canModify.value, onSelect: chooseUploadFolder },
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
  pasteDialog.open = false;
  containerPickerOpen.value = false;
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
    pathInput.value = editablePath.value;

    if (session.status === "connected" && !state.hasLoaded && !state.isLoading) {
      fileExplorerStore.loadDirectory(session, state.currentPath, { pushHistory: false });
    }
  },
  { immediate: true },
);

watch(
  currentPath,
  () => {
    pathInput.value = editablePath.value;
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

// 分析模式下进入一个目录就统计这一层；算过的直接用缓存
watch(
  () => [props.activeSession?.id, currentPath.value, isConnected.value],
  ([, path, connected]) => {
    if (connected) diskUsageStore.visit(props.activeSession, path);
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
  const container = activeContainer.value;
  if (!container) {
    fileExplorerStore.loadDirectory(props.activeSession, pathInput.value);
    return;
  }
  // 容器里手输的路径按容器自己的根来解析，软链接也在容器里跟随
  fileExplorerStore.openContainerPath(props.activeSession, container.root, pathInput.value)
    .catch((error) => uiStore.setError(error));
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
  pathInput.value = editablePath.value;
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

// 刚打开就先对齐一次：点它的时候，多半就是想看终端现在在哪
function toggleFollowTerminal() {
  const next = !followTerminal.value;
  settingsStore.setFollowTerminalCwd(next);
  if (next && canFollowTerminal.value && props.activeSession) {
    fileExplorerStore.followTerminal(props.activeSession, terminalCwd.value.path);
  }
}

function enterContainer(container) {
  containerPickerOpen.value = false;
  if (canNavigate.value) fileExplorerStore.enterContainer(props.activeSession, container);
}

function exitContainer() {
  containerPickerOpen.value = false;
  if (canNavigate.value) fileExplorerStore.exitContainer(props.activeSession);
}

function toggleDiskUsage() {
  const session = props.activeSession;
  if (!session) return;

  if (diskUsageEnabled.value) {
    diskUsageStore.disable(session.id);
    if (sortKey.value === "usage") {
      sortKey.value = "name";
      sortDirection.value = "asc";
    }
    return;
  }

  if (!canNavigate.value) return;
  sortKey.value = "usage";
  sortDirection.value = "desc";
  diskUsageStore.enable(session, currentPath.value);
}

function rerunDiskUsage() {
  if (canNavigate.value) diskUsageStore.analyze(props.activeSession, currentPath.value);
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
  Promise.resolve(fileExplorerStore.openEntry(props.activeSession, entry)).catch((error) => uiStore.setError(error));
}

function isSelected(entry) {
  return selectedPathSet.value.has(entry.path);
}

async function chooseUploadFiles() {
  closeContextMenu();
  if (!canModify.value) return;

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
  if (!canModify.value) return;

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
  // 容器里的软链接会被服务器按宿主机解析，下载到的是宿主机的文件，后端也会跳过它们
  const remoteEntries = entries.filter((entry) => entry && !(activeContainer.value && entry.isSymlink));
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

// 剪贴板里是复制的文件就直接上传；是截图或文字就弹窗起个名字，存成远程文件
async function pasteFromClipboard() {
  closeContextMenu();
  if (!canModify.value) return;

  try {
    const content = await readClipboard();
    if (content.files.length) {
      await startUploadPaths(content.files);
      return;
    }
    if (!content.text && !content.hasImage) {
      toast.add({
        title: t("paste.emptyTitle"),
        description: t("paste.empty"),
        icon: "i-lucide-clipboard-x",
        color: "neutral",
        duration: 3000,
      });
      return;
    }
    pasteDialog.content = content;
    pasteDialog.directory = currentPath.value;
    pasteDialog.open = true;
  } catch (error) {
    uiStore.setError(error);
  }
}

async function startUploadPaths(paths) {
  const localPaths = normalizeDialogSelection(paths);
  if (!canModify.value || !localPaths.length) return;

  await transferStore.uploadPaths(props.activeSession, localPaths, currentPath.value);
}

function openCreateFolderDialog() {
  closeContextMenu();
  if (!canModify.value) return;

  nameDialog.open = true;
  nameDialog.mode = "create";
  nameDialog.entry = null;
  nameDialog.value = "";
  nameDialog.error = "";
}

function openCreateFileDialog() {
  closeContextMenu();
  if (!canModify.value) return;

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
  if (!canModify.value || !entry) return;

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
  if (!canModify.value || !entry || selectedEntries.value.length > 1) return;

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
  if (!canModify.value || !deletableEntries.length) return;

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

  // 容器里走 openEntry：软链接先在容器里解析，文件只读打开
  if (activeContainer.value) {
    openEntry(entry);
    return;
  }
  editorStore.openFile(props.activeSession, entry);
}

// 压缩包落在被压缩项所在的目录，也就是当前目录
async function compressEntries(entries, format) {
  closeContextMenu();
  if (!canModify.value || !entries.length) return;

  try {
    await fileOperationStore.startCompress(
      props.activeSession,
      currentPath.value,
      entries,
      format,
      defaultArchiveName(entries, currentPath.value),
    );
  } catch (error) {
    uiStore.setError(error);
  }
}

async function extractEntry(entry, intoFolder) {
  closeContextMenu();
  if (!canModify.value || !entry) return;

  try {
    await fileOperationStore.startExtract(props.activeSession, entry, intoFolder);
  } catch (error) {
    uiStore.setError(error);
  }
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

  // 只认焦点真在文件面板里的 Ctrl+V：复制的文件会立刻开始上传，不能被别处的粘贴误触发
  if (isCommand && key === "v" && fileExplorerRoot.value?.contains(document.activeElement)) {
    event.preventDefault();
    pasteFromClipboard();
    return;
  }

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

// 从桌面拖进窗口的文件：落在终端上就传到终端所在的目录、传完把路径插到命令行；落在别处照旧传到文件面板当前目录
function handleDragDropPayload(payload) {
  if (!payload) return;

  if (payload.type === "leave") {
    isDragOver.value = false;
    highlightTerminalDrop(null);
    return;
  }

  // Tauri 给的是物理像素，elementFromPoint 要的是 CSS 像素
  const terminal = canTransfer.value && payload.position
    ? terminalDropTargetAt(payload.position.x / window.devicePixelRatio, payload.position.y / window.devicePixelRatio)
    : null;

  if (payload.type === "enter" || payload.type === "over") {
    isDragOver.value = canModify.value && !terminal;
    highlightTerminalDrop(terminal, t("files.dropUploadToTerminal", { path: terminalUploadDir.value }));
    return;
  }

  if (payload.type === "drop") {
    isDragOver.value = false;
    highlightTerminalDrop(null);
    if (terminal) uploadToTerminal(payload.paths || []);
    else startUploadPaths(payload.paths || []);
  }
}

// 拖到终端上的本地文件：传完再把远程路径插到命令行——还没传完就插，用户可能马上回车去用一个不完整的文件。
// 重名时用户选了「改名」，落地的名字就不是这个了，不插；选了「覆盖 / 跳过」等任务续上后照样插
async function uploadToTerminal(localPaths) {
  const paths = normalizeDialogSelection(localPaths);
  const session = props.activeSession;
  if (!canTransfer.value || !paths.length) return;

  const remoteDir = terminalUploadDir.value;
  const remotePaths = paths.map((path) => joinRemotePath(remoteDir, path.split(/[\\/]/).filter(Boolean).pop()));
  try {
    const taskId = await transferStore.uploadPaths(session, paths, remoteDir);
    const stop = watch(() => transferStore.findTask(taskId)?.status, (status) => {
      const task = transferStore.findTask(taskId);
      if (status === "failed" && isTransferConflictError(task?.error)) return;
      if (status && !["completed", "failed", "canceled", "paused"].includes(status)) return;
      stop();
      if (status === "completed" && task.conflictPolicy !== "rename") {
        emit("insert-terminal-text", formatPathsForShell(remotePaths), session.id);
      }
    });
  } catch (error) {
    uiStore.setError(error);
  }
}

// 按下的那一行要拖哪些路径：它在多选里就拖整组（按列表顺序），否则只拖它自己
function describeDraggedEntries(entry) {
  if (!canTransfer.value) return null;
  const selected = isSelected(entry) && fileState.value.selectedPaths.length > 1
    ? visibleEntries.value.filter((item) => selectedPathSet.value.has(item.path))
    : [entry];
  return {
    paths: selected.map((item) => item.path),
    label: selected.length === 1 ? selected[0].name : t("files.dragItems", { count: selected.length }),
    icon: selected.length === 1 ? selected[0].icon : "i-lucide-files",
    hint: t("files.dropPathToTerminal"),
  };
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

function entryUsage(entry) {
  return diskUsage.value?.sizes[entry.name] ?? null;
}

function formatUsage(entry) {
  const bytes = entryUsage(entry);
  if (bytes != null) return formatBytes(bytes);
  return diskUsage.value?.status === "running" ? "…" : "--";
}

function usagePercent(entry) {
  const bytes = entryUsage(entry);
  if (bytes == null || !diskUsageBase.value) return null;
  return Math.min((bytes / diskUsageBase.value) * 100, 100);
}

function formatShare(percent) {
  if (percent > 0 && percent < 0.1) return "<0.1%";
  return `${percent.toFixed(percent < 10 ? 1 : 0)}%`;
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

function permissionsSummary(entry) {
  return summarizePermissions(entry.permissionsMode, t, locale.value);
}

function permissionsDetail(entry) {
  return describePermissions(entry.permissionsMode, entry.isDirectory, t, locale.value);
}

// 目录始终置顶，用户切换列时仍能先定位可展开的远程路径；缺失元数据统一排在该列末尾。
// 按占用排时例外：要找的是「谁最大」，文件和目录混在一起比。
function sortEntries(entries, key, direction, collator, usageSizes) {
  const multiplier = direction === "desc" ? -1 : 1;
  const directoriesFirst = key !== "usage";
  return [...entries].sort((left, right) => {
    if (directoriesFirst && left.isDirectory !== right.isDirectory) return left.isDirectory ? -1 : 1;

    const leftValue = entrySortValue(left, key, usageSizes);
    const rightValue = entrySortValue(right, key, usageSizes);
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

function entrySortValue(entry, key, usageSizes) {
  if (key === "usage") return usageSizes?.[entry.name] ?? null;
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
  // 占用默认从大到小：点它就是想找最大的
  sortDirection.value = nextKey === "usage" ? "desc" : "asc";
}

function sortIcon(key) {
  if (effectiveSortKey.value !== key) return "i-lucide-arrow-up-down";
  return sortDirection.value === "asc" ? "i-lucide-arrow-up" : "i-lucide-arrow-down";
}

function sortAriaSort(key) {
  if (effectiveSortKey.value !== key) return "none";
  return sortDirection.value === "asc" ? "ascending" : "descending";
}

function sortAriaLabel(key) {
  const field = key === "usage" ? t("diskUsage.usage") : t(`files.${key === "modified" ? "modified" : key}`);
  const direction = effectiveSortKey.value === key && sortDirection.value === "desc"
    ? t("files.sortDescending") : t("files.sortAscending");
  return t("files.sortBy", { field, direction });
}
</script>

<template>
  <section ref="fileExplorerRoot" class="file-explorer liquid-rim" aria-label="Remote file explorer">
    <!-- 拖着文件往终端去时跟在指针右下方的小标签 -->
    <Teleport to="body">
      <div
        v-if="fileDrag.chip.visible"
        class="file-drag-chip"
        :style="{ transform: `translate(${fileDrag.chip.x + 14}px, ${fileDrag.chip.y + 12}px)` }"
      >
        <UIcon :name="fileDrag.chip.icon" />
        <span>{{ fileDrag.chip.label }}</span>
      </div>
    </Teleport>
    <!-- 工具栏浮在列表上：标题 · [目录树 | 后退 前进 上级] · 面包屑 ‖ 筛选 · 上传 · 新建 · 更多，每组一颗玻璃胶囊。
         目录树从它底下滚过去，玻璃边缘才有东西可折射；文件表的行只滚到表头下沿（样式见 file-explorer.css）。
         低频操作（上传文件夹、下载、重命名、删除、刷新、历史、隐藏文件）收进「更多」，右键菜单保留全部 -->
    <header class="file-toolbar">
      <h3 class="file-toolbar-title">{{ t("files.fileTab") }}</h3>

      <div v-liquid-glass="{ scale: 12 }" class="file-nav-capsule liquid-glass">
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
        <span class="file-nav-divider" aria-hidden="true"></span>
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
        <span class="file-nav-divider" aria-hidden="true"></span>
        <UButton
          class="file-tool"
          :class="{ active: followTerminal && canFollowTerminal, stalled: followTerminal && !canFollowTerminal }"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          :icon="followTerminal ? 'i-lucide-link' : 'i-lucide-unlink'"
          :aria-pressed="followTerminal"
          :aria-label="t('files.followTerminal')"
          :title="followTerminalTitle"
          @click="toggleFollowTerminal"
        />
        <UPopover v-model:open="containerPickerOpen" :content="{ align: 'start' }">
          <UButton
            class="file-tool"
            :class="{ active: activeContainer }"
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-container"
            :aria-label="t('docker.title')"
            :title="activeContainer ? t('docker.browsing', { name: activeContainer.name }) : t('docker.title')"
            :disabled="!canNavigate"
          />
          <template #content>
            <ContainerPicker
              :session="activeSession"
              :active-container="activeContainer"
              @enter="enterContainer"
              @exit="exitContainer"
            />
          </template>
        </UPopover>
      </div>

      <!-- 平时是可点的面包屑；点空白处 / Ctrl+L 进入输入模式，回车跳转，Esc 或失焦退出 -->
      <form
        v-liquid-glass="{ scale: 12 }"
        class="path-bar liquid-glass"
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
            <!-- 根目录「/」自己就是分隔符；容器名后面要补一个 -->
            <span v-if="index > 1 || (index === 1 && activeContainer)" class="path-separator" aria-hidden="true">/</span>
            <button
              type="button"
              class="path-crumb"
              :class="{ current: crumb.path === currentPath, ellipsis: crumb.ellipsis, container: crumb.icon }"
              :disabled="!canNavigate || crumb.ellipsis"
              :title="crumb.ellipsis ? editablePath : crumb.path"
              @click.stop="navigateToPath(crumb.path)"
            >
              <UIcon v-if="crumb.icon" :name="crumb.icon" />
              {{ crumb.label }}
            </button>
          </template>
        </nav>
        <UBadge
          v-if="activeContainer && !isEditingPath"
          class="path-readonly"
          color="neutral"
          variant="soft"
          size="sm"
          icon="i-lucide-lock"
          :label="t('docker.readOnly')"
          :title="t('docker.readOnlyHint')"
        />
      </form>

      <UInput
        ref="searchInputControl"
        v-model="query"
        v-liquid-glass="{ scale: 12 }"
        class="liquid-glass"
        color="neutral"
        variant="none"
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
        :disabled="!canModify"
        @click="chooseUploadFiles"
      />

      <UDropdownMenu :items="createMenuItems" size="sm" :content="{ align: 'end' }">
        <UButton
          v-liquid-glass="{ scale: 12 }"
          class="file-create liquid-glass"
          color="neutral"
          variant="ghost"
          size="sm"
          icon="i-lucide-plus"
          trailing-icon="i-lucide-chevron-down"
          :label="t('files.create')"
          :disabled="!canModify"
        />
      </UDropdownMenu>

      <UButton
        v-liquid-glass="{ scale: 12 }"
        class="file-analyze liquid-glass"
        :class="{ active: diskUsageEnabled }"
        color="neutral"
        variant="ghost"
        size="sm"
        square
        icon="i-lucide-chart-pie"
        :aria-pressed="diskUsageEnabled"
        :aria-label="t('diskUsage.toggle')"
        :title="t(diskUsageEnabled ? 'diskUsage.toggleOn' : 'diskUsage.toggle')"
        :disabled="!canTransfer"
        @click="toggleDiskUsage"
      />

      <UDropdownMenu :items="moreMenuItems" size="sm" :content="{ align: 'end' }">
        <UButton
          v-liquid-glass="{ scale: 12 }"
          class="file-more liquid-glass"
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
        <UContextMenu :items="treeContextMenuItems" size="xs" :ui="compactMenuUi">
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
        <UContextMenu :items="contextMenuItems" size="xs" :ui="compactMenuUi">
          <div
            class="file-workbench"
            :class="{ 'drag-over': isDragOver, analyzing: diskUsage }"
            tabindex="-1"
            @contextmenu="prepareContextMenu"
            @wheel.passive="scrollTableFromChrome"
          >
        <DiskUsageSummary
          v-if="diskUsage"
          :analysis="diskUsage"
          @stop="diskUsageStore.cancel(activeSession.id, currentPath)"
          @rerun="rerunDiskUsage"
          @close="toggleDiskUsage"
        />

        <div ref="fileTableHead" class="file-table-head">
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
            <!-- 分析模式下「大小 / 类型」两列换成「占用 / 占比」，列宽不变 -->
            <template v-if="diskUsage">
              <button
                type="button"
                class="file-sort-button"
                :title="t('diskUsage.usageHint')"
                :aria-label="sortAriaLabel('usage')"
                :aria-sort="sortAriaSort('usage')"
                @click="toggleSort('usage')"
              >
                <span>{{ t("diskUsage.usage") }}</span>
                <UIcon :name="sortIcon('usage')" />
              </button>
              <span>{{ t("diskUsage.share") }}</span>
            </template>
            <template v-else>
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
            </template>
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
        </div>

        <section ref="fileTableWrap" class="file-table-wrap" @scroll="syncTableHeadScroll">
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
              @pointerdown="fileDrag.start(fileEntry, $event)"
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
              <template v-if="diskUsage">
                <span class="usage-size">{{ formatUsage(fileEntry) }}</span>
                <span class="usage-share">
                  <template v-if="usagePercent(fileEntry) != null">
                    <UProgress :model-value="usagePercent(fileEntry)" size="xs" class="usage-share-bar" />
                    <span class="usage-share-label">{{ formatShare(usagePercent(fileEntry)) }}</span>
                  </template>
                </span>
              </template>
              <template v-else>
                <span>{{ formatSize(fileEntry) }}</span>
                <span>{{ fileTypeLabel(fileEntry) }}</span>
              </template>
              <span>{{ formatModified(fileEntry) }}</span>
              <span class="file-permissions" :title="permissionsDetail(fileEntry)">{{ permissionsSummary(fileEntry) }}</span>
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

    <ClipboardPasteDialog
      v-model:open="pasteDialog.open"
      :session="activeSession"
      :directory="pasteDialog.directory"
      :content="pasteDialog.content"
    />

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
