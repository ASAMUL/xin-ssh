<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useFileExplorerStore } from "../stores/fileExplorerStore";

const props = defineProps({
  activeSession: {
    type: Object,
    default: null,
  },
});

const { t } = useI18n();
const fileExplorerStore = useFileExplorerStore();

const currentPath = computed(() => props.activeSession?.remotePath || fileExplorerStore.currentPath);
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
const hasEntries = computed(() => fileExplorerStore.entries.length > 0);
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
        <button type="button" :aria-label="t('files.history')">{{ t("files.history") }}</button>
        <button type="button" :aria-label="t('files.refresh')">
          <UIcon name="i-lucide-refresh-cw" />
        </button>
        <button type="button" :aria-label="t('files.download')">
          <UIcon name="i-lucide-download" />
        </button>
        <button type="button" :aria-label="t('files.upload')">
          <UIcon name="i-lucide-upload" />
        </button>
        <button type="button" :aria-label="t('files.newFolder')">
          <UIcon name="i-lucide-folder-plus" />
        </button>
      </div>
    </header>

    <div class="file-path-row">
      <div class="navigation-tools">
        <button type="button" :aria-label="t('files.back')">
          <UIcon name="i-lucide-arrow-left" />
        </button>
        <button type="button" :aria-label="t('files.forward')">
          <UIcon name="i-lucide-arrow-right" />
        </button>
        <button type="button" :aria-label="t('files.parent')">
          <UIcon name="i-lucide-arrow-up" />
        </button>
      </div>

      <div class="path-bar" aria-label="Current remote path">
        <template v-for="crumb in pathCrumbs" :key="crumb.id">
          <button type="button">{{ crumb.label }}</button>
          <span v-if="crumb.id !== pathCrumbs.at(-1)?.id" class="path-separator">/</span>
        </template>
      </div>

      <label class="file-search">
        <UIcon name="i-lucide-search" />
        <input type="search" :placeholder="t('files.search')" />
      </label>
    </div>

    <div class="file-workbench">
      <aside class="remote-tree">
        <button type="button" class="remote-tree-root active">
          <UIcon name="i-lucide-folder-open" />
          <span>{{ currentPath }}</span>
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

        <div v-if="!hasEntries" class="file-empty-state">
          <UIcon name="i-lucide-folder-search" />
          <strong>{{ t("files.emptyTitle") }}</strong>
          <span>{{ t("files.emptyDescription") }}</span>
        </div>

        <button
          v-for="fileEntry in fileExplorerStore.entries"
          :key="fileEntry.id"
          type="button"
          class="file-row file-entry"
        >
          <span class="name-column">
            <span class="file-icon">
              <UIcon :name="fileEntry.icon" />
            </span>
            <span class="file-name">{{ fileEntry.name }}</span>
          </span>
          <span>{{ fileEntry.size }}</span>
          <span>{{ fileEntry.type }}</span>
          <span>{{ fileEntry.modified }}</span>
          <span>{{ fileEntry.permissions }}</span>
        </button>
      </section>
    </div>
  </section>
</template>

<style scoped src="../assets/css/file-explorer.css"></style>
