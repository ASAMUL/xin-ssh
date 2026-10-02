<script setup>
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { listAgentIdentities } from "../../services/workspaceApi";
import { normalizeAgentConfig } from "../../utils/sshAgent";
import { localizeError } from "../../utils/appError";

const config = defineModel({ type: Object, required: true });
const { t } = useI18n();
const identities = ref([]);
const error = ref("");
const isLoading = ref(false);
const hasLoaded = ref(false);
let generation = 0;
const platform = navigator.userAgentData?.platform ?? navigator.platform ?? "";
const isWindows = platform.toLowerCase().includes("win");
const providerItems = computed(() => [
  { label: t("profile.agentSystem"), value: "system" },
  { label: "Pageant (Windows)", value: "pageant", disabled: !isWindows },
]);
const socketPlaceholder = isWindows ? "\\\\.\\pipe\\openssh-ssh-agent" : "SSH_AUTH_SOCK";
const provider = computed({
  get: () => config.value.provider,
  set: (value) => { config.value = { ...config.value, provider: value, socketPath: null }; },
});
const socketPath = computed({
  get: () => config.value.socketPath ?? "",
  set: (value) => { config.value = { ...config.value, socketPath: value }; },
});
const keyFingerprint = computed({
  get: () => config.value.keyFingerprint || "agent-order",
  set: (value) => {
    config.value = { ...config.value, keyFingerprint: value === "agent-order" ? null : value };
  },
});
const identityItems = computed(() => {
  const items = identities.value.map((identity) => ({
    label: identity.comment || identity.algorithm,
    description: identity.fingerprint,
    value: identity.fingerprint,
  }));
  const selected = config.value.keyFingerprint;
  if (selected && !items.some((item) => item.value === selected)) {
    items.unshift({ label: selected, value: selected });
  }
  return [{ label: t("profile.agentAllKeys"), value: "agent-order" }, ...items];
});

watch(() => [config.value.provider, config.value.socketPath], () => {
  generation += 1;
  identities.value = [];
  error.value = "";
  hasLoaded.value = false;
}, { flush: "sync" });
onBeforeUnmount(() => { generation += 1; });

// 切换来源后不能把旧 agent 返回的身份放入新列表，已选指纹保留供用户核对。
async function refreshIdentities() {
  if (isLoading.value) return;
  const requestGeneration = ++generation;
  isLoading.value = true;
  error.value = "";
  try {
    const result = await listAgentIdentities(normalizeAgentConfig(config.value));
    if (requestGeneration !== generation) return;
    identities.value = result;
    hasLoaded.value = true;
  } catch (cause) {
    if (requestGeneration === generation) error.value = localizeError(cause);
  } finally {
    isLoading.value = false;
  }
}
</script>

<template>
  <!-- 拼进连接表单「认证」那一组：一张分组卡片 + 卡片下的说明 / 报错（结构见 form-list.css） -->
  <div class="form-section">
    <div class="form-list">
      <div class="form-row">
        <span class="form-row-label">{{ t('profile.agentProvider') }}</span>
        <USelect v-model="provider" :items="providerItems" variant="none" class="w-full" />
      </div>
      <label v-if="provider === 'system'" class="form-row">
        <span class="form-row-label">{{ t('profile.agentSocket') }}</span>
        <UInput v-model="socketPath" :placeholder="socketPlaceholder" variant="none" class="w-full" spellcheck="false" />
      </label>
      <div class="form-row">
        <span class="form-row-label">{{ t('profile.agentIdentity') }}</span>
        <USelect v-model="keyFingerprint" :items="identityItems" variant="none" class="w-full min-w-0" />
        <UButton
          color="neutral"
          variant="ghost"
          size="sm"
          square
          icon="i-lucide-refresh-cw"
          :loading="isLoading"
          :aria-label="t('profile.agentRefresh')"
          :title="t('profile.agentRefresh')"
          @click="refreshIdentities"
        />
      </div>
    </div>
    <p v-if="error" class="form-section-footer error">{{ error }}</p>
    <p v-else-if="hasLoaded && !identities.length" class="form-section-footer error">{{ t('profile.agentNoKeys') }}</p>
    <p v-else-if="provider === 'system'" class="form-section-footer">{{ t('profile.agentSocketHint') }}</p>
  </div>
</template>
