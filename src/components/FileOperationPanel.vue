<script setup>
import { useI18n } from "vue-i18n";
import { useFileOperationStore } from "../stores/fileOperationStore";
import { localizeError } from "../utils/appError";

const { t, n } = useI18n();
const operations = useFileOperationStore();
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
              <UIcon name="i-lucide-trash-2" />
              <strong>{{ task.entryCount === 1 ? task.label : t('fileOperations.items', { count: task.entryCount }) }}</strong>
              <UTooltip :text="t(operations.isActive(task) ? 'common.cancel' : 'fileOperations.remove')">
                <UButton color="neutral" variant="ghost" square size="xs"
                  :icon="operations.isActive(task) ? 'i-lucide-square' : 'i-lucide-x'"
                  :disabled="task.status === 'canceling'"
                  :aria-label="t(operations.isActive(task) ? 'common.cancel' : 'fileOperations.remove')"
                  @click="operations.isActive(task) ? operations.cancelTask(task.id) : operations.removeTask(task.id)" />
              </UTooltip>
            </div>
            <div class="file-operation-meta"><span>{{ task.sessionName }}</span><span>{{ t(`fileOperations.status.${task.status}`) }}</span></div>
            <UProgress v-if="operations.isActive(task)" :model-value="null" size="xs" color="warning"
              :aria-label="t(`fileOperations.status.${task.status}`)" />
            <p class="file-operation-path" :title="task.currentPath">{{ task.currentPath || task.paths[0] }}</p>
            <p class="file-operation-count">{{ t('fileOperations.deletedCount', { count: n(task.completedEntries) }) }}</p>
            <UAlert v-if="task.error" color="error" variant="soft" :title="localizeError(task.error)" />
          </article>
        </div>
        <p class="file-operation-note">{{ t('fileOperations.cancelHint') }}</p>
      </section>
    </template>
  </UPopover>
</template>

<style src="../assets/css/file-operation-panel.css"></style>
