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
import { compactMenuUi } from "../config/menuUi";

const { t } = useI18n();
const transferStore = useTransferStore();
const uiStore = useUiStore();

const contextTask = ref(null);

const FINISHED_STATUSES = ["completed", "failed", "canceled", "paused"];

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

  if (transferStore.canPauseTask(task)) {
    manageGroup.push({
      label: t("transfers.pause"),
      icon: "i-lucide-pause",
      onSelect: () => transferStore.pauseTask(task.id),
    });
  }
  if (transferStore.isResumeCandidate(task)) {
    manageGroup.push({
      label: t("transfers.resume"),
      icon: "i-lucide-play",
      disabled: !transferStore.canResumeTask(task),
      onSelect: () => resumeTask(task),
    });
  }
  if (transferStore.canCancelTask(task)) {
    manageGroup.push({
      label: t("transfers.cancel"),
      icon: "i-lucide-x",
      disabled: task.status === "canceling",
      onSelect: () => transferStore.cancelTask(task.id),
    });
  }
  if (!isActive(task)) {
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
  if (task.status === "paused") return "i-lucide-circle-pause";

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
  if (task.status === "paused") {
    const parts = [t("transfers.paused")];
    if (task.bytesTotal) parts.push(`${formatBytes(task.bytesTransferred)} / ${formatBytes(task.bytesTotal)}`);
    if (task.sessionName) parts.push(task.sessionName);
    return parts.join(" · ");
  }

  if (isFinished(task)) {
    const statusText =
      task.status === "completed" ? t("transfers.completed") : localizeError(task.error) || t("transfers.canceled");
    const parts = [statusText];
    if (task.bytesTransferred) parts.push(formatBytes(task.bytesTransferred));
    if (task.sessionName) parts.push(task.sessionName);
    return parts.join(" · ");
  }

  if (task.status === "canceling") return t("transfers.canceling");
  if (task.status === "pausing") return t("transfers.pausing");
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

    <UContextMenu :items="contextMenuItems" size="xs" :ui="compactMenuUi">
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

          <small v-if="task.status === 'paused'">{{ taskProgress(task) }}%</small>
          <small v-else-if="isFinished(task)" class="transfer-time">{{
            formatTaskTime(task.updatedAt)
          }}</small>
          <small v-else-if="task.status === 'planning'">{{ t("transfers.planning") }}</small>
          <small v-else>{{ taskProgress(task) }}%</small>

          <div class="transfer-task-actions">
            <UTooltip v-if="transferStore.isResumeCandidate(task)" :text="resumeHint(task)">
              <span :tabindex="transferStore.canResumeTask(task) ? undefined : 0">
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
              :aria-label="t('transfers.revealInFolder')"
              :title="t('transfers.revealInFolder')"
              @click="runOpenAction(() => revealTaskInFolder(task))"
            />
            <UButton
              v-if="transferStore.canPauseTask(task) || task.status === 'pausing'"
              color="neutral"
              variant="ghost"
              size="xs"
              :icon="task.status === 'pausing' ? 'i-lucide-loader-circle' : 'i-lucide-pause'"
              :disabled="task.status === 'pausing'"
              :aria-label="t('transfers.pause')"
              :title="t('transfers.pause')"
              @click="transferStore.pauseTask(task.id)"
            />
            <UButton
              v-if="transferStore.canCancelTask(task)"
              color="neutral"
              variant="ghost"
              size="xs"
              :icon="task.status === 'canceling' ? 'i-lucide-loader-circle' : 'i-lucide-x'"
              :disabled="task.status === 'canceling'"
              :aria-label="t('transfers.cancelHint')"
              :title="t('transfers.cancelHint')"
              @click="transferStore.cancelTask(task.id)"
            />
          </div>

          <span v-if="isActive(task) || task.status === 'paused'" class="task-progress">
            <span :style="{ width: `${taskProgress(task)}%` }"></span>
          </span>
        </article>
      </div>
    </UContextMenu>
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
  grid-template-columns: 24px minmax(0, 1fr) auto auto;
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

/* 进行中有暂停、取消两个按钮，其余状态一个或没有；留出最小宽度让右侧百分比和时间对齐 */
.transfer-task-actions {
  min-width: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
}

.task-progress {
  grid-column: 1 / -1;
  height: 4px;
  overflow: hidden;
  border-radius: 999px;
  background: rgb(var(--xin-ink-rgb) / 0.09);
}

.task-progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--xin-accent-deep), #2f8cff);
}

.transfer-task.download .task-progress span {
  background: linear-gradient(90deg, #16a34a, #22c55e);
}

/* 暂停时进度条停在原处并褪成灰色，一眼和正在传的区分开 */
.transfer-task.paused .task-progress span {
  background: rgb(var(--xin-ink-rgb) / 0.28);
}

.transfer-task.paused .transfer-icon {
  color: var(--xin-text-muted);
}

@media (max-width: 980px) {
  .transfer-panel {
    right: 14px;
    bottom: 14px;
    width: calc(100% - 28px);
  }
}
</style>
