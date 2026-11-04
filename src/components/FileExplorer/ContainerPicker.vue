<script setup>
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { RefreshCw } from "@respeak/lucide-motion-vue";
import MotionIcon from "../MotionIcon.vue";
import { listDockerContainers } from "../../services/workspaceApi";
import { localizeError } from "../../utils/appError";

// 只是弹层里的内容；触发按钮留在文件工具栏的胶囊里，和其它工具按钮共用样式
const props = defineProps({
  session: {
    type: Object,
    default: null,
  },
  // 路径在 /proc/<PID>/root 下时是当前所在的容器，否则为 null
  activeContainer: {
    type: Object,
    default: null,
  },
});
const emit = defineEmits(["enter", "exit", "logs"]);
const { t } = useI18n();
const containers = ref([]);
const isRoot = ref(true);
const isLoading = ref(false);
const error = ref("");
let requestToken = 0;

// 后端已按「在跑的在前」排好；停掉的单独成组，没有根目录可进，点它就是看日志
const groups = computed(() => {
  const running = containers.value.filter((container) => container.root);
  const stopped = containers.value.filter((container) => !container.root);
  if (!stopped.length) return [{ key: "running", items: running }];
  return [
    { key: "running", label: t("docker.running", { count: running.length }), items: running },
    { key: "stopped", label: t("docker.stopped", { count: stopped.length }), items: stopped },
  ].filter((group) => group.items.length);
});

// 弹层每次打开都重新挂载、重新列：容器起停很频繁，重启后 PID 也会变
onMounted(refresh);

async function refresh() {
  if (!props.session) return;
  const token = ++requestToken;
  isLoading.value = true;
  error.value = "";
  try {
    const result = await listDockerContainers(props.session.id);
    if (token !== requestToken) return;
    containers.value = result.containers;
    isRoot.value = result.isRoot;
  } catch (listError) {
    if (token !== requestToken) return;
    containers.value = [];
    error.value = localizeError(listError);
  } finally {
    if (token === requestToken) isLoading.value = false;
  }
}

function pick(container) {
  emit(container.root ? "enter" : "logs", container);
}

function stateLabel(container) {
  if (container.root) return `PID ${container.pid}`;
  return container.status === "exited"
    ? t("docker.exitCode", { code: container.exitCode })
    : t(`containerLogs.state.${container.status}`, container.status);
}
</script>

<template>
  <section class="container-picker" :aria-label="t('docker.title')">
    <header class="container-picker-header">
      <strong>{{ t("docker.title") }}</strong>
      <UTooltip :text="t('docker.refresh')">
        <UButton
          color="neutral"
          variant="ghost"
          size="xs"
          square
          icon="i-lucide-refresh-cw"
          :loading="isLoading"
          :aria-label="t('docker.refresh')"
          @click="refresh"
        >
          <!-- 加载中让出插槽，Nuxt UI 自带的转圈图标才会出来 -->
          <template v-if="!isLoading" #leading="{ ui }">
            <MotionIcon :icon="RefreshCw" animation="lucide-animated" hover :class="ui.leadingIcon()" />
          </template>
        </UButton>
      </UTooltip>
    </header>

    <div v-if="activeContainer" class="container-picker-current">
      <UIcon name="i-lucide-container" />
      <span>{{ t("docker.browsing", { name: activeContainer.name }) }}</span>
      <UButton
        v-if="activeContainer.id"
        color="neutral"
        variant="soft"
        size="xs"
        icon="i-lucide-scroll-text"
        :label="t('docker.logs')"
        @click="emit('logs', activeContainer)"
      />
      <UButton
        color="neutral"
        variant="soft"
        size="xs"
        icon="i-lucide-log-out"
        :label="t('docker.exit')"
        @click="emit('exit')"
      />
    </div>

    <UAlert
      v-if="error"
      class="container-picker-alert"
      color="error"
      variant="soft"
      icon="i-lucide-circle-alert"
      :title="error"
    />
    <UAlert
      v-else-if="!isLoading && !isRoot"
      class="container-picker-alert"
      color="warning"
      variant="soft"
      icon="i-lucide-shield-alert"
      :title="t('docker.notRootTitle')"
      :description="t('docker.notRoot')"
    />

    <div class="container-picker-list">
      <p v-if="isLoading && !containers.length" class="container-picker-empty">
        <UIcon name="i-lucide-loader-circle" class="spin-icon" />
        {{ t("docker.loading") }}
      </p>
      <p v-else-if="!error && !containers.length" class="container-picker-empty">
        {{ t("docker.empty") }}
      </p>
      <template v-for="group in groups" :key="group.key">
        <p v-if="group.label" class="container-picker-group">{{ group.label }}</p>
        <div
          v-for="container in group.items"
          :key="container.id"
          class="container-picker-row"
          :class="{ active: container.root && activeContainer?.root === container.root, stopped: !container.root }"
        >
          <UButton
            class="container-picker-item"
            color="neutral"
            variant="ghost"
            block
            :icon="container.root ? 'i-lucide-box' : 'i-lucide-package-x'"
            :title="container.root ? undefined : t('docker.stoppedHint')"
            :ui="{ base: 'justify-start text-left' }"
            @click="pick(container)"
          >
            <span class="container-picker-text">
              <strong>{{ container.name }}</strong>
              <span>{{ container.image }}</span>
            </span>
            <span class="container-picker-state">{{ stateLabel(container) }}</span>
          </UButton>
          <UTooltip :text="t('docker.logs')">
            <UButton
              class="container-picker-logs"
              color="neutral"
              variant="ghost"
              size="xs"
              square
              icon="i-lucide-scroll-text"
              :aria-label="t('docker.logsOf', { name: container.name })"
              @click="emit('logs', container)"
            />
          </UTooltip>
        </div>
      </template>
    </div>

    <p class="container-picker-note">{{ t("docker.note") }}</p>
  </section>
</template>

<style src="../../assets/css/container-picker.css"></style>
