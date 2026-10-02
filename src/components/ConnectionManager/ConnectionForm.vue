<script setup>
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { useProfileStore } from "../../stores/profileStore";
import { useCredentialStore } from "../../stores/credentialStore";
import { testConnection as testConnectionRequest } from "../../services/workspaceApi";
import { errorCode, localizeError } from "../../utils/appError";
import { normalizeAgentConfig } from "../../utils/sshAgent";
import AgentSettings from "./AgentSettings.vue";
import OsIconPicker from "./OsIconPicker.vue";
import { OS_ICON_AUTO } from "../../config/osIcons";
import { usesSavedCredentials } from "../../utils/authentication";
import { useAuthenticationStore } from "../../stores/authenticationStore";

const props = defineProps({
  profileId: {
    type: String,
    default: null,
  },
});

const emit = defineEmits(["close"]);
const { t } = useI18n();
const profileStore = useProfileStore();
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
let testGeneration = 0;
let testAuthentication = null;
const testOwner = `test:${crypto.randomUUID()}`;

const defaultGroupId = computed(() => profileStore.activeGroups[0]?.id ?? "group-default");
const editingProfile = computed(() =>
  props.profileId ? profileStore.profiles.find((profile) => profile.id === props.profileId) : null,
);
const isEditingProfile = computed(() => Boolean(editingProfile.value));
const hasSavedCredential = computed(() => usesSavedCredentials(editingProfile.value?.authMethod)
  && Boolean(editingProfile.value?.credentialRef));
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
// 四个选项排成一行的分段控件，只放文字：带图标的话「交互认证 / 2FA」挤不下一格
const authMethodItems = computed(() => [
  { value: "password", label: t("profile.passwordAuth") },
  { value: "privateKey", label: t("profile.privateKeyAuth") },
  { value: "agent", label: t("profile.agentAuth") },
  { value: "keyboardInteractive", label: t("profile.interactiveAuth") },
]);
// 认证卡片下面那行说明；agent 的说明和报错由 AgentSettings 自己写在它的卡片下面
const authFooter = computed(() => {
  if (usesSavedCredentials(form.authMethod)) return saveCredentialDescription.value;
  if (form.authMethod === "keyboardInteractive") return t("profile.interactiveHint");
  return "";
});

const form = reactive({
  name: "",
  osIcon: OS_ICON_AUTO,
  groupId: "",
  host: "",
  port: 22,
  username: "",
  authMethod: "password",
  privateKeyPath: "",
  agent: normalizeAgentConfig(),
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

// 分组创建和后台更新都会替换快照，同一连接的快照刷新不能覆盖尚未保存的输入。
watch(
  [() => props.profileId, () => editingProfile.value?.id],
  () => {
    resetForm();
  },
  { immediate: true },
);

// 凭据不能跨编辑对象复用，切换连接时连同临时测试结果一起清空。
function resetForm() {
  const profile = editingProfile.value;

  form.name = profile?.name ?? "";
  form.osIcon = profile?.osIcon ?? OS_ICON_AUTO;
  form.groupId = profile?.groupId ?? defaultGroupId.value;
  form.host = profile?.host ?? "";
  form.port = profile?.port ?? 22;
  form.username = profile?.username ?? "";
  form.authMethod = profile?.authMethod ?? "password";
  form.privateKeyPath = profile?.privateKeyPath ?? "";
  form.agent = normalizeAgentConfig(profile?.agent);
  form.defaultRemotePath = profile?.defaultRemotePath ?? "/";
  secretPassword.value = "";
  passphrase.value = "";
  shouldSaveCredential.value = hasSavedCredential.value;
  closeInlineGroupCreator();
  error.value = "";
  connectionTestResult.value = null;
  saveProgressKey.value = "";
}

function groupDisplayName(group) {
  return group.id === "group-default" ? t("sidebar.ungrouped") : group.name;
}

// 底栏只放得下「已连通 · 32 ms」，服务器指纹放进悬停提示，要核对时再看
const testStatusText = computed(() =>
  connectionTestResult.value
    ? t("profile.testConnectedShort", { latency: connectionTestResult.value.latencyMs })
    : "",
);
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

// 保存使用点击时的快照，凭据落盘期间修改认证方式不能混入同一次提交。
async function submitProfile() {
  error.value = "";

  if (isSavingProfile.value) return;

  if (!canSave.value) {
    error.value = t("profile.required");
    return;
  }

  isSavingProfile.value = true;
  const draft = { ...form, agent: normalizeAgentConfig(form.agent) };
  const credentialDraft = {
    authMethod: draft.authMethod,
    shouldSave: shouldSaveCredential.value,
    password: secretPassword.value,
    passphrase: passphrase.value,
  };
  try {
    const now = Date.now().toString();
    const currentProfile = editingProfile.value;
    const profileId = currentProfile?.id ?? crypto.randomUUID();
    const credentialRef = await resolveCredentialRef(currentProfile, profileId, credentialDraft);

    await runSaveStep("profile.saveProgress.saveProfile", () =>
      profileStore.saveProfile({
        id: profileId,
        syncId: currentProfile?.syncId ?? profileId,
        groupId: draft.groupId,
        name: draft.name.trim(),
        // 「自动」不落具体值：以后换了识别规则，旧连接也能跟着新结果走
        osIcon: draft.osIcon === OS_ICON_AUTO ? null : draft.osIcon,
        host: draft.host.trim(),
        port: Number(draft.port),
        username: draft.username.trim(),
        authMethod: draft.authMethod,
        privateKeyPath: draft.authMethod === "privateKey" ? draft.privateKeyPath : null,
        agent: draft.authMethod === "agent" ? draft.agent : null,
        credentialRef,
        defaultRemotePath: draft.defaultRemotePath || "/",
        ...(!currentProfile ? {
          sortOrder: Math.max(-1, ...profileStore.activeProfiles.map((profile) => profile.sortOrder)) + 1,
          favorite: false,
          lastConnectedAt: null,
          createdAt: now,
          updatedAt: now,
          deletedAt: null,
        } : {}),
      }),
    );

    emit("close");
  } catch (submitError) {
    // 凭据存储相关的失败额外加一句上下文：多半是本机密钥出了问题，不是填错了
    error.value = errorCode(submitError).startsWith("credential.")
      ? `${t("profile.credentialStoreUnavailable")}: ${localizeError(submitError)}`
      : localizeError(submitError);
  } finally {
    saveProgressKey.value = "";
    isSavingProfile.value = false;
  }
}

async function resolveCredentialRef(currentProfile, profileId, credentials) {
  // 无本地凭据的模式只保留后端清理重试引用，不能当作仍可使用的已保存密码。
  if (!usesSavedCredentials(credentials.authMethod)) return null;
  const currentCredentialRef = usesSavedCredentials(currentProfile?.authMethod)
    ? (currentProfile?.credentialRef ?? null) : null;

  if (!credentials.shouldSave) {
    if (currentCredentialRef) {
      await runSaveStep("profile.saveProgress.removeCredential", async () => {
        await credentialStore.removeCredential(currentCredentialRef, ["password", "passphrase"]);
      });
    }

    return null;
  }

  if (credentials.authMethod === "password") {
    if (!credentials.password && !currentCredentialRef) {
      throw new Error(t("profile.savePasswordRequired"));
    }

    if (!credentials.password) return currentCredentialRef;

    const credentialRef = currentCredentialRef || profileId;
    await runSaveStep("profile.saveProgress.saveCredential", () =>
      credentialStore.saveCredential(credentialRef, { password: credentials.password }),
    );
    return credentialRef;
  }

  if (!credentials.passphrase) return currentCredentialRef;

  const credentialRef = currentCredentialRef || profileId;
  await runSaveStep("profile.saveProgress.saveCredential", () =>
    credentialStore.saveCredential(credentialRef, { passphrase: credentials.passphrase }),
  );
  return credentialRef;
}

async function runSaveStep(stepKey, task) {
  saveProgressKey.value = stepKey;
  const startedAt = performance.now();

  try {
    return await task();
  } finally {
    console.info("[Xin SSH] connection save step", {
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
    agent: form.authMethod === "agent" ? normalizeAgentConfig(form.agent) : null,
  };
}

// agent 确认可能持续较久，编辑目标或关闭表单后旧测试结果不能标记新配置为成功。
watch(() => [
  props.profileId, form.host, form.port, form.username, form.authMethod,
  form.privateKeyPath, secretPassword.value, passphrase.value,
  form.agent.provider, form.agent.socketPath, form.agent.keyFingerprint,
], () => {
  testGeneration += 1;
  connectionTestResult.value = null;
  testAuthentication?.close();
}, { flush: "sync" });
onBeforeUnmount(() => {
  testGeneration += 1;
  testAuthentication?.close();
});

async function testCurrentConnection() {
  if (isTestingConnection.value) return;
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
  const generation = ++testGeneration;
  const request = buildConnectionTestRequest();
  let authentication;
  try {
    authentication = useAuthenticationStore().createFlow(testOwner, {
      host: request.host, port: request.port, username: request.username, name: form.name, purpose: "test",
    });
    testAuthentication = authentication;
    const result = await testConnectionRequest(request, authentication.channel);
    if (generation === testGeneration) connectionTestResult.value = result;
  } catch (testError) {
    if (generation === testGeneration) error.value = `${t("profile.testFailed")}: ${localizeError(testError)}`;
  } finally {
    authentication?.close();
    if (testAuthentication === authentication) testAuthentication = null;
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

  isSavingGroup.value = true;
  try {
    form.groupId = await profileStore.createGroup(name);
    closeInlineGroupCreator();
  } catch (groupError) {
    error.value = `${t("sidebar.createGroupFailed")}: ${localizeError(groupError)}`;
  } finally {
    isSavingGroup.value = false;
  }
}
</script>

<template>
  <div class="modal-backdrop">
    <form class="connection-form liquid-sheet" @submit.prevent="submitProfile">
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
        <div class="identity-card">
          <div class="identity-avatar">
            <OsIconPicker v-model="form.osIcon" :detected-os="editingProfile?.detectedOs ?? null" />
          </div>
          <div class="form-list identity-rows">
            <label class="form-row">
              <span class="form-row-label">{{ t("profile.name") }}</span>
              <UInput
                v-model="form.name"
                variant="none"
                class="w-full"
                :placeholder="t('profile.namePlaceholder')"
              />
            </label>
            <div class="form-row">
              <span class="form-row-label">{{ t("profile.group") }}</span>
              <USelect v-model="form.groupId" variant="none" class="w-full" :items="groupSelectItems" />
              <UButton
                color="neutral"
                variant="ghost"
                size="sm"
                square
                icon="i-lucide-folder-plus"
                :aria-label="t('sidebar.addGroup')"
                :title="t('sidebar.addGroup')"
                @click="openInlineGroupCreator"
              />
            </div>
            <div v-if="isGroupCreatorOpen" class="form-row">
              <span class="form-row-label">{{ t("sidebar.addGroup") }}</span>
              <UInput
                v-model="newGroupName"
                variant="none"
                class="w-full"
                :placeholder="t('sidebar.groupName')"
                autofocus
                @keydown.enter.prevent="createInlineGroup"
              />
              <div class="group-creator-actions">
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
            </div>
          </div>
        </div>

        <section class="form-section">
          <h3 class="form-section-title">{{ t("profile.sectionTarget") }}</h3>
          <div class="form-list">
            <label class="form-row">
              <span class="form-row-label">{{ t("profile.host") }}</span>
              <UInput
                v-model="form.host"
                variant="none"
                class="w-full"
                spellcheck="false"
                :placeholder="t('profile.hostPlaceholder')"
              />
            </label>
            <label class="form-row">
              <span class="form-row-label">{{ t("profile.port") }}</span>
              <UInputNumber
                v-model="form.port"
                variant="none"
                class="w-full"
                :min="1"
                :max="65535"
                :increment="false"
                :decrement="false"
                :format-options="{ useGrouping: false }"
              />
            </label>
            <label class="form-row">
              <span class="form-row-label">{{ t("profile.username") }}</span>
              <UInput v-model="form.username" variant="none" class="w-full" spellcheck="false" placeholder="root" />
            </label>
          </div>
        </section>

        <section class="form-section">
          <h3 class="form-section-title">{{ t("profile.sectionAuth") }}</h3>
          <div class="auth-methods" role="group" :aria-label="t('profile.authMethod')">
            <UButton
              v-for="method in authMethodItems"
              :key="method.value"
              color="neutral"
              variant="ghost"
              :class="{ active: form.authMethod === method.value }"
              :aria-pressed="form.authMethod === method.value"
              :label="method.label"
              @click="form.authMethod = method.value"
            />
          </div>

          <div v-if="usesSavedCredentials(form.authMethod)" class="form-list">
            <label v-if="form.authMethod === 'password'" class="form-row">
              <span class="form-row-label">{{ t("profile.password") }}</span>
              <UInput
                v-model="secretPassword"
                type="password"
                variant="none"
                class="w-full"
                autocomplete="new-password"
                :placeholder="hasSavedCredential ? t('profile.savedPasswordPlaceholder') : ''"
              />
            </label>
            <template v-else>
              <label class="form-row">
                <span class="form-row-label">{{ t("profile.privateKeyPath") }}</span>
                <UInput
                  v-model="form.privateKeyPath"
                  variant="none"
                  class="w-full"
                  spellcheck="false"
                  :placeholder="t('profile.privateKeyPlaceholder')"
                />
                <UButton
                  color="neutral"
                  variant="soft"
                  size="sm"
                  :label="t('profile.choosePrivateKey')"
                  @click="choosePrivateKey"
                />
              </label>
              <label class="form-row">
                <span class="form-row-label">{{ t("profile.passphraseShort") }}</span>
                <UInput
                  v-model="passphrase"
                  type="password"
                  variant="none"
                  class="w-full"
                  autocomplete="new-password"
                  :placeholder="hasSavedCredential ? t('profile.savedPassphrasePlaceholder') : t('profile.passphrasePlaceholder')"
                />
              </label>
            </template>
            <label class="form-row form-row-toggle">
              <span class="form-row-label">{{ saveCredentialLabel }}</span>
              <USwitch v-model="shouldSaveCredential" />
            </label>
          </div>

          <AgentSettings v-else-if="form.authMethod === 'agent'" v-model="form.agent" />

          <p v-if="authFooter" class="form-section-footer">{{ authFooter }}</p>
        </section>

        <section class="form-section">
          <h3 class="form-section-title">{{ t("profile.sectionFiles") }}</h3>
          <div class="form-list">
            <label class="form-row">
              <span class="form-row-label">{{ t("profile.defaultRemotePath") }}</span>
              <UInput v-model="form.defaultRemotePath" variant="none" class="w-full" spellcheck="false" />
            </label>
          </div>
          <p class="form-section-footer">{{ t("profile.defaultRemotePathHint") }}</p>
        </section>

        <p v-if="error" class="form-error">{{ error }}</p>
        <p v-else-if="saveProgressMessage" class="form-progress">{{ saveProgressMessage }}</p>
      </div>

      <footer>
        <UButton
          color="neutral"
          variant="ghost"
          size="lg"
          icon="i-lucide-plug-zap"
          :loading="isTestingConnection"
          :disabled="!canSave"
          :label="isTestingConnection ? t('profile.testing') : t('profile.test')"
          @click="testCurrentConnection"
        />
        <span v-if="connectionTestResult" class="connection-test-status" :title="testResultMessage">
          {{ testStatusText }}
        </span>
        <UButton
          color="neutral"
          variant="outline"
          size="lg"
          class="footer-cancel"
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

<style scoped src="../../assets/css/connection-form.css"></style>
