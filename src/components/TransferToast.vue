<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps({
  summary: {
    type: Object,
    required: true,
  },
  tasks: {
    type: Array,
    required: true,
  },
});

const emit = defineEmits(["clear-finished", "cancel"]);
const { t } = useI18n();

const hasFinishedTask = computed(() =>
  props.tasks.some((task) => ["completed", "failed", "canceled"].includes(task.status)),
);

function taskIcon(task) {
  if (task.status === "failed") return "i-lucide-circle-alert";
  if (task.status === "completed") return "i-lucide-circle-check";
  if (task.status === "canceled") return "i-lucide-circle-minus";

  return task.direction === "upload" ? "i-lucide-arrow-up" : "i-lucide-arrow-down";
}

function taskProgress(task) {
  if (task.status === "completed") return 100;
  if (!task.bytesTotal) return 0;

  return Math.min(Math.round((task.bytesTransferred / task.bytesTotal) * 100), 100);
}

function formatBytes(value) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = Number(value || 0);
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  const digits = unitIndex === 0 || size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}

function taskMeta(task) {
  if (task.status === "failed") return task.error || t("transfers.failed");
  if (task.status === "completed") return t("transfers.completed");
  if (task.status === "canceled") return t("transfers.canceled");
  if (task.status === "canceling") return t("transfers.canceling");
  if (task.status === "queued") return t("transfers.queued");

  const speed = task.speedBytesPerSecond ? `${formatBytes(task.speedBytesPerSecond)}/s` : "--";
  return t("transfers.progressMeta", {
    speed,
    done: task.filesDone,
    total: task.filesTotal,
  });
}
</script>

<template>
  <aside class="transfer-toast" aria-label="Transfer queue">
    <header class="transfer-head">
      <div>
        <strong>{{ t("transfers.title") }}</strong>
        <span>{{ t("transfers.summary", { count: summary.totalCount }) }}</span>
      </div>

      <button
        type="button"
        :aria-label="t('transfers.clearFinished')"
        :disabled="!hasFinishedTask"
        @click="emit('clear-finished')"
      >
        <UIcon name="i-lucide-list-checks" />
      </button>
    </header>

    <div class="transfer-list">
      <article
        v-for="task in tasks"
        :key="task.id"
        class="transfer-task"
        :class="[task.direction, task.status]"
      >
        <span class="transfer-icon">
          <UIcon :name="taskIcon(task)" :class="{ 'spin-icon': task.status === 'running' }" />
        </span>

        <div class="transfer-copy">
          <strong>{{ task.label }}</strong>
          <span>{{ taskMeta(task) }}</span>
        </div>

        <small>{{ taskProgress(task) }}%</small>
        <button
          v-if="['queued', 'running', 'canceling'].includes(task.status)"
          type="button"
          class="transfer-cancel"
          :disabled="task.status === 'canceling'"
          :aria-label="t('transfers.cancel')"
          :title="t('transfers.cancel')"
          @click="emit('cancel', task.id)"
        >
          <UIcon
            :name="task.status === 'canceling' ? 'i-lucide-loader-circle' : 'i-lucide-x'"
            :class="{ 'spin-icon': task.status === 'canceling' }"
          />
        </button>
        <span class="task-progress"><span :style="{ width: `${taskProgress(task)}%` }"></span></span>
      </article>
    </div>

    <div class="transfer-progress">
      <span :style="{ width: `${summary.progress}%` }"></span>
    </div>
  </aside>
</template>

<style scoped>
.transfer-toast {
  position: absolute;
  right: 22px;
  bottom: 22px;
  z-index: 30;
  width: min(430px, calc(100% - 44px));
  display: grid;
  gap: 10px;
  padding: 14px;
  color: #0f172a;
  border: 1px solid rgba(148, 163, 184, 0.22);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 24px 60px rgba(15, 23, 42, 0.22);
  backdrop-filter: blur(20px);
}

.transfer-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.transfer-head div {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.transfer-head strong {
  font-size: 13px;
  font-weight: 780;
}

.transfer-head span,
.transfer-copy span {
  overflow: hidden;
  color: #64748b;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transfer-head button {
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  color: #475569;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 6px;
  background: #ffffff;
}

.transfer-head button:disabled {
  opacity: 0.42;
}

.transfer-list {
  display: grid;
  gap: 6px;
}

.transfer-task {
  min-width: 0;
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr) auto 26px;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border: 1px solid rgba(148, 163, 184, 0.14);
  border-radius: 7px;
  background: #fbfcfe;
}

.transfer-icon {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  color: #0757e6;
}

.transfer-task.download .transfer-icon {
  color: #16a34a;
}

.transfer-task.failed .transfer-icon {
  color: #dc2626;
}

.transfer-task.completed .transfer-icon {
  color: #059669;
}

.transfer-task.canceled .transfer-icon {
  color: #64748b;
}

.transfer-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.transfer-copy strong {
  overflow: hidden;
  font-size: 12px;
  font-weight: 760;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transfer-task small {
  color: #64748b;
  font-size: 11px;
  font-weight: 720;
}

.task-progress {
  grid-column: 1 / -1;
  height: 4px;
  overflow: hidden;
  border-radius: 999px;
  background: #e2e8f0;
}

.task-progress span,
.transfer-progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #0757e6, #2f8cff);
}

.transfer-task.download .task-progress span {
  background: linear-gradient(90deg, #16a34a, #22c55e);
}

.transfer-task.failed .task-progress span {
  background: #dc2626;
}

.transfer-task.canceled .task-progress span {
  background: #94a3b8;
}

.transfer-cancel {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  color: #64748b;
  border: 0;
  border-radius: 7px;
  background: rgba(148, 163, 184, 0.12);
}

.transfer-cancel:hover:not(:disabled) {
  color: #0f172a;
  background: rgba(148, 163, 184, 0.2);
}

.transfer-cancel:disabled {
  cursor: progress;
  opacity: 0.72;
}

.transfer-progress {
  height: 5px;
  overflow: hidden;
  border-radius: 999px;
  background: #e2e8f0;
}

@media (max-width: 980px) {
  .transfer-toast {
    right: 14px;
    bottom: 14px;
    width: calc(100% - 28px);
  }
}
</style>
