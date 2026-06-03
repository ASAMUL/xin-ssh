<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";
import { useFileExplorerStore } from "../stores/fileExplorerStore";
import { useTransferStore } from "../stores/transferStore";
import { useUiStore } from "../stores/uiStore";

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
const pathInput = ref("/");
const isDragOver = ref(false);
const lastSelectedPath = ref("");
const contextMenu = ref({
  visible: false,
  x: 0,
  y: 0,
  entry: null,
});

let unlistenDragDrop = null;

const fileState = computed(() => fileExplorerStore.getSessionState(props.activeSession?.id));
const currentPath = computed(() => fileState.value.currentPath || props.activeSession?.remotePath || "/");
const isConnected = computed(() => props.activeSession?.status === "connected");
const canNavigate = computed(() => isConnected.value && !fileState.value.isLoading);
const canTransfer = computed(() => Boolean(props.activeSession) && isConnected.value);
const canGoBack = computed(() => fileState.value.historyIndex > 0);
const canGoForward = computed(() => fileState.value.historyIndex < fileState.value.history.length - 1);
const hasEntries = computed(() => fileState.value.entries.length > 0);
const query = computed({
  get: () => fileState.value.query,
  set: (value) => fileExplorerStore.setQuery(props.activeSession?.id, value),
});
const pathCrumbs = computed(() => {
  const parts = currentPath.value.split("/").filter(Boolean);
  const crumbs = [{ id: "root", label: "/", path: "/" }];

  parts.reduce((path, part) => {
    const nextPath = `${path === "/" ? "" : path}/${part}`;
    crumbs.push({
      id: nextPath,
      label: part,
      path: nextPath,
    });
    return nextPath;
  }, "/");

  return crumbs;
});
const visibleEntries = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return fileState.value.entries;

  return fileState.value.entries.filter((entry) => entry.name.toLowerCase().includes(keyword));
});
const selectedEntries = computed(() => {
  const selectedPaths = new Set(fileState.value.selectedPaths);
  return fileState.value.entries.filter((entry) => selectedPaths.has(entry.path));
});
const canDownload = computed(() => canTransfer.value && selectedEntries.value.length > 0);
const contextEntries = computed(() => {
  const entry = contextMenu.value.entry;
  if (!entry) return selectedEntries.value;

  return isSelected(entry) && selectedEntries.value.length ? selectedEntries.value : [entry];
});
const emptyState = computed(() => {
  if (!isConnected.value) {
    return {
      icon: "i-lucide-unplug",
      title: t("files.waitingTitle"),
      description: t("files.waitingDescription"),
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

onMounted(() => {
  window.addEventListener("click", closeContextMenu);
  window.addEventListener("keydown", handleWindowKeydown);
  registerDragDrop();
});

onBeforeUnmount(() => {
  window.removeEventListener("click", closeContextMenu);
  window.removeEventListener("keydown", handleWindowKeydown);
  unlistenDragDrop?.();
});

function submitPath() {
  if (!props.activeSession || !canNavigate.value) return;

  fileExplorerStore.loadDirectory(props.activeSession, pathInput.value);
}

function refreshDirectory() {
  closeContextMenu();
  if (!props.activeSession || !isConnected.value) return;

  fileExplorerStore.refreshDirectory(props.activeSession);
}

function openPath(path) {
  if (!props.activeSession || !canNavigate.value) return;

  fileExplorerStore.loadDirectory(props.activeSession, path);
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
  return fileState.value.selectedPaths.includes(entry.path);
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

function openContextMenu(event, entry = null) {
  if (!isConnected.value) return;

  if (entry && !isSelected(entry)) {
    selectEntry(entry);
  }

  const { x, y } = clampMenuPosition(event, entry);
  contextMenu.value = {
    visible: true,
    x,
    y,
    entry,
  };
}

function closeContextMenu() {
  if (!contextMenu.value.visible) return;

  contextMenu.value = {
    visible: false,
    x: 0,
    y: 0,
    entry: null,
  };
}

function openContextEntry() {
  const entry = contextMenu.value.entry;
  if (!entry) return;

  openEntry(entry);
}

async function copyContextPath() {
  const path = contextMenu.value.entry?.path;
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
    closeContextMenu();
    isDragOver.value = false;
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

function clampMenuPosition(event, entry) {
  const menuWidth = 218;
  const menuHeight = entry ? 260 : 188;
  const maxX = Math.max(8, window.innerWidth - menuWidth - 8);
  const maxY = Math.max(8, window.innerHeight - menuHeight - 8);

  return {
    x: Math.min(Math.max(event.clientX, 8), maxX),
    y: Math.min(Math.max(event.clientY, 8), maxY),
  };
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

function formatModified(entry) {
  if (!entry.modifiedAt) return "--";

  return new Date(entry.modifiedAt * 1000).toLocaleString(locale.value, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function fileTypeLabel(entry) {
  return t(`files.kinds.${entry.kind || "other"}`);
}
</script>

<template>
  <section class="file-explorer" aria-label="Remote file explorer">
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
        <span class="file-count">{{ t("files.itemCount", { count: visibleEntries.length }) }}</span>
        <button type="button" :aria-label="t('files.history')" :title="t('files.history')" :disabled="!hasEntries">
          <UIcon name="i-lucide-history" />
        </button>
        <button
          type="button"
          :aria-label="t('files.refresh')"
          :title="t('files.refresh')"
          :disabled="!isConnected"
          @click="refreshDirectory"
        >
          <UIcon name="i-lucide-refresh-cw" :class="{ 'spin-icon': fileState.isLoading }" />
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
        <button type="button" :aria-label="t('files.newFolder')" :title="t('files.newFolder')" disabled>
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
        <input v-model="pathInput" type="text" :disabled="!isConnected" spellcheck="false" />
      </form>

      <label class="file-search">
        <UIcon name="i-lucide-search" />
        <input v-model="query" type="search" :placeholder="t('files.search')" />
      </label>
    </div>

    <div class="file-workbench" :class="{ 'drag-over': isDragOver }" @contextmenu.prevent="openContextMenu($event)">
      <aside class="remote-tree">
        <button
          v-for="crumb in pathCrumbs"
          :key="crumb.id"
          type="button"
          class="remote-tree-root"
          :class="{ active: crumb.path === currentPath }"
          :disabled="!canNavigate"
          @click="openPath(crumb.path)"
        >
          <UIcon :name="crumb.path === currentPath ? 'i-lucide-folder-open' : 'i-lucide-folder'" />
          <span>{{ crumb.label }}</span>
        </button>
      </aside>

      <section class="file-table-wrap">
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

        <button
          v-for="fileEntry in visibleEntries"
          :key="fileEntry.id"
          type="button"
          class="file-row file-entry"
          :class="{ selected: isSelected(fileEntry), directory: fileEntry.isDirectory }"
          @click="selectEntry(fileEntry, $event)"
          @dblclick="openEntry(fileEntry)"
          @contextmenu.stop.prevent="openContextMenu($event, fileEntry)"
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
      </section>

      <div v-if="isDragOver" class="file-drop-overlay">
        <UIcon name="i-lucide-cloud-upload" />
        <strong>{{ t("files.dropUploadTitle") }}</strong>
        <span>{{ t("files.dropUploadDescription", { path: currentPath }) }}</span>
      </div>
    </div>

    <div
      v-if="contextMenu.visible"
      class="file-context-menu"
      :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
      @click.stop
      @contextmenu.prevent
    >
      <button
        v-if="contextMenu.entry?.isDirectory"
        type="button"
        class="context-item"
        @click="openContextEntry"
      >
        <UIcon name="i-lucide-folder-open" />
        <span>{{ t("files.open") }}</span>
      </button>
      <button
        type="button"
        class="context-item"
        :disabled="!contextEntries.length"
        @click="downloadContextEntries"
      >
        <UIcon name="i-lucide-download" />
        <span>{{ t("files.download") }}</span>
      </button>
      <button
        v-if="contextMenu.entry"
        type="button"
        class="context-item"
        @click="copyContextPath"
      >
        <UIcon name="i-lucide-copy" />
        <span>{{ t("files.copyPath") }}</span>
      </button>
      <span class="context-divider"></span>
      <button type="button" class="context-item" :disabled="!canTransfer" @click="chooseUploadFiles">
        <UIcon name="i-lucide-upload" />
        <span>{{ t("files.uploadFiles") }}</span>
      </button>
      <button type="button" class="context-item" :disabled="!canTransfer" @click="chooseUploadFolder">
        <UIcon name="i-lucide-folder-up" />
        <span>{{ t("files.uploadFolder") }}</span>
      </button>
      <button type="button" class="context-item" :disabled="!isConnected" @click="refreshDirectory">
        <UIcon name="i-lucide-refresh-cw" />
        <span>{{ t("files.refresh") }}</span>
      </button>
    </div>
  </section>
</template>

<style scoped src="../assets/css/file-explorer.css"></style>
