<script setup>
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { localizeError } from "../../utils/appError";

const props = defineProps({
  open: {
    type: Boolean,
    default: false,
  },
  group: {
    type: Object,
    default: null,
  },
});

// confirm 携带 ({ deleteProfiles: boolean }, { done, fail })
const emit = defineEmits(["close", "confirm"]);
const { t } = useI18n();
const profileStore = useProfileStore();
const sessionStore = useSessionStore();

const deleteMode = ref("move");
const error = ref("");
const isSubmitting = ref(false);

const profiles = computed(() => props.group ? profileStore.profilesInGroup(props.group.id) : []);
const profileCount = computed(() => profiles.value.length);
const openSessionCount = computed(() => profiles.value.reduce(
  (count, profile) => count + sessionStore.sessionCountForProfile(profile.id), 0,
));
const hasSavedCredentials = computed(() => profiles.value.some((profile) => profile.credentialRef));
const deleteOptions = computed(() => [
  {
    value: "move",
    label: t("home.deleteGroup.moveTitle"),
    description: t("home.deleteGroup.moveDescription"),
  },
  {
    value: "deleteAll",
    label: t("home.deleteGroup.deleteAllTitle"),
    description: t("home.deleteGroup.deleteAllDescription", { count: profileCount.value }),
  },
]);
const isEmpty = computed(() => profileCount.value === 0);
const confirmLabel = computed(() =>
  deleteMode.value === "deleteAll"
    ? t("home.deleteGroup.confirmAll")
    : t("home.deleteConnection.confirm"),
);
const confirmColor = computed(() => (deleteMode.value === "deleteAll" ? "error" : "primary"));

watch(
  () => props.open,
  (open) => {
    if (!open) return;

    deleteMode.value = "move";
    error.value = "";
    isSubmitting.value = false;
  },
);

function close() {
  if (isSubmitting.value) return;

  emit("close");
}

// 等待父组件完成整组删除后再解锁，失败时保留处理选项供用户重试。
function confirm() {
  if (isSubmitting.value || !props.group) return;

  isSubmitting.value = true;
  error.value = "";
  emit(
    "confirm",
    { deleteProfiles: deleteMode.value === "deleteAll" },
    {
      done: () => {
        isSubmitting.value = false;
        emit("close");
      },
      fail: (confirmError) => {
        isSubmitting.value = false;
        error.value = localizeError(confirmError);
      },
    },
  );
}
</script>

<template>
  <UModal
    :open="open"
    :title="t('sidebar.deleteGroup')"
    :description="group ? t('home.deleteGroup.description', { name: group.name }) : ''"
    :ui="{ footer: 'justify-end' }"
    :dismissible="!isSubmitting"
    :close="!isSubmitting"
    @update:open="(value) => !value && close()"
  >
    <template #body>
      <div v-if="group" class="home-dialog-form">
        <p v-if="isEmpty" class="home-dialog-hint">{{ t("home.deleteGroup.empty") }}</p>
        <template v-else>
          <p class="home-dialog-hint">
            {{ t("home.deleteGroup.hasProfiles", { count: profileCount }) }}
          </p>
          <URadioGroup
            v-model="deleteMode"
            :items="deleteOptions"
            :disabled="isSubmitting"
            :color="deleteMode === 'deleteAll' ? 'error' : 'primary'"
            variant="card"
            :ui="{ item: 'rounded-md', wrapper: 'min-w-0', label: 'break-words', description: 'break-words' }"
          />
          <UAlert
            v-if="deleteMode === 'deleteAll' && openSessionCount > 0"
            color="warning"
            variant="soft"
            icon="i-lucide-square-terminal"
            :title="t('home.deleteGroup.openSessions', { count: openSessionCount })"
          />
          <UAlert
            v-if="deleteMode === 'deleteAll' && hasSavedCredentials"
            color="neutral"
            variant="soft"
            icon="i-lucide-key-round"
            :title="t('home.deleteConnection.credentialRemoved')"
          />
        </template>
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
        :color="confirmColor"
        icon="i-lucide-trash-2"
        :loading="isSubmitting"
        :label="confirmLabel"
        @click="confirm"
      />
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/home-dialogs.css"></style>
