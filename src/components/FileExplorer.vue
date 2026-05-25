<script setup>
defineProps({
  activeSession: {
    type: Object,
    default: null,
  },
  files: {
    type: Array,
    required: true,
  },
  pathCrumbs: {
    type: Array,
    required: true,
  },
  selectedFileId: {
    type: String,
    required: true,
  },
});

const emit = defineEmits(["selectFile"]);

function selectFile(fileId) {
  emit("selectFile", fileId);
}
</script>

<template>
  <section class="file-explorer" aria-label="Remote file explorer">
    <div class="file-toolbar">
      <div class="navigation-tools">
        <button type="button" aria-label="Back">
          <UIcon name="i-lucide-arrow-left" />
        </button>
        <button type="button" aria-label="Forward">
          <UIcon name="i-lucide-arrow-right" />
        </button>
        <button type="button" aria-label="Parent directory">
          <UIcon name="i-lucide-arrow-up" />
        </button>
      </div>

      <div class="path-bar" aria-label="Current remote path">
        <UIcon name="i-lucide-monitor" class="path-root-icon" />
        <template v-for="crumb in pathCrumbs" :key="crumb.id">
          <span class="path-separator">/</span>
          <button type="button">{{ crumb.label }}</button>
        </template>
      </div>

      <label class="file-search">
        <UIcon name="i-lucide-search" />
        <input
          type="search"
          :placeholder="activeSession ? `Search in ${pathCrumbs.at(-1)?.label}` : 'Search'"
        />
      </label>

      <div class="view-tools">
        <button type="button" aria-label="Open in editor">
          <UIcon name="i-lucide-square-arrow-out-up-right" />
        </button>
        <button type="button" class="active" aria-label="List view">
          <UIcon name="i-lucide-list" />
        </button>
        <button type="button" aria-label="Split view">
          <UIcon name="i-lucide-columns-2" />
        </button>
        <button type="button" aria-label="More file actions">
          <UIcon name="i-lucide-more-vertical" />
        </button>
      </div>
    </div>

    <div class="drop-zone">
      <UIcon name="i-lucide-cloud-upload" />
      <span>Drag and drop files here to upload</span>
    </div>

    <div class="file-table">
      <div class="file-row file-heading">
        <span class="name-column">Name <UIcon name="i-lucide-chevron-up" /></span>
        <span>Size</span>
        <span>Type</span>
        <span>Date Modified</span>
        <span></span>
      </div>

      <button
        v-for="fileEntry in files"
        :key="fileEntry.id"
        type="button"
        class="file-row file-entry"
        :class="{ selected: fileEntry.id === selectedFileId }"
        @click="selectFile(fileEntry.id)"
      >
        <span class="name-column">
          <span class="file-icon" :class="fileEntry.tone">
            <UIcon :name="fileEntry.icon" />
          </span>
          <span class="file-name">{{ fileEntry.name }}</span>
        </span>
        <span>{{ fileEntry.size }}</span>
        <span>{{ fileEntry.type }}</span>
        <span>{{ fileEntry.modified }}</span>
        <span class="row-action">
          <UIcon name="i-lucide-more-horizontal" />
        </span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.file-explorer {
  min-height: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  padding: 22px 22px 0;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.88), rgba(248, 250, 252, 0.96)),
    linear-gradient(90deg, rgba(37, 99, 235, 0.05), rgba(20, 184, 166, 0.04));
}

.file-toolbar {
  display: grid;
  grid-template-columns: auto minmax(240px, 1fr) minmax(190px, 270px) auto;
  align-items: center;
  gap: 12px;
}

.navigation-tools,
.view-tools {
  display: flex;
  align-items: center;
  gap: 6px;
}

.navigation-tools button,
.view-tools button {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  color: #475569;
  border: 0;
  border-radius: 8px;
  background: transparent;
}

.navigation-tools button:hover,
.view-tools button:hover,
.view-tools button.active {
  color: #0f172a;
  background: rgba(148, 163, 184, 0.14);
}

.path-bar,
.file-search {
  height: 42px;
  display: flex;
  align-items: center;
  color: #0f172a;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 0 1px 8px rgba(15, 23, 42, 0.04);
}

.path-bar {
  gap: 9px;
  padding: 0 16px;
}

.path-root-icon {
  color: #334155;
}

.path-bar button {
  color: #0757e6;
  font-size: 13px;
  font-weight: 760;
  border: 0;
  background: transparent;
}

.path-separator {
  color: #94a3b8;
}

.file-search {
  gap: 9px;
  padding: 0 13px;
  color: #64748b;
}

.file-search input {
  width: 100%;
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: #0f172a;
  font-size: 13px;
}

.drop-zone {
  height: 62px;
  margin: 22px 0 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 13px;
  color: #0757e6;
  font-size: 14px;
  font-weight: 720;
  border: 1px dashed rgba(37, 99, 235, 0.45);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.34);
}

.drop-zone :deep(svg) {
  font-size: 21px;
}

.file-table {
  min-height: 0;
  overflow: auto;
  border-top: 1px solid rgba(148, 163, 184, 0.2);
}

.file-row {
  min-width: 850px;
  display: grid;
  grid-template-columns: minmax(300px, 1.4fr) minmax(110px, 0.45fr) minmax(180px, 0.72fr) minmax(210px, 0.82fr) 44px;
  align-items: center;
  min-height: 51px;
  padding: 0 18px;
  color: #475569;
  border: 0;
  border-bottom: 1px solid rgba(148, 163, 184, 0.16);
  background: transparent;
}

.file-heading {
  min-height: 47px;
  color: #0f172a;
  font-size: 13px;
  font-weight: 760;
}

.file-entry {
  width: 100%;
  text-align: left;
  transition:
    background 140ms ease,
    box-shadow 140ms ease;
}

.file-entry:hover,
.file-entry.selected {
  background: rgba(37, 99, 235, 0.06);
}

.name-column {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 12px;
  color: #0f172a;
  font-weight: 720;
}

.file-heading .name-column {
  gap: 5px;
}

.file-icon {
  width: 25px;
  height: 25px;
  display: grid;
  place-items: center;
  color: #ffffff;
  font-size: 19px;
  border-radius: 6px;
  background: #2563eb;
}

.file-icon.sky {
  background: #0ea5e9;
}

.file-icon.orange {
  background: #f97316;
}

.file-icon.amber {
  background: #eab308;
}

.file-icon.violet {
  background: #8b5cf6;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-action {
  justify-self: end;
  color: #334155;
}

@media (max-width: 1120px) {
  .file-toolbar {
    grid-template-columns: auto minmax(220px, 1fr) auto;
  }

  .file-search {
    display: none;
  }
}
</style>
