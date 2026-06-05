<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore } from "../../stores/settingsStore";

const emit = defineEmits(["close"]);
const { t } = useI18n();
const appStore = useAppStore();
const settingsStore = useSettingsStore();

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
    </section>
  </div>
</template>
