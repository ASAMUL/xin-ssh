<script setup>
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
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
const emit = defineEmits(["enter", "exit"]);
const { t } = useI18n();
const containers = ref([]);
const isRoot = ref(true);
const isLoading = ref(false);
const error = ref("");
let requestToken = 0;

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
        />
      </UTooltip>
    </header>

    <div v-if="activeContainer" class="container-picker-current">
      <UIcon name="i-lucide-container" />
      <span>{{ t("docker.browsing", { name: activeContainer.name }) }}</span>
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
      <UButton
        v-for="container in containers"
        :key="container.id"
        class="container-picker-item"
        :class="{ active: activeContainer?.root === container.root }"
        color="neutral"
        variant="ghost"
        block
        icon="i-lucide-box"
        :ui="{ base: 'justify-start text-left' }"
        @click="emit('enter', container)"
      >
        <span class="container-picker-text">
          <strong>{{ container.name }}</strong>
          <span>{{ container.image }}</span>
        </span>
        <span class="container-picker-pid">PID {{ container.pid }}</span>
      </UButton>
    </div>

    <p class="container-picker-note">{{ t("docker.note") }}</p>
  </section>
</template>

<style src="../../assets/css/container-picker.css"></style>
