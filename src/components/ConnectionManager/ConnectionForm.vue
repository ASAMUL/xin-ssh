<script setup>
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { useProfileStore } from "../../stores/profileStore";
import { useAppStore } from "../../stores/appStore";
import { useCredentialStore } from "../../stores/credentialStore";
import { testConnection as testConnectionRequest } from "../../services/workspaceApi";

const props = defineProps({
  profileId: {
    type: String,
    default: null,
  },
});

const emit = defineEmits(["close"]);
const { t } = useI18n();
const profileStore = useProfileStore();
const appStore = useAppStore();
const credentialStore = useCredentialStore();

const error = ref("");
const secretPassword = ref("");
const passphrase = ref("");
const shouldSaveCredential = ref(false);
const isTestingConnection = ref(false);
const isSavingProfile = ref(false);
const isGroupCreatorOpen = ref(false);
const isSavingGroup = ref(false);
const connectionTestResult = ref(null);
const saveProgressKey = ref("");
const newGroupName = ref("");

const defaultGroupId = computed(() => profileStore.activeGroups[0]?.id ?? "group-default");
const editingProfile = computed(() =>
  props.profileId ? profileStore.profiles.find((profile) => profile.id === props.profileId) : null,
);
const isEditingProfile = computed(() => Boolean(editingProfile.value));
const hasSavedCredential = computed(() => Boolean(editingProfile.value?.credentialRef));
const formTitle = computed(() => t(isEditingProfile.value ? "profile.editTitle" : "profile.newTitle"));
const saveButtonText = computed(() => {
  if (isSavingProfile.value) return t("profile.saving");

  return t(isEditingProfile.value ? "profile.update" : "profile.save");
});
const saveProgressMessage = computed(() => (saveProgressKey.value ? t(saveProgressKey.value) : ""));
const groupOptions = computed(() => profileStore.activeGroups);
const groupSelectItems = computed(() =>
  groupOptions.value.map((group) => ({ label: groupDisplayName(group), value: group.id })),
);
const saveCredentialLabel = computed(() =>
  t(form.authMethod === "password" ? "profile.savePassword" : "profile.savePassphrase"),
);
const saveCredentialDescription = computed(() =>
  hasSavedCredential.value
    ? t("profile.credentialSavedHint")
    : t("profile.saveCredentialDescription"),
);

const form = reactive({
  name: "",
  groupId: "",
  host: "",
  port: 22,
  username: "",
  authMethod: "password",
  privateKeyPath: "",
  defaultRemotePath: "/",
});

const canSave = computed(
  () =>
    form.name.trim() &&
    form.groupId &&
    form.host.trim() &&
    form.username.trim() &&
    Number(form.port) > 0,
);

watch(
  [editingProfile, defaultGroupId],
  () => {
    resetForm();
  },
  { immediate: true },
);

function resetForm() {
  const profile = editingProfile.value;

  form.name = profile?.name ?? "";
  form.groupId = profile?.groupId ?? defaultGroupId.value;
  form.host = profile?.host ?? "";
  form.port = profile?.port ?? 22;
  form.username = profile?.username ?? "";
  form.authMethod = profile?.authMethod ?? "password";
  form.privateKeyPath = profile?.privateKeyPath ?? "";
  form.defaultRemotePath = profile?.defaultRemotePath ?? "/";
  secretPassword.value = "";
  passphrase.value = "";
  shouldSaveCredential.value = Boolean(profile?.credentialRef);
  closeInlineGroupCreator();
  error.value = "";
  connectionTestResult.value = null;
  saveProgressKey.value = "";
}

function groupDisplayName(group) {
  return group.name === "Ungrouped" ? t("sidebar.ungrouped") : group.name;
}

const testResultMessage = computed(() => {
  if (!connectionTestResult.value) return "";

  return t("profile.testSuccess", {
    latency: connectionTestResult.value.latencyMs,
    fingerprint: connectionTestResult.value.serverFingerprint ?? t("profile.noFingerprint"),
  });
});

async function choosePrivateKey() {
  const selected = await open({
    multiple: false,
    directory: false,
    title: t("profile.choosePrivateKey"),
  });

  if (typeof selected === "string") {
    form.privateKeyPath = selected;
  }
}

async function submitProfile() {
  error.value = "";

  if (isSavingProfile.value) return;

  if (!canSave.value) {
    error.value = t("profile.required");
    return;
  }

  isSavingProfile.value = true;
  try {
    const now = Date.now().toString();
    const currentProfile = editingProfile.value;
    const profileId = currentProfile?.id ?? crypto.randomUUID();
    const credentialRef = await resolveCredentialRef(currentProfile, profileId);

    const workspaceSnapshot = await runSaveStep("profile.saveProgress.saveProfile", () =>
      profileStore.saveProfile({
        id: profileId,
        syncId: currentProfile?.syncId ?? profileId,
        groupId: form.groupId || defaultGroupId.value,
        name: form.name.trim(),
        host: form.host.trim(),
        port: Number(form.port),
        username: form.username.trim(),
        authMethod: form.authMethod,
        privateKeyPath: form.authMethod === "privateKey" ? form.privateKeyPath : null,
        credentialRef,
        defaultRemotePath: form.defaultRemotePath || "/",
        sortOrder: currentProfile?.sortOrder ?? profileStore.activeProfiles.length,
        favorite: currentProfile?.favorite ?? false,
        lastConnectedAt: currentProfile?.lastConnectedAt ?? null,
        createdAt: currentProfile?.createdAt ?? now,
        updatedAt: now,
        deletedAt: currentProfile?.deletedAt ?? null,
      }),
    );

    appStore.consumeSnapshot(workspaceSnapshot);
    emit("close");
  } catch (submitError) {
    const message = String(submitError);
    error.value = message.includes("Credential store")
      ? `${t("profile.credentialStoreUnavailable")}: ${message}`
      : message;
  } finally {
    saveProgressKey.value = "";
    isSavingProfile.value = false;
  }
}

async function resolveCredentialRef(currentProfile, profileId) {
  const currentCredentialRef = currentProfile?.credentialRef ?? null;

  if (!shouldSaveCredential.value) {
    if (currentCredentialRef) {
      await runSaveStep("profile.saveProgress.removeCredential", async () => {
        await credentialStore.removeCredential(currentCredentialRef, ["password", "passphrase"]);
      });
    }

    return null;
  }

  if (form.authMethod === "password") {
    if (!secretPassword.value && !currentCredentialRef) {
      throw new Error(t("profile.savePasswordRequired"));
    }

    if (!secretPassword.value) return currentCredentialRef;

    const credentialRef = currentCredentialRef || profileId;
    await runSaveStep("profile.saveProgress.saveCredential", () =>
      credentialStore.saveCredential(credentialRef, { password: secretPassword.value }),
    );
    return credentialRef;
  }

  if (!passphrase.value) return currentCredentialRef;

  const credentialRef = currentCredentialRef || profileId;
  await runSaveStep("profile.saveProgress.saveCredential", () =>
    credentialStore.saveCredential(credentialRef, { passphrase: passphrase.value }),
  );
  return credentialRef;
}

async function runSaveStep(stepKey, task) {
  saveProgressKey.value = stepKey;
  const startedAt = performance.now();

  try {
    return await task();
  } finally {
    console.info("[Xin Terminal] connection save step", {
      step: stepKey,
      durationMs: Math.round(performance.now() - startedAt),
    });
  }
}

function buildConnectionTestRequest() {
  return {
    host: form.host.trim(),
    port: Number(form.port),
    username: form.username.trim(),
    authMethod: form.authMethod,
    password: form.authMethod === "password" ? secretPassword.value : null,
    privateKeyPath: form.authMethod === "privateKey" ? form.privateKeyPath.trim() : null,
    passphrase: form.authMethod === "privateKey" ? passphrase.value : null,
  };
}

async function testCurrentConnection() {
  error.value = "";
  connectionTestResult.value = null;

  if (!canSave.value) {
    error.value = t("profile.required");
    return;
  }

  if (form.authMethod === "password" && !secretPassword.value) {
    error.value = t("profile.testPasswordRequired");
    return;
  }

  if (form.authMethod === "privateKey" && !form.privateKeyPath.trim()) {
    error.value = t("profile.testPrivateKeyRequired");
    return;
  }

  isTestingConnection.value = true;
  try {
    connectionTestResult.value = await testConnectionRequest(buildConnectionTestRequest());
  } catch (testError) {
    error.value = `${t("profile.testFailed")}: ${String(testError)}`;
  } finally {
    isTestingConnection.value = false;
  }
}

function openInlineGroupCreator() {
  isGroupCreatorOpen.value = true;
}

function closeInlineGroupCreator() {
  isGroupCreatorOpen.value = false;
  newGroupName.value = "";
}

async function createInlineGroup() {
  const name = newGroupName.value.trim();
  if (!name || isSavingGroup.value) return;

  const now = Date.now().toString();
  const groupId = crypto.randomUUID();

  isSavingGroup.value = true;
  try {
    await profileStore.saveGroup({
      id: groupId,
      syncId: groupId,
      name,
      sortOrder: profileStore.activeGroups.length,
      collapsed: false,
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    });
    form.groupId = groupId;
    closeInlineGroupCreator();
  } catch (groupError) {
    error.value = `${t("sidebar.createGroupFailed")}: ${String(groupError)}`;
  } finally {
    isSavingGroup.value = false;
  }
}
</script>

<template>
  <div class="modal-backdrop">
    <form class="connection-form" @submit.prevent="submitProfile">
      <header>
        <h2>{{ formTitle }}</h2>
        <UButton
          color="neutral"
          variant="ghost"
          square
          icon="i-lucide-x"
          :aria-label="t('profile.cancel')"
          @click="emit('close')"
        />
      </header>

      <div class="form-body">
        <div class="form-grid">
          <UFormField :label="t('profile.name')">
            <UInput v-model="form.name" size="lg" class="w-full" />
          </UFormField>
          <UFormField :label="t('profile.group')">
            <div class="inline-field">
              <USelect v-model="form.groupId" size="lg" class="w-full" :items="groupSelectItems" />
              <UButton
                color="primary"
                variant="soft"
                size="lg"
                square
                icon="i-lucide-folder-plus"
                :aria-label="t('sidebar.addGroup')"
                :title="t('sidebar.addGroup')"
                @click="openInlineGroupCreator"
              />
            </div>
          </UFormField>
        </div>

        <div v-if="isGroupCreatorOpen" class="form-group-creator">
          <UIcon name="i-lucide-folder-plus" />
          <UInput
            v-model="newGroupName"
            size="sm"
            class="w-full"
            :placeholder="t('sidebar.groupName')"
            autofocus
            @keydown.enter.prevent="createInlineGroup"
          />
          <UButton
            color="primary"
            variant="soft"
            size="sm"
            square
            icon="i-lucide-check"
            :loading="isSavingGroup"
            :disabled="!newGroupName.trim()"
            :aria-label="t('sidebar.addGroup')"
            @click="createInlineGroup"
          />
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-x"
            :aria-label="t('profile.cancel')"
            @click="closeInlineGroupCreator"
          />
        </div>

        <div class="form-grid host-grid">
          <UFormField :label="t('profile.host')">
            <UInput v-model="form.host" size="lg" class="w-full" />
          </UFormField>
          <UFormField :label="t('profile.port')">
            <UInputNumber
              v-model="form.port"
              size="lg"
              class="w-full"
              :min="1"
              :max="65535"
              :format-options="{ useGrouping: false }"
            />
          </UFormField>
          <UFormField :label="t('profile.username')">
            <UInput v-model="form.username" size="lg" class="w-full" />
          </UFormField>
        </div>

        <div class="segmented-control">
          <button
            type="button"
            :class="{ active: form.authMethod === 'password' }"
            @click="form.authMethod = 'password'"
          >
            <UIcon name="i-lucide-key-round" />
            <span>{{ t("profile.passwordAuth") }}</span>
          </button>
          <button
            type="button"
            :class="{ active: form.authMethod === 'privateKey' }"
            @click="form.authMethod = 'privateKey'"
          >
            <UIcon name="i-lucide-file-key-2" />
            <span>{{ t("profile.privateKeyAuth") }}</span>
          </button>
        </div>

        <UFormField v-if="form.authMethod === 'password'" :label="t('profile.password')">
          <UInput
            v-model="secretPassword"
            type="password"
            size="lg"
            class="w-full"
            autocomplete="new-password"
            :placeholder="hasSavedCredential ? t('profile.savedPasswordPlaceholder') : ''"
          />
        </UFormField>

        <div v-else class="key-fields">
          <UFormField :label="t('profile.privateKeyPath')">
            <div class="inline-field">
              <UInput v-model="form.privateKeyPath" size="lg" class="w-full" />
              <UButton
                color="primary"
                variant="soft"
                size="lg"
                :label="t('profile.choosePrivateKey')"
                @click="choosePrivateKey"
              />
            </div>
          </UFormField>
          <UFormField :label="t('profile.passphrase')">
            <UInput
              v-model="passphrase"
              type="password"
              size="lg"
              class="w-full"
              autocomplete="new-password"
              :placeholder="hasSavedCredential ? t('profile.savedPassphrasePlaceholder') : ''"
            />
          </UFormField>
        </div>

        <UCheckbox
          v-model="shouldSaveCredential"
          class="credential-save-option"
          :label="saveCredentialLabel"
          :description="saveCredentialDescription"
        />

        <UFormField :label="t('profile.defaultRemotePath')">
          <UInput v-model="form.defaultRemotePath" size="lg" class="w-full" />
        </UFormField>

        <div class="connection-test-panel" :class="{ success: connectionTestResult }">
          <UButton
            color="neutral"
            variant="outline"
            size="lg"
            class="test-action"
            icon="i-lucide-plug-zap"
            :loading="isTestingConnection"
            :disabled="!canSave"
            :label="isTestingConnection ? t('profile.testing') : t('profile.test')"
            @click="testCurrentConnection"
          />
          <p v-if="connectionTestResult" class="connection-test-result">
            <strong>{{ t("profile.testConnected") }}</strong>
            <span>{{ testResultMessage }}</span>
          </p>
        </div>

        <p v-if="error" class="form-error">{{ error }}</p>
        <p v-else-if="saveProgressMessage" class="form-progress">{{ saveProgressMessage }}</p>
      </div>

      <footer>
        <UButton
          color="neutral"
          variant="outline"
          size="lg"
          :label="t('profile.cancel')"
          @click="emit('close')"
        />
        <UButton
          type="submit"
          color="primary"
          size="lg"
          class="cta-button"
          icon="i-lucide-save"
          :loading="isSavingProfile"
          :disabled="!canSave"
          :label="saveButtonText"
        />
      </footer>
    </form>
  </div>
</template>
