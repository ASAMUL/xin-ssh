<script setup>
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@nuxt/ui/composables";
import { save } from "@tauri-apps/plugin-dialog";
import { RefreshCw } from "@respeak/lucide-motion-vue";
import ContainerLogConsole from "./ContainerLogConsole.vue";
import MotionIcon from "../MotionIcon.vue";
import { getTerminalTheme, resolveTerminalFontStack, resolveThemeVariant } from "../../config/terminalThemes";
import { cancelFileOperation, exportContainerLogs, startContainerLogs } from "../../services/workspaceApi";
import {
  CONTAINER_LOG_SINCE_OPTIONS,
  CONTAINER_LOG_TAIL_OPTIONS,
  useContainerLogStore,
} from "../../stores/containerLogStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { resolvedAppearance } from "../../utils/appearanceMode";
import { localizeError } from "../../utils/appError";

// 外壳和文件编辑器同一种大窗口，日志画布用终端的调色板和字体
const MODAL_UI = {
  content: "solid-sheet container-log-modal max-w-[min(1180px,94vw)] sm:max-w-[min(1180px,94vw)] h-[86vh] flex flex-col",
  body: "relative flex-1 min-h-0 p-0 sm:p-0 overflow-hidden flex flex-col",
  footer: "justify-between gap-3",
};

const { t } = useI18n();
const toast = useToast();
const logStore = useContainerLogStore();
const settingsStore = useSettingsStore();
const consoleRef = ref(null);
// connecting → streaming → ended / canceled / failed
const status = ref("connecting");
const streamError = ref(null);
const isExporting = ref(false);
// 当前这条流：重新加载、改条件、关窗时换掉，旧流迟到的输出认不出自己就丢掉
let stream = null;

const container = computed(() => logStore.container);
// 列表里有根目录的就是在跑的；停掉的容器读完已有日志就结束
const isRunning = computed(() => Boolean(container.value?.root));
const isConnected = computed(() => logStore.session?.status === "connected");
const theme = computed(() => resolveThemeVariant(
  getTerminalTheme(settingsStore.terminalTheme),
  settingsStore.terminalAppearance,
  resolvedAppearance.value,
));
const fontFamily = computed(() => resolveTerminalFontStack(settingsStore.terminalFontFamily));
// xterm 四周的留白也要是画布色，不然露出一圈玻璃
const canvasStyle = computed(() => ({ background: theme.value.xterm.background }));

const tailItems = computed(() => CONTAINER_LOG_TAIL_OPTIONS.map((value) => ({
  value,
  label: t("containerLogs.tailLines", { count: value.toLocaleString() }),
})));
const sinceItems = computed(() => CONTAINER_LOG_SINCE_OPTIONS.map(({ value, labelKey }) => ({
  value,
  label: t(labelKey),
})));
const tail = computed({
  get: () => logStore.tail,
  set: (value) => { logStore.tail = value; },
});
const sinceSeconds = computed({
  get: () => logStore.sinceSeconds,
  set: (value) => { logStore.sinceSeconds = value; },
});

const subtitle = computed(() => {
  const target = container.value;
  if (!target) return "";
  // 退出码只对真正退出过的容器有意义：created 的从没跑过，码也是 0
  const state = target.status === "exited"
    ? t("containerLogs.exited", { code: target.exitCode })
    : t(`containerLogs.state.${target.status}`, target.status);
  return [target.image, target.id, state].filter(Boolean).join(" · ");
});

const statusLabel = computed(() => {
  switch (status.value) {
    case "streaming":
      return isRunning.value ? t("containerLogs.following") : t("containerLogs.loading");
    case "ended":
      return isRunning.value ? t("containerLogs.endedStopped") : t("containerLogs.endedHistory");
    case "canceled":
      return isConnected.value ? t("containerLogs.stopped") : t("containerLogs.disconnected");
    case "failed":
      return t("containerLogs.failed");
    default:
      return t("containerLogs.loading");
  }
});
const errorText = computed(() => localizeError(streamError.value));

// 条件一变就按新条件从头读；关着窗口时只记下来，下次打开用
watch(() => [logStore.tail, logStore.sinceSeconds, logStore.timestamps], () => {
  if (logStore.isOpen) start();
});
watch(() => logStore.isOpen, (open) => {
  if (!open) stop();
});
// 断线时服务器上的 docker logs 跟着连接一起没了；重新连上就接着看，不用用户再点一次
watch(isConnected, (connected) => {
  if (connected && logStore.isOpen && status.value === "canceled") start();
});
onBeforeUnmount(stop);

async function start() {
  stop();
  const session = logStore.session;
  const target = container.value;
  if (!session || !target || !consoleRef.value) return;

  consoleRef.value.clear();
  const current = { taskId: crypto.randomUUID() };
  stream = current;
  status.value = "connecting";
  streamError.value = null;
  try {
    await startContainerLogs({
      taskId: current.taskId,
      sessionId: session.id,
      containerId: target.id,
      tail: logStore.tail,
      sinceSeconds: logStore.sinceSeconds || null,
      timestamps: logStore.timestamps,
    }, (event) => handleEvent(current, event));
    // 回执到了就是开始读了；没有新日志的容器可能一直没有输出，不能一直显示「正在读取」
    if (stream === current && status.value === "connecting") status.value = "streaming";
  } catch (error) {
    if (stream === current) finish("failed", error);
  }
}

function handleEvent(current, event) {
  if (stream !== current) return;
  if (event.kind === "output") {
    status.value = "streaming";
    consoleRef.value?.write(event.data);
    return;
  }
  finish(event.kind, event.error ?? null);
}

function finish(kind, error = null) {
  stream = null;
  status.value = kind;
  streamError.value = error;
}

function stop() {
  if (!stream) return;
  const { taskId } = stream;
  stream = null;
  // 会话已经断了的话任务早就随连接结束，取消失败无所谓
  cancelFileOperation(taskId).catch(() => {});
}

function toggleTimestamps() {
  logStore.timestamps = !logStore.timestamps;
}

async function exportLogs() {
  const content = consoleRef.value?.readText() ?? "";
  if (!content) {
    toast.add({ title: t("containerLogs.exportEmpty"), color: "warning" });
    return;
  }
  const selected = await save({
    defaultPath: `${container.value.name}-${fileStamp(new Date())}.log`,
    title: t("containerLogs.exportTitle"),
    filters: [{ name: "Log", extensions: ["log", "txt"] }],
  });
  if (typeof selected !== "string") return;

  isExporting.value = true;
  try {
    await exportContainerLogs(selected, content);
    toast.add({ title: t("containerLogs.exported"), icon: "i-lucide-check", color: "success" });
  } catch (error) {
    toast.add({ title: t("containerLogs.exportFailed"), description: localizeError(error), color: "error" });
  } finally {
    isExporting.value = false;
  }
}

// 文件名里用本地时间，看名字就知道是哪次导出的
function fileStamp(date) {
  const pad = (value) => String(value).padStart(2, "0");
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-`
    + `${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

function handleOpenChange(open) {
  if (!open) logStore.close();
}
</script>

<template>
  <UModal :open="logStore.isOpen" :ui="MODAL_UI" :close="false" @update:open="handleOpenChange">
    <template #header>
      <div class="container-log-header">
        <span class="container-log-icon" :class="{ stopped: !isRunning }">
          <UIcon name="i-lucide-scroll-text" />
        </span>
        <div class="container-log-heading">
          <strong class="container-log-name">{{ t("containerLogs.title", { name: container?.name }) }}</strong>
          <span class="container-log-subtitle" :title="subtitle">{{ subtitle }}</span>
        </div>

        <div class="container-log-tools">
          <USelect
            v-model="tail"
            class="container-log-select"
            color="neutral"
            variant="soft"
            size="sm"
            icon="i-lucide-rows-3"
            :items="tailItems"
            :aria-label="t('containerLogs.tail')"
          />
          <USelect
            v-model="sinceSeconds"
            class="container-log-select"
            color="neutral"
            variant="soft"
            size="sm"
            icon="i-lucide-history"
            :items="sinceItems"
            :aria-label="t('containerLogs.since')"
          />
          <UTooltip :text="t('containerLogs.timestamps')">
            <UButton
              :color="logStore.timestamps ? 'primary' : 'neutral'"
              :variant="logStore.timestamps ? 'soft' : 'ghost'"
              size="sm"
              square
              icon="i-lucide-clock"
              :aria-pressed="logStore.timestamps"
              :aria-label="t('containerLogs.timestamps')"
              @click="toggleTimestamps"
            />
          </UTooltip>
          <span class="container-log-divider" aria-hidden="true"></span>
          <UTooltip :text="t('containerLogs.search')" :kbds="['ctrl', 'F']">
            <UButton
              color="neutral"
              variant="ghost"
              size="sm"
              square
              icon="i-lucide-search"
              :aria-label="t('containerLogs.search')"
              @click="consoleRef?.toggleSearch()"
            />
          </UTooltip>
          <UTooltip :text="t('containerLogs.clear')">
            <UButton
              color="neutral"
              variant="ghost"
              size="sm"
              square
              icon="i-lucide-eraser"
              :aria-label="t('containerLogs.clear')"
              @click="consoleRef?.clear()"
            />
          </UTooltip>
          <UTooltip :text="t('containerLogs.export')">
            <UButton
              color="neutral"
              variant="ghost"
              size="sm"
              square
              icon="i-lucide-download"
              :loading="isExporting"
              :aria-label="t('containerLogs.export')"
              @click="exportLogs"
            />
          </UTooltip>
          <UTooltip :text="t('containerLogs.reload')">
            <UButton
              color="neutral"
              variant="ghost"
              size="sm"
              square
              icon="i-lucide-refresh-cw"
              :disabled="!isConnected"
              :aria-label="t('containerLogs.reload')"
              @click="start"
            >
              <template #leading="{ ui }">
                <MotionIcon :icon="RefreshCw" animation="lucide-animated" hover :class="ui.leadingIcon()" />
              </template>
            </UButton>
          </UTooltip>
        </div>

        <UButton
          color="neutral"
          variant="ghost"
          square
          icon="i-lucide-x"
          :aria-label="t('containerLogs.close')"
          @click="logStore.close()"
        />
      </div>
    </template>

    <template #body>
      <ContainerLogConsole
        ref="consoleRef"
        :style="canvasStyle"
        :theme="theme"
        :font-family="fontFamily"
        :font-size="settingsStore.terminalFontSize"
        @ready="start"
      />
      <UAlert
        v-if="status === 'failed'"
        class="container-log-alert"
        color="error"
        variant="soft"
        icon="i-lucide-circle-alert"
        :title="t('containerLogs.failed')"
        :description="errorText"
        :actions="isConnected ? [{ label: t('containerLogs.retry'), icon: 'i-lucide-refresh-cw', color: 'neutral', variant: 'outline', onClick: start }] : []"
      />
    </template>

    <template #footer>
      <div class="container-log-status" :data-status="status" :class="{ live: status === 'streaming' && isRunning }">
        <UIcon v-if="status === 'connecting'" name="i-lucide-loader-circle" class="spin-icon" />
        <span v-else class="container-log-status-dot" aria-hidden="true"></span>
        <span>{{ statusLabel }}</span>
      </div>
      <UButton color="neutral" variant="outline" :label="t('containerLogs.close')" @click="logStore.close()" />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/container-log-window.css"></style>
