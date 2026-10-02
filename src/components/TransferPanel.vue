<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  canOpenTaskLocally,
  openTaskFile,
  revealTaskInFolder,
  taskLocalFilePath,
} from "../services/localOpener";
import { useTransferStore } from "../stores/transferStore";
import { useUiStore } from "../stores/uiStore";
import { localizeError } from "../utils/appError";

const { t } = useI18n();
const transferStore = useTransferStore();
const uiStore = useUiStore();

const contextTask = ref(null);

const contextMenuUi = {
  content: "min-w-44 rounded-lg border border-default bg-default shadow-xl backdrop-blur-xl",
  viewport: "divide-y divide-default",
  group: "p-1",
  separator: "my-1 bg-border",
  item: "min-h-8 items-center gap-2 rounded-md px-2 py-1.5 text-xs font-semibold leading-none",
  itemLeadingIcon: "size-4 w-4 shrink-0 text-muted",
  itemWrapper: "min-w-0 flex-1 justify-center",
  itemLabel: "truncate leading-4",
};

const FINISHED_STATUSES = ["completed", "failed", "canceled"];

function isFinished(task) {
  return FINISHED_STATUSES.includes(task.status);
}

function isActive(task) {
  return !isFinished(task);
}

const contextMenuItems = computed(() => {
  const task = contextTask.value;
  if (!task) {
    return [
      [
        {
          label: t("transfers.clearFinished"),
          icon: "i-lucide-list-checks",
          disabled: !transferStore.hasFinishedTasks,
          onSelect: () => transferStore.clearFinished(),
        },
      ],
    ];
  }

  const groups = [];
  const openGroup = [];
  const copyGroup = [];
  const manageGroup = [];

  if (canOpenTaskLocally(task)) {
    if (taskLocalFilePath(task) && !task.remoteEntries?.[0]?.isDirectory) {
      openGroup.push({
        label: t("transfers.openFile"),
        icon: "i-lucide-file-symlink",
        onSelect: () => runOpenAction(() => openTaskFile(task)),
      });
    }
    openGroup.push({
      label: t("transfers.revealInFolder"),
      icon: "i-lucide-folder-open",
      onSelect: () => runOpenAction(() => revealTaskInFolder(task)),
    });
  }

  if (task.direction === "download" && task.localDir) {
    copyGroup.push({
      label: t("transfers.copyLocalPath"),
      icon: "i-lucide-copy",
      onSelect: () => copyText(taskLocalFilePath(task) ?? task.localDir),
    });
  }

  const remotePath = remotePathOf(task);
  if (remotePath) {
    copyGroup.push({
      label: t("transfers.copyRemotePath"),
      icon: "i-lucide-copy",
      onSelect: () => copyText(remotePath),
    });
  }

  if (isActive(task)) {
    manageGroup.push({
      label: t("transfers.cancel"),
      icon: "i-lucide-x",
      disabled: task.status === "canceling",
      onSelect: () => transferStore.cancelTask(task.id),
    });
  } else {
    if (transferStore.isResumeCandidate(task)) {
      manageGroup.push({
        label: t("transfers.resume"),
        icon: "i-lucide-play",
        disabled: !transferStore.canResumeTask(task),
        onSelect: () => resumeTask(task),
      });
    }
    manageGroup.push({
      label: t("transfers.removeRecord"),
      icon: "i-lucide-trash-2",
      disabled: Boolean(transferStore.retryingTasks[task.id]),
      onSelect: () => transferStore.removeTask(task.id),
    });
  }

  if (openGroup.length) groups.push(openGroup);
  if (copyGroup.length) groups.push(copyGroup);
  if (manageGroup.length) groups.push(manageGroup);

  return groups;
});

function prepareContextMenu(event) {
  const target = event.target instanceof HTMLElement ? event.target : null;
  const row = target?.closest("[data-task-id]");
  const taskId = row?.dataset.taskId;

  contextTask.value = taskId ? transferStore.findTask(taskId) : null;
}

function remotePathOf(task) {
  if (task.direction === "upload") return task.remoteDir ?? null;

  const paths = (task.remoteEntries || []).map((entry) => entry.path).filter(Boolean);
  return paths.length ? paths.join("\n") : null;
}

async function runOpenAction(action) {
  try {
    await action();
  } catch (error) {
    uiStore.setError(error);
  }
}

async function resumeTask(task) {
  try {
    await transferStore.resumeTask(task.id);
  } catch (error) {
    uiStore.setError(error);
  }
}

function resumeHint(task) {
  return localizeError(transferStore.resumeBlockReason(task)) || t("transfers.resume");
}

async function copyText(text) {
  if (!text) return;

  try {
    await navigator.clipboard?.writeText(text);
  } catch (error) {
    uiStore.setError(error);
  }
}

function taskIcon(task) {
  if (task.status === "planning") return "i-lucide-folder-search";
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
  if (task.status === "failed") return localizeError(task.error) || t("transfers.failed");

  if (isFinished(task)) {
    const statusText =
      task.status === "completed" ? t("transfers.completed") : localizeError(task.error) || t("transfers.canceled");
    const parts = [statusText];
    if (task.bytesTransferred) parts.push(formatBytes(task.bytesTransferred));
    if (task.sessionName) parts.push(task.sessionName);
    return parts.join(" · ");
  }

  if (task.status === "canceling") return t("transfers.canceling");
  if (task.status === "queued") return t("transfers.queued");
  if (task.status === "planning") return t("transfers.planningMeta", { count: task.filesTotal });
  if (task.phase === "verifying") return t("transfers.verifying");

  const speed = task.speedBytesPerSecond ? `${formatBytes(task.speedBytesPerSecond)}/s` : "--";
  return t("transfers.progressMeta", {
    speed,
    done: task.filesDone,
    total: task.filesTotal,
  });
}

function formatTaskTime(timestamp) {
  if (!timestamp) return "";

  const date = new Date(timestamp);
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  const sameYear = date.getFullYear() === now.getFullYear();
  const pad = (value) => String(value).padStart(2, "0");
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}`;

  if (sameDay) return time;
  if (sameYear) return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${time}`;

  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}
</script>

<template>
  <aside class="transfer-panel" :aria-label="t('transfers.title')">
    <header class="transfer-head">
      <div>
        <strong>{{ t("transfers.title") }}</strong>
        <span>{{
          transferStore.summary.activeCount
            ? t("transfers.summaryActive", { count: transferStore.summary.activeCount })
            : t("transfers.summary", { count: transferStore.summary.totalCount })
        }}</span>
      </div>

      <div class="transfer-head-actions">
        <UButton
          color="neutral"
          variant="ghost"
          size="xs"
          icon="i-lucide-list-checks"
          :disabled="!transferStore.hasFinishedTasks"
          :aria-label="t('transfers.clearFinished')"
          :title="t('transfers.clearFinished')"
          @click="transferStore.clearFinished"
        />
        <UButton
          color="neutral"
          variant="ghost"
          size="xs"
          icon="i-lucide-x"
          :aria-label="t('transfers.closePanel')"
          :title="t('transfers.closePanel')"
          @click="transferStore.closePanel"
        />
      </div>
    </header>

    <UProgress v-if="transferStore.historyIsLoading" :model-value="null" size="xs"
      :aria-label="t('transfers.loadingHistory')" />
    <UAlert v-if="transferStore.historyError" color="warning" variant="soft"
      icon="i-lucide-database" :title="localizeError(transferStore.historyError)"
      :actions="[{ label: t('common.retry'), icon: 'i-lucide-refresh-cw', onClick: () => transferStore.retryHistoryStorage() }]" />

    <UContextMenu :items="contextMenuItems" size="xs" :ui="contextMenuUi">
      <div class="transfer-list" @contextmenu="prepareContextMenu">
        <p v-if="!transferStore.hasTasks && !transferStore.historyIsLoading" class="transfer-empty">
          <UIcon name="i-lucide-inbox" class="transfer-empty-icon" />
          {{ t("transfers.empty") }}
        </p>

        <article
          v-for="task in transferStore.tasks"
          :key="task.id"
          class="transfer-task"
          :class="[task.direction, task.status]"
          :data-task-id="task.id"
        >
          <span class="transfer-icon">
            <UIcon :name="taskIcon(task)" :class="{ 'spin-icon': task.status === 'running' }" />
          </span>

          <div class="transfer-copy">
            <strong :title="task.label">{{ task.label }}</strong>
            <span :title="taskMeta(task)">{{ taskMeta(task) }}</span>
          </div>

          <small v-if="isFinished(task)" class="transfer-time">{{
            formatTaskTime(task.updatedAt)
          }}</small>
          <small v-else-if="task.status === 'planning'">{{ t("transfers.planning") }}</small>
          <small v-else>{{ taskProgress(task) }}%</small>

          <UTooltip v-if="transferStore.isResumeCandidate(task)" :text="resumeHint(task)">
            <span class="transfer-task-action" :tabindex="transferStore.canResumeTask(task) ? undefined : 0">
              <UButton color="primary" variant="ghost" size="xs" icon="i-lucide-play"
                :disabled="!transferStore.canResumeTask(task)" :aria-label="resumeHint(task)"
                @click="resumeTask(task)" />
            </span>
          </UTooltip>
          <UButton
            v-else-if="canOpenTaskLocally(task)"
            color="neutral"
            variant="ghost"
            size="xs"
            icon="i-lucide-folder-open"
            class="transfer-task-action"
            :aria-label="t('transfers.revealInFolder')"
            :title="t('transfers.revealInFolder')"
            @click="runOpenAction(() => revealTaskInFolder(task))"
          />
          <UButton
            v-else-if="transferStore.canCancelTask(task)"
            color="neutral"
            variant="ghost"
            size="xs"
            class="transfer-task-action"
            :icon="task.status === 'canceling' ? 'i-lucide-loader-circle' : 'i-lucide-x'"
            :disabled="task.status === 'canceling'"
            :aria-label="t('transfers.cancel')"
            :title="t('transfers.cancel')"
            @click="transferStore.cancelTask(task.id)"
          />
          <span v-else class="transfer-task-action"></span>

          <span v-if="isActive(task)" class="task-progress">
            <span :style="{ width: `${taskProgress(task)}%` }"></span>
          </span>
        </article>
      </div>
    </UContextMenu>

    <div v-if="transferStore.summary.activeCount" class="transfer-progress">
      <span :style="{ width: `${transferStore.summary.progress}%` }"></span>
    </div>
  </aside>
</template>

<style scoped>
.transfer-panel {
  position: absolute;
  right: 22px;
  bottom: 22px;
  z-index: 30;
  width: min(430px, calc(100% - 44px));
  display: grid;
  gap: 10px;
  padding: 14px;
  color: var(--xin-text);
  border: 1px solid var(--xin-glass-border);
  border-radius: var(--xin-radius-lg);
  background: var(--xin-glass-strong);
  box-shadow:
    var(--xin-specular),
    0 24px 60px rgb(var(--xin-shadow-rgb) / 0.22);
  backdrop-filter: var(--xin-glass-blur);
}

.transfer-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.transfer-head div:first-child {
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
  color: var(--xin-text-muted);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transfer-head-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.transfer-list {
  max-height: min(46vh, 420px);
  display: grid;
  gap: 6px;
  align-content: start;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.transfer-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  margin: 0;
  padding: 22px 0 26px;
  color: var(--xin-status-idle);
  font-size: 12px;
  font-weight: 640;
}

.transfer-empty-icon {
  font-size: 16px;
}

.transfer-task {
  min-width: 0;
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr) auto 26px;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border: 1px solid rgb(var(--xin-neutral-rgb) / 0.16);
  border-radius: 11px;
  background: rgb(var(--xin-fill-rgb) / 0.55);
}

.transfer-icon {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  color: var(--xin-accent-deep);
}

.transfer-task.download .transfer-icon {
  color: #16a34a;
}

.transfer-task.failed .transfer-icon {
  color: var(--xin-danger-strong);
}

.transfer-task.completed .transfer-icon {
  color: #059669;
}

.transfer-task.canceled .transfer-icon {
  color: var(--xin-text-muted);
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
  color: var(--xin-text-muted);
  font-size: 11px;
  font-weight: 720;
}

.transfer-time {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.transfer-task-action {
  justify-self: center;
}

.task-progress {
  grid-column: 1 / -1;
  height: 4px;
  overflow: hidden;
  border-radius: 999px;
  background: rgb(var(--xin-ink-rgb) / 0.09);
}

.task-progress span,
.transfer-progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--xin-accent-deep), #2f8cff);
}

.transfer-task.download .task-progress span {
  background: linear-gradient(90deg, #16a34a, #22c55e);
}

.transfer-progress {
  height: 5px;
  overflow: hidden;
  border-radius: 999px;
  background: rgb(var(--xin-ink-rgb) / 0.09);
}

@media (max-width: 980px) {
  .transfer-panel {
    right: 14px;
    bottom: 14px;
    width: calc(100% - 28px);
  }
}
</style>
