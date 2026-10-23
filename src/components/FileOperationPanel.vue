<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useFileOperationStore } from "../stores/fileOperationStore";
import { localizeError } from "../utils/appError";

const { t, n } = useI18n();
const operations = useFileOperationStore();

const KIND_ICONS = {
  delete: "i-lucide-trash-2",
  compress: "i-lucide-file-archive",
  extract: "i-lucide-package-open",
};
// 删除取消不可恢复，压缩解压取消会清掉半成品：两种说明只在列表里有对应任务时才显示
const notes = computed(() => {
  const kinds = new Set(operations.tasks.map((task) => task.kind));
  return [
    kinds.has("delete") && t("fileOperations.cancelHint"),
    (kinds.has("compress") || kinds.has("extract")) && t("fileOperations.archiveCancelHint"),
  ].filter(Boolean);
});

function taskTitle(task) {
  if (task.kind === "delete") {
    return task.entryCount === 1 ? task.label : t("fileOperations.items", { count: task.entryCount });
  }
  if (task.kind === "compress" && task.entryCount > 1) {
    return t("fileOperations.compressItems", { count: task.entryCount });
  }
  return t(`fileOperations.titles.${task.kind}`, { name: task.label });
}

// 进行中和失败的说法跟着任务类型走（正在删除 / 正在压缩），其余状态通用
function statusLabel(task) {
  if (task.status === "running" || task.status === "failed") {
    return t(`fileOperations.status.${task.status}.${task.kind}`);
  }
  return t(`fileOperations.status.${task.status}`);
}

function countLabel(task) {
  return t(`fileOperations.counts.${task.kind}`, { count: n(task.completedEntries) });
}
</script>

<template>
  <UPopover v-if="operations.tasks.length" v-model:open="operations.panelOpen" :content="{ align: 'end' }">
    <UChip :show="operations.activeCount > 0" :text="operations.activeCount" color="warning" size="3xs" inset>
      <UButton color="neutral" variant="ghost" size="sm" icon="i-lucide-list-checks"
        :aria-label="t('fileOperations.title')" :title="t('fileOperations.title')" />
    </UChip>
    <template #content>
      <section class="file-operation-panel" :aria-label="t('fileOperations.title')">
        <header class="file-operation-header">
          <strong>{{ t('fileOperations.title') }}</strong>
          <UTooltip :text="t('fileOperations.clearFinished')">
            <UButton color="neutral" variant="ghost" square icon="i-lucide-list-x"
              :disabled="!operations.hasFinished" :aria-label="t('fileOperations.clearFinished')"
              @click="operations.clearFinished()" />
          </UTooltip>
        </header>
        <div class="file-operation-list">
          <article v-for="task in operations.tasks" :key="task.id" class="file-operation-item">
            <div class="file-operation-heading">
              <UIcon :name="KIND_ICONS[task.kind]" />
              <strong>{{ taskTitle(task) }}</strong>
              <UTooltip :text="t(operations.isActive(task) ? 'common.cancel' : 'fileOperations.remove')">
                <UButton color="neutral" variant="ghost" square size="xs"
                  :icon="operations.isActive(task) ? 'i-lucide-square' : 'i-lucide-x'"
                  :disabled="task.status === 'canceling'"
                  :aria-label="t(operations.isActive(task) ? 'common.cancel' : 'fileOperations.remove')"
                  @click="operations.isActive(task) ? operations.cancelTask(task.id) : operations.removeTask(task.id)" />
              </UTooltip>
            </div>
            <div class="file-operation-meta"><span>{{ task.sessionName }}</span><span>{{ statusLabel(task) }}</span></div>
            <UProgress v-if="operations.isActive(task)" :model-value="null" size="xs" color="warning"
              :aria-label="statusLabel(task)" />
            <p class="file-operation-path" :title="task.currentPath">{{ task.currentPath || task.paths[0] }}</p>
            <!-- python3 兜底压缩解压时不报进度，数到 0 的计数没有意义 -->
            <p v-if="task.kind === 'delete' || task.completedEntries > 0" class="file-operation-count">
              {{ countLabel(task) }}
            </p>
            <p v-if="task.resultPath && task.status === 'completed'" class="file-operation-result" :title="task.resultPath">
              {{ t('fileOperations.result', { path: task.resultPath }) }}
            </p>
            <UAlert v-if="task.error" color="error" variant="soft" :title="localizeError(task.error)" />
          </article>
        </div>
        <p v-for="note in notes" :key="note" class="file-operation-note">{{ note }}</p>
      </section>
    </template>
  </UPopover>
</template>

<style src="../assets/css/file-operation-panel.css"></style>
