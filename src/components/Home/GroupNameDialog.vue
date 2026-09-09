<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { localizeError } from "../../utils/appError";

const props = defineProps({
  open: {
    type: Boolean,
    default: false,
  },
  // create | rename
  mode: {
    type: String,
    default: "create",
  },
  initialName: {
    type: String,
    default: "",
  },
});

// submit 携带 (name, { done, fail })：父组件完成后调 done() 关窗，出错调 fail(error) 就地展示
const emit = defineEmits(["close", "submit"]);
const { t } = useI18n();

const name = ref("");
const error = ref("");
const isSubmitting = ref(false);

const isRename = computed(() => props.mode === "rename");
const title = computed(() => t(isRename.value ? "sidebar.renameGroup" : "sidebar.addGroup"));
const submitLabel = computed(() => t(isRename.value ? "common.save" : "sidebar.saveGroup"));
const canSubmit = computed(() => {
  const trimmed = name.value.trim();
  return Boolean(trimmed) && (!isRename.value || trimmed !== props.initialName);
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;

    name.value = props.initialName;
    error.value = "";
    isSubmitting.value = false;
  },
);

function close() {
  if (isSubmitting.value) return;

  emit("close");
}

// 持久化结果到达前保留表单和加载状态，失败后不用重新填写名称。
function submit() {
  if (!canSubmit.value || isSubmitting.value) return;

  isSubmitting.value = true;
  error.value = "";
  emit("submit", name.value.trim(), {
    done: () => {
      isSubmitting.value = false;
      emit("close");
    },
    fail: (submitError) => {
      isSubmitting.value = false;
      error.value = localizeError(submitError);
    },
  });
}
</script>

<template>
  <UModal
    :open="open"
    :title="title"
    :description="t('home.groupDialog.description')"
    :ui="{ footer: 'justify-end' }"
    :dismissible="!isSubmitting"
    :close="!isSubmitting"
    @update:open="(value) => !value && close()"
  >
    <template #body>
      <form class="home-dialog-form" @submit.prevent="submit">
        <UFormField name="groupName" :label="t('sidebar.groupName')">
          <UInput
            v-model="name"
            class="w-full"
            :disabled="isSubmitting"
            color="neutral"
            variant="outline"
            :placeholder="t('sidebar.groupName')"
            autofocus
          />
        </UFormField>
        <UAlert
          v-if="error"
          color="error"
          variant="soft"
          icon="i-lucide-circle-alert"
          :title="error"
        />
      </form>
    </template>
    <template #footer>
      <UButton color="neutral" variant="outline" :disabled="isSubmitting" :label="t('common.cancel')" @click="close" />
      <UButton
        color="primary"
        icon="i-lucide-check"
        :disabled="!canSubmit"
        :loading="isSubmitting"
        :label="submitLabel"
        @click="submit"
      />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/home-dialogs.css"></style>
