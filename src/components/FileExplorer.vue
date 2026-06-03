<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useFileExplorerStore } from "../stores/fileExplorerStore";

const props = defineProps({
  activeSession: {
    type: Object,
    default: null,
  },
});

const { t, locale } = useI18n();
const fileExplorerStore = useFileExplorerStore();
const pathInput = ref("/");

const fileState = computed(() => fileExplorerStore.getSessionState(props.activeSession?.id));
const currentPath = computed(() => fileState.value.currentPath || props.activeSession?.remotePath || "/");
const isConnected = computed(() => props.activeSession?.status === "connected");
const canNavigate = computed(() => isConnected.value && !fileState.value.isLoading);
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

function submitPath() {
  if (!props.activeSession || !canNavigate.value) return;

  fileExplorerStore.loadDirectory(props.activeSession, pathInput.value);
}

function refreshDirectory() {
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

function selectEntry(entry) {
  fileExplorerStore.selectEntry(props.activeSession?.id, entry.path);
}

function openEntry(entry) {
  selectEntry(entry);
  fileExplorerStore.openEntry(props.activeSession, entry);
}

function isSelected(entry) {
  return fileState.value.selectedPaths.includes(entry.path);
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
        <button type="button" :aria-label="t('files.history')" :disabled="!hasEntries">
          <UIcon name="i-lucide-history" />
        </button>
        <button type="button" :aria-label="t('files.refresh')" :disabled="!isConnected" @click="refreshDirectory">
          <UIcon name="i-lucide-refresh-cw" :class="{ 'spin-icon': fileState.isLoading }" />
        </button>
        <button type="button" :aria-label="t('files.download')" disabled>
          <UIcon name="i-lucide-download" />
        </button>
        <button type="button" :aria-label="t('files.upload')" disabled>
          <UIcon name="i-lucide-upload" />
        </button>
        <button type="button" :aria-label="t('files.newFolder')" disabled>
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

    <div class="file-workbench">
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
          @click="selectEntry(fileEntry)"
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
      </section>
    </div>
  </section>
</template>

<style scoped src="../assets/css/file-explorer.css"></style>
