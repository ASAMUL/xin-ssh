<script setup>
import { computed, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { useProfileStore } from "../../stores/profileStore";
import { useVaultStore } from "../../stores/vaultStore";
import { useAppStore } from "../../stores/appStore";
import { testConnection as testConnectionRequest } from "../../services/workspaceApi";

const emit = defineEmits(["close"]);
const { t } = useI18n();
const profileStore = useProfileStore();
const vaultStore = useVaultStore();
const appStore = useAppStore();

const error = ref("");
const secretPassword = ref("");
const passphrase = ref("");
const isTestingConnection = ref(false);
const isSavingProfile = ref(false);
const connectionTestResult = ref(null);

const defaultGroupId = computed(() => profileStore.activeGroups[0]?.id ?? "group-default");

const form = reactive({
  name: "",
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
    form.host.trim() &&
    form.username.trim() &&
    Number(form.port) > 0,
);

const secretToSave = computed(() => ({
  password: form.authMethod === "password" ? secretPassword.value : "",
  passphrase: form.authMethod === "privateKey" ? passphrase.value : "",
}));

const needsVault = computed(() => Boolean(secretToSave.value.password || secretToSave.value.passphrase));

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
    if (needsVault.value) {
      await vaultStore.ensureUnlocked();
    }

    const now = Date.now().toString();
    const profileId = crypto.randomUUID();
    const credentialRef = needsVault.value ? `credential-${profileId}` : null;

    if (credentialRef) {
      await vaultStore.saveCredential(credentialRef, secretToSave.value);
    }

    await profileStore.saveProfile({
      id: profileId,
      syncId: profileId,
      groupId: defaultGroupId.value,
      name: form.name.trim(),
      host: form.host.trim(),
      port: Number(form.port),
      username: form.username.trim(),
      authMethod: form.authMethod,
      privateKeyPath: form.authMethod === "privateKey" ? form.privateKeyPath : null,
      credentialRef,
      defaultRemotePath: form.defaultRemotePath || "/",
      sortOrder: profileStore.activeProfiles.length,
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    });

    await appStore.bootstrap();
    emit("close");
  } catch (submitError) {
    const message = String(submitError);
    error.value = message.includes("Credential store")
      ? `${t("profile.credentialStoreUnavailable")}: ${message}`
      : message;
  } finally {
    isSavingProfile.value = false;
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
</script>

<template>
  <div class="modal-backdrop">
    <form class="connection-form" @submit.prevent="submitProfile">
      <header>
        <h2>{{ t("profile.newTitle") }}</h2>
        <button type="button" :aria-label="t('profile.cancel')" @click="emit('close')">
          <UIcon name="i-lucide-x" />
        </button>
      </header>

      <div class="form-grid">
        <label>
          <span>{{ t("profile.name") }}</span>
          <input v-model="form.name" type="text" />
        </label>
        <label>
          <span>{{ t("profile.host") }}</span>
          <input v-model="form.host" type="text" />
        </label>
        <label>
          <span>{{ t("profile.port") }}</span>
          <input v-model="form.port" type="number" min="1" max="65535" />
        </label>
        <label>
          <span>{{ t("profile.username") }}</span>
          <input v-model="form.username" type="text" />
        </label>
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

      <label v-if="form.authMethod === 'password'">
        <span>{{ t("profile.password") }}</span>
        <input v-model="secretPassword" type="password" autocomplete="new-password" />
      </label>

      <div v-else class="key-fields">
        <label>
          <span>{{ t("profile.privateKeyPath") }}</span>
          <div class="inline-field">
            <input v-model="form.privateKeyPath" type="text" />
            <button type="button" @click="choosePrivateKey">{{ t("profile.choosePrivateKey") }}</button>
          </div>
        </label>
        <label>
          <span>{{ t("profile.passphrase") }}</span>
          <input v-model="passphrase" type="password" autocomplete="new-password" />
        </label>
      </div>

      <label>
        <span>{{ t("profile.defaultRemotePath") }}</span>
        <input v-model="form.defaultRemotePath" type="text" />
      </label>

      <div class="connection-test-panel" :class="{ success: connectionTestResult }">
        <button
          type="button"
          class="secondary-action test-action"
          :disabled="!canSave || isTestingConnection"
          @click="testCurrentConnection"
        >
          <UIcon :name="isTestingConnection ? 'i-lucide-loader-circle' : 'i-lucide-plug-zap'" />
          <span>{{ isTestingConnection ? t("profile.testing") : t("profile.test") }}</span>
        </button>
        <p v-if="connectionTestResult" class="connection-test-result">
          <strong>{{ t("profile.testConnected") }}</strong>
          <span>{{ testResultMessage }}</span>
        </p>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>

      <footer>
        <button type="button" class="secondary-action" @click="emit('close')">
          {{ t("profile.cancel") }}
        </button>
        <button type="submit" class="primary-action" :disabled="!canSave || isSavingProfile">
          <UIcon :name="isSavingProfile ? 'i-lucide-loader-circle' : 'i-lucide-save'" />
          <span>{{ isSavingProfile ? t("profile.saving") : t("profile.save") }}</span>
        </button>
      </footer>
    </form>
  </div>
</template>
