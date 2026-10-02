<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@nuxt/ui/composables";
import { clearCommandHistory } from "../../services/commandHistory";
import { useAppStore } from "../../stores/appStore";
import { useSettingsStore, SSH_KEEPALIVE_SECONDS_OPTIONS } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import GlassTintSetting from "./GlassTintSetting.vue";
import WallpaperSetting from "./WallpaperSetting.vue";

const emit = defineEmits(["close"]);
const { t } = useI18n();
const appStore = useAppStore();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();
const toast = useToast();
const isClearingHistory = ref(false);

const locale = computed({
  get: () => settingsStore.locale,
  set: (value) => settingsStore.setLocale(value),
});

const localeItems = [
  { label: "中文", value: "zh-CN" },
  { label: "English", value: "en-US" },
];

const appearanceMode = computed({
  get: () => settingsStore.appearanceMode,
  set: (value) => settingsStore.setAppearanceMode(value),
});

// 三选一且互斥，用 UTabs 的 pill 变体当分段控件（:content="false" 表示只要标签栏）
const appearanceItems = computed(() => [
  { value: "system", label: t("settings.appearanceSystem"), icon: "i-lucide-monitor-smartphone" },
  { value: "light", label: t("settings.appearanceLight"), icon: "i-lucide-sun" },
  { value: "dark", label: t("settings.appearanceDark"), icon: "i-lucide-moon" },
]);

const commandSuggestions = computed({
  get: () => settingsStore.commandSuggestions,
  set: (value) => settingsStore.setCommandSuggestions(value),
});

async function clearHistory() {
  isClearingHistory.value = true;
  try {
    await clearCommandHistory();
    toast.add({ title: t("settings.commandHistoryCleared"), color: "success", icon: "i-lucide-check" });
  } catch (error) {
    uiStore.setError(error);
  } finally {
    isClearingHistory.value = false;
  }
}

const sshKeepaliveSeconds = computed({
  get: () => settingsStore.sshKeepaliveSeconds,
  set: (value) => settingsStore.setSshKeepaliveSeconds(value),
});

const keepaliveItems = computed(() =>
  SSH_KEEPALIVE_SECONDS_OPTIONS.map((seconds) => ({
    label:
      seconds === 0
        ? t("settings.keepaliveOff")
        : t("settings.keepaliveSeconds", { seconds }),
    value: seconds,
  })),
);
</script>

<template>
  <div class="modal-backdrop">
    <section class="settings-panel liquid-sheet">
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
        <div class="settings-row settings-row-stacked">
          <div>
            <strong>{{ t("settings.appearance") }}</strong>
            <small>{{ t("settings.appearanceHint") }}</small>
          </div>
          <UTabs
            v-model="appearanceMode"
            class="settings-appearance"
            variant="pill"
            size="sm"
            :items="appearanceItems"
            :content="false"
          />
        </div>

        <GlassTintSetting />

        <WallpaperSetting />

        <div class="settings-row">
          <div>
            <strong>{{ t("settings.language") }}</strong>
          </div>
          <USelect v-model="locale" class="settings-locale" :items="localeItems" />
        </div>

        <div class="settings-row">
          <div>
            <strong>{{ t("settings.keepalive") }}</strong>
            <small>{{ t("settings.keepaliveHint") }}</small>
          </div>
          <USelect
            v-model="sshKeepaliveSeconds"
            class="settings-locale"
            :items="keepaliveItems"
          />
        </div>

        <div class="settings-row">
          <div>
            <strong>{{ t("settings.commandSuggestions") }}</strong>
            <small>{{ t("settings.commandSuggestionsHint") }}</small>
          </div>
          <div class="settings-actions">
            <UButton
              color="neutral"
              variant="ghost"
              size="sm"
              icon="i-lucide-eraser"
              :label="t('settings.clearCommandHistory')"
              :loading="isClearingHistory"
              @click="clearHistory"
            />
            <USwitch v-model="commandSuggestions" :aria-label="t('settings.commandSuggestions')" />
          </div>
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
