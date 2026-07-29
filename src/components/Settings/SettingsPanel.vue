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

const localeItems = [
  { label: "中文", value: "zh-CN" },
  { label: "English", value: "en-US" },
];
</script>

<template>
  <div class="modal-backdrop">
    <section class="settings-panel">
      <header>
        <h2>{{ t("settings.title") }}</h2>
        <UButton
          color="neutral"
          variant="ghost"
          square
          icon="i-lucide-x"
          :aria-label="t('settings.close')"
          @click="emit('close')"
        />
      </header>

      <div class="settings-body">
        <div class="settings-row">
          <div>
            <strong>{{ t("settings.language") }}</strong>
          </div>
          <USelect v-model="locale" class="settings-locale" :items="localeItems" />
        </div>

        <div class="settings-row">
          <div>
            <strong>{{ t("settings.workspaceDir") }}</strong>
            <small>{{ appStore.workspaceDir }}</small>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
