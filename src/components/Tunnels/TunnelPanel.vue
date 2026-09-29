<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useProfileStore } from "../../stores/profileStore";
import { useTunnelStore } from "../../stores/tunnelStore";
import { useUiStore } from "../../stores/uiStore";
import { localizeError } from "../../utils/appError";

const emit = defineEmits(["close"]);
const { t } = useI18n();
const profileStore = useProfileStore();
const tunnelStore = useTunnelStore();
const uiStore = useUiStore();

const mode = ref("list");
const editingTunnelId = ref(null);
const formError = ref("");
const isSaving = ref(false);
// 待启动但需要补输凭据的隧道 id + 输入值
const pendingStartId = ref(null);
const pendingCredentials = reactive({ password: "", passphrase: "" });
// 两段式删除：第一次点击进入待确认态，3 秒内再点才删
const armedDeleteId = ref(null);
let armedDeleteTimer = null;

const form = reactive({
  name: "",
  profileId: null,
  localPort: null,
  remoteHost: "127.0.0.1",
  remotePort: null,
});

const profileOptions = computed(() =>
  profileStore.activeProfiles.map((profile) => ({
    label: `${profile.name} (${profile.username}@${profile.host})`,
    value: profile.id,
  })),
);

const canSave = computed(
  () =>
    form.profileId &&
    String(form.remoteHost ?? "").trim() &&
    isValidPort(form.localPort) &&
    isValidPort(form.remotePort),
);

onMounted(() => {
  tunnelStore.loadConfigs();
});

onBeforeUnmount(() => {
  clearTimeout(armedDeleteTimer);
});

function isValidPort(port) {
  const value = Number(port);
  return Number.isInteger(value) && value >= 1 && value <= 65535;
}

function profileOf(tunnel) {
  return profileStore.activeProfiles.find((profile) => profile.id === tunnel.profileId) ?? null;
}

function tunnelDisplayName(tunnel) {
  return tunnel.name?.trim() || `${tunnel.localPort} → ${tunnel.remotePort}`;
}

function forwardSummary(tunnel) {
  return `localhost:${tunnel.localPort} → ${tunnel.remoteHost}:${tunnel.remotePort}`;
}

function statusOf(tunnel) {
  return tunnelStore.runtimeOf(tunnel.id);
}

function openCreateForm() {
  editingTunnelId.value = null;
  form.name = "";
  form.profileId = profileStore.activeProfiles[0]?.id ?? null;
  form.localPort = null;
  form.remoteHost = "127.0.0.1";
  form.remotePort = null;
  formError.value = "";
  mode.value = "form";
}

function openEditForm(tunnel) {
  if (!tunnelStore.canEditConfig(tunnel.id)) return;
  editingTunnelId.value = tunnel.id;
  form.name = tunnel.name ?? "";
  form.profileId = tunnel.profileId;
  form.localPort = tunnel.localPort;
  form.remoteHost = tunnel.remoteHost;
  form.remotePort = tunnel.remotePort;
  formError.value = "";
  mode.value = "form";
}

async function submitForm() {
  if (isSaving.value) return;
  if (!canSave.value) {
    formError.value = t("tunnel.requiredFields");
    return;
  }

  isSaving.value = true;
  formError.value = "";
  try {
    const now = Date.now().toString();
    const existing = tunnelStore.configs.find((item) => item.id === editingTunnelId.value);
    const tunnelId = existing?.id ?? crypto.randomUUID();

    await tunnelStore.saveConfig({
      id: tunnelId,
      syncId: existing?.syncId ?? tunnelId,
      profileId: form.profileId,
      name: form.name.trim(),
      localPort: Number(form.localPort),
      remoteHost: form.remoteHost.trim() || "127.0.0.1",
      remotePort: Number(form.remotePort),
      sortOrder: existing?.sortOrder ?? tunnelStore.activeConfigs.length,
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
      deletedAt: existing?.deletedAt ?? null,
    });
    mode.value = "list";
  } catch (error) {
    formError.value = localizeError(error);
  } finally {
    isSaving.value = false;
  }
}

function requestStart(tunnel) {
  if (!tunnelStore.canEditConfig(tunnel.id)) return;
  const profile = profileOf(tunnel);
  if (!profile) return;

  // 未保存凭据时先内联收集：密码认证必填密码，私钥认证可留空 passphrase
  if (["password", "privateKey"].includes(profile.authMethod) && !profile.credentialRef) {
    pendingStartId.value = tunnel.id;
    pendingCredentials.password = "";
    pendingCredentials.passphrase = "";
    return;
  }

  tunnelStore.start(tunnel);
}

function confirmPendingStart(tunnel) {
  const profile = profileOf(tunnel);
  if (profile?.authMethod === "password" && !pendingCredentials.password) return;

  tunnelStore.start(tunnel, {
    password: pendingCredentials.password,
    passphrase: pendingCredentials.passphrase,
  });
  cancelPendingStart();
}

function cancelPendingStart() {
  pendingStartId.value = null;
  pendingCredentials.password = "";
  pendingCredentials.passphrase = "";
}

function requestDelete(tunnel) {
  if (tunnelStore.busyConfigs[tunnel.id]) return;
  if (armedDeleteId.value === tunnel.id) {
    clearTimeout(armedDeleteTimer);
    armedDeleteId.value = null;
    if (pendingStartId.value === tunnel.id) cancelPendingStart();
    tunnelStore.removeConfig(tunnel.id).catch((error) => uiStore.setError(error));
    return;
  }

  armedDeleteId.value = tunnel.id;
  clearTimeout(armedDeleteTimer);
  armedDeleteTimer = setTimeout(() => {
    armedDeleteId.value = null;
  }, 3000);
}

function stopTunnel(tunnel) {
  tunnelStore.stop(tunnel.id).catch((error) => uiStore.setError(error));
}
</script>

<template>
  <div class="modal-backdrop">
    <section class="settings-panel tunnel-panel liquid-sheet">
      <header>
        <h2>{{ mode === "form" ? t(editingTunnelId ? "tunnel.edit" : "tunnel.add") : t("tunnel.title") }}</h2>
        <UButton
          color="neutral"
          variant="ghost"
          square
          icon="i-lucide-x"
          :aria-label="t('settings.close')"
          @click="emit('close')"
        />
      </header>

      <div v-if="mode === 'list'" class="form-body tunnel-list">
        <p class="tunnel-intro">{{ t("tunnel.intro") }}</p>

        <div v-if="tunnelStore.activeConfigs.length === 0" class="tunnel-empty">
          <UIcon name="i-lucide-waypoints" />
          <span>{{ t("tunnel.empty") }}</span>
        </div>

        <article
          v-for="tunnel in tunnelStore.activeConfigs"
          :key="tunnel.id"
          class="tunnel-item"
          :class="statusOf(tunnel).status"
        >
          <div class="tunnel-item-head">
            <span class="tunnel-item-icon">
              <UIcon name="i-lucide-waypoints" />
            </span>

            <div class="tunnel-item-copy">
              <strong>{{ tunnelDisplayName(tunnel) }}</strong>
              <small>{{ profileOf(tunnel)?.name ?? "—" }} · {{ forwardSummary(tunnel) }}</small>
            </div>

            <span class="tunnel-item-status">
              <i></i>
              <span>{{ t(`tunnel.status.${statusOf(tunnel).status}`) }}</span>
            </span>

            <div class="tunnel-item-actions">
              <UButton
                v-if="['stopped', 'error'].includes(statusOf(tunnel).status)"
                color="primary"
                variant="soft"
                size="xs"
                icon="i-lucide-play"
                :aria-label="t('tunnel.start')"
                :title="t('tunnel.start')"
                :disabled="!profileOf(tunnel) || !tunnelStore.canEditConfig(tunnel.id)"
                @click="requestStart(tunnel)"
              />
              <UButton
                v-else
                color="neutral"
                variant="soft"
                size="xs"
                icon="i-lucide-square"
                :loading="statusOf(tunnel).status === 'stopping'"
                :aria-label="t('tunnel.stop')"
                :title="t('tunnel.stop')"
                @click="stopTunnel(tunnel)"
              />
              <UButton
                color="neutral"
                variant="ghost"
                size="xs"
                icon="i-lucide-pencil"
                :aria-label="t('tunnel.edit')"
                :title="tunnelStore.canEditConfig(tunnel.id) ? t('tunnel.edit') : t('tunnel.stopBeforeEdit')"
                :disabled="!tunnelStore.canEditConfig(tunnel.id)"
                @click="openEditForm(tunnel)"
              />
              <UButton
                :color="armedDeleteId === tunnel.id ? 'error' : 'neutral'"
                :variant="armedDeleteId === tunnel.id ? 'soft' : 'ghost'"
                size="xs"
                icon="i-lucide-trash-2"
                :disabled="Boolean(tunnelStore.busyConfigs[tunnel.id])"
                :aria-label="t('tunnel.delete')"
                :title="armedDeleteId === tunnel.id ? t('tunnel.deleteArmed') : t('tunnel.delete')"
                @click="requestDelete(tunnel)"
              />
            </div>
          </div>

          <p v-if="statusOf(tunnel).error" class="tunnel-item-error">
            {{ statusOf(tunnel).error }}
          </p>

          <div v-if="pendingStartId === tunnel.id" class="tunnel-credentials">
            <span>{{ t("tunnel.credentialsNeeded") }}</span>
            <div class="tunnel-credentials-row">
              <UInput
                v-if="profileOf(tunnel)?.authMethod === 'password'"
                v-model="pendingCredentials.password"
                type="password"
                size="sm"
                :placeholder="t('profile.password')"
                @keyup.enter="confirmPendingStart(tunnel)"
              />
              <UInput
                v-else
                v-model="pendingCredentials.passphrase"
                type="password"
                size="sm"
                :placeholder="t('tunnel.passphraseOptional')"
                @keyup.enter="confirmPendingStart(tunnel)"
              />
              <UButton
                color="primary"
                variant="solid"
                size="sm"
                :label="t('tunnel.start')"
                :disabled="profileOf(tunnel)?.authMethod === 'password' && !pendingCredentials.password"
                @click="confirmPendingStart(tunnel)"
              />
              <UButton
                color="neutral"
                variant="outline"
                size="sm"
                :label="t('common.cancel')"
                @click="cancelPendingStart"
              />
            </div>
          </div>
        </article>
      </div>

      <div v-else class="form-body">
        <UFormField :label="t('tunnel.profile')" required>
          <USelect
            v-model="form.profileId"
            class="tunnel-field"
            size="lg"
            :items="profileOptions"
            :placeholder="t('tunnel.profile')"
          />
        </UFormField>

        <UFormField :label="t('tunnel.name')">
          <UInput
            v-model="form.name"
            class="tunnel-field"
            size="lg"
            :placeholder="t('tunnel.namePlaceholder')"
          />
        </UFormField>

        <div class="form-grid">
          <UFormField :label="t('tunnel.localPort')" required>
            <UInputNumber
              v-model="form.localPort"
              class="tunnel-field"
              size="lg"
              :min="1"
              :max="65535"
              :step="1"
              placeholder="18080"
            />
          </UFormField>
          <UFormField :label="t('tunnel.remotePort')" required>
            <UInputNumber
              v-model="form.remotePort"
              class="tunnel-field"
              size="lg"
              :min="1"
              :max="65535"
              :step="1"
              placeholder="8080"
            />
          </UFormField>
        </div>

        <UFormField :label="t('tunnel.remoteHost')" :hint="t('tunnel.remoteHostHint')" required>
          <UInput
            v-model="form.remoteHost"
            class="tunnel-field"
            size="lg"
            placeholder="127.0.0.1"
          />
        </UFormField>

        <p v-if="formError" class="form-error">{{ formError }}</p>
      </div>

      <footer class="tunnel-footer">
        <template v-if="mode === 'list'">
          <UButton
            color="primary"
            variant="solid"
            size="lg"
            class="cta-button"
            icon="i-lucide-plus"
            :label="t('tunnel.add')"
            :disabled="profileOptions.length === 0"
            @click="openCreateForm"
          />
        </template>
        <template v-else>
          <UButton
            color="neutral"
            variant="outline"
            size="lg"
            :label="t('common.cancel')"
            @click="mode = 'list'"
          />
          <UButton
            color="primary"
            variant="solid"
            size="lg"
            class="cta-button"
            :label="t('tunnel.save')"
            :loading="isSaving"
            :disabled="!canSave"
            @click="submitForm"
          />
        </template>
      </footer>
    </section>
  </div>
</template>

<style scoped src="../../assets/css/tunnel-panel.css"></style>
