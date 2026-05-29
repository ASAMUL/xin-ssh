<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useVaultStore } from "../../stores/vaultStore";

const emit = defineEmits(["close"]);
const { t } = useI18n();
const appStore = useAppStore();
const settingsStore = useSettingsStore();
const vaultStore = useVaultStore();

const locale = computed({
  get: () => settingsStore.locale,
  set: (value) => settingsStore.setLocale(value),
});
</script>

<template>
  <div class="modal-backdrop">
    <section class="settings-panel">
      <header>
        <h2>{{ t("settings.title") }}</h2>
        <button type="button" :aria-label="t('settings.close')" @click="emit('close')">
          <UIcon name="i-lucide-x" />
        </button>
      </header>

      <div class="settings-row">
        <div>
          <strong>{{ t("settings.language") }}</strong>
        </div>
        <select v-model="locale">
          <option value="zh-CN">中文</option>
          <option value="en-US">English</option>
        </select>
      </div>

      <div class="settings-row">
        <div>
          <strong>{{ t("settings.workspaceDir") }}</strong>
          <small>{{ appStore.workspaceDir }}</small>
        </div>
      </div>

      <div class="settings-row">
        <div>
          <strong>{{ t("settings.vault") }}</strong>
          <small>{{ vaultStore.isUnlocked ? t("settings.vaultUnlocked") : t("settings.vaultLocked") }}</small>
        </div>
        <button type="button" class="secondary-action" :disabled="!vaultStore.isUnlocked" @click="vaultStore.lock">
          {{ t("settings.lockVault") }}
        </button>
      </div>
    </section>
  </div>
</template>
