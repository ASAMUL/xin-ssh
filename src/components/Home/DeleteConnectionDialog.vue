<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "../../stores/sessionStore";
import { localizeError } from "../../utils/appError";

const props = defineProps({
  open: {
    type: Boolean,
    default: false,
  },
  profile: {
    type: Object,
    default: null,
  },
});

// confirm 携带 (profile, { done, fail })：父组件完成后调 done() 关窗，出错调 fail(error) 就地展示
const emit = defineEmits(["close", "confirm"]);
const { t } = useI18n();
const sessionStore = useSessionStore();

const error = ref("");
const isSubmitting = ref(false);

// 该连接当前已打开的终端数：删除会一并关闭，得先告诉用户
const openSessionCount = computed(() =>
  props.profile ? sessionStore.sessionCountForProfile(props.profile.id) : 0,
);
const subtitle = computed(() => {
  const profile = props.profile;
  if (!profile) return "";

  return `${profile.username}@${profile.host}:${profile.port}`;
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;

    error.value = "";
    isSubmitting.value = false;
  },
);

function close() {
  if (isSubmitting.value) return;

  emit("close");
}

// 关闭终端和删除配置都由父组件协调，完成前锁住确认按钮以免重复执行。
function confirm() {
  if (isSubmitting.value || !props.profile) return;

  isSubmitting.value = true;
  error.value = "";
  emit("confirm", props.profile, {
    done: () => {
      isSubmitting.value = false;
      emit("close");
    },
    fail: (confirmError) => {
      isSubmitting.value = false;
      error.value = localizeError(confirmError);
    },
  });
}
</script>

<template>
  <UModal
    :open="open"
    :title="t('sidebar.deleteConnection')"
    :description="t('home.deleteConnection.description')"
    :ui="{ footer: 'justify-end' }"
    :dismissible="!isSubmitting"
    :close="!isSubmitting"
    @update:open="(value) => !value && close()"
  >
    <template #body>
      <div v-if="profile" class="home-dialog-form">
        <div class="home-dialog-preview">
          <UIcon name="i-lucide-server-off" class="home-dialog-preview-icon" />
          <span>
            <strong>{{ profile.name }}</strong>
            <small>{{ subtitle }}</small>
          </span>
        </div>
        <UAlert
          v-if="openSessionCount > 0"
          color="warning"
          variant="soft"
          icon="i-lucide-square-terminal"
          :title="t('home.deleteConnection.openSessions', { count: openSessionCount })"
        />
        <UAlert
          v-if="profile.credentialRef"
          color="neutral"
          variant="soft"
          icon="i-lucide-key-round"
          :title="t('home.deleteConnection.credentialRemoved')"
        />
        <UAlert
          v-if="error"
          color="error"
          variant="soft"
          icon="i-lucide-circle-alert"
          :title="error"
        />
      </div>
    </template>
    <template #footer>
      <UButton color="neutral" variant="outline" :disabled="isSubmitting" :label="t('common.cancel')" @click="close" />
      <UButton
        color="error"
        icon="i-lucide-trash-2"
        :loading="isSubmitting"
        :label="t('home.deleteConnection.confirm')"
        @click="confirm"
      />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/home-dialogs.css"></style>
