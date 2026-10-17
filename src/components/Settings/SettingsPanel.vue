<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useToast } from "@nuxt/ui/composables";
import { openPath } from "@tauri-apps/plugin-opener";
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

// 左边分类、右边是这一类的设置（macOS 系统设置的结构）。设置项会越加越多，
// 一整页从上滚到下很快就找不到东西；分好类以后加新项只是往某一页里多放一张卡片
const activeSection = ref("appearance");
const sectionItems = computed(() => [
  { value: "appearance", label: t("settings.appearance"), icon: "i-lucide-palette", slot: "appearance" },
  { value: "terminal", label: t("settings.terminal"), icon: "i-lucide-square-terminal", slot: "terminal" },
  { value: "general", label: t("settings.general"), icon: "i-lucide-settings-2", slot: "general" },
]);

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

// 只把路径写在那儿，想去目录里看一眼还得自己复制粘贴；给个按钮直接在文件管理器里打开
async function openWorkspaceDir() {
  try {
    await openPath(appStore.workspaceDir);
  } catch (error) {
    uiStore.setError(error);
  }
}
</script>

<template>
  <div class="modal-backdrop">
    <section class="settings-window liquid-sheet" :aria-label="t('settings.title')">
      <UTabs
        v-model="activeSection"
        class="nav-window-tabs"
        orientation="vertical"
        variant="pill"
        :items="sectionItems"
        :ui="{
          root: 'items-stretch gap-0',
          list: 'nav-window-nav',
          trigger: 'nav-window-item',
          indicator: 'nav-window-indicator',
          content: 'nav-window-pane',
        }"
      >
        <template #list-leading>
          <h2 class="nav-window-title">{{ t("settings.title") }}</h2>
        </template>

        <template #appearance>
          <h3 class="nav-window-pane-title">{{ t("settings.appearance") }}</h3>
          <section class="form-section">
            <div class="form-list">
              <div class="form-row form-row-split">
                <span class="form-row-label">{{ t("settings.colorScheme") }}</span>
                <UTabs
                  v-model="appearanceMode"
                  v-liquid-stretch="appearanceMode"
                  class="appearance-switch"
                  variant="pill"
                  size="sm"
                  :items="appearanceItems"
                  :content="false"
                />
              </div>
            </div>
            <p class="form-section-footer">{{ t("settings.appearanceHint") }}</p>
          </section>
          <GlassTintSetting />
          <WallpaperSetting />
        </template>

        <template #terminal>
          <h3 class="nav-window-pane-title">{{ t("settings.terminal") }}</h3>
          <section class="form-section">
            <div class="form-list">
              <label class="form-row form-row-split">
                <span class="form-row-label">{{ t("settings.commandSuggestions") }}</span>
                <USwitch v-model="commandSuggestions" />
              </label>
              <div class="form-row form-row-split">
                <span class="form-row-label">{{ t("settings.commandHistory") }}</span>
                <UButton
                  color="neutral"
                  variant="soft"
                  size="sm"
                  icon="i-lucide-eraser"
                  :label="t('settings.clearCommandHistory')"
                  :loading="isClearingHistory"
                  @click="clearHistory"
                />
              </div>
            </div>
            <p class="form-section-footer">{{ t("settings.commandSuggestionsHint") }}</p>
          </section>
          <section class="form-section">
            <h4 class="form-section-title">{{ t("settings.connection") }}</h4>
            <div class="form-list">
              <div class="form-row form-row-split">
                <span class="form-row-label">{{ t("settings.keepalive") }}</span>
                <USelect
                  v-model="sshKeepaliveSeconds"
                  class="settings-select"
                  variant="soft"
                  size="sm"
                  :items="keepaliveItems"
                />
              </div>
            </div>
            <p class="form-section-footer">{{ t("settings.keepaliveHint") }}</p>
          </section>
        </template>

        <template #general>
          <h3 class="nav-window-pane-title">{{ t("settings.general") }}</h3>
          <section class="form-section">
            <div class="form-list">
              <div class="form-row form-row-split">
                <span class="form-row-label">{{ t("settings.language") }}</span>
                <USelect v-model="locale" class="settings-select" variant="soft" size="sm" :items="localeItems" />
              </div>
            </div>
          </section>
          <section class="form-section">
            <h4 class="form-section-title">{{ t("settings.data") }}</h4>
            <div class="form-list">
              <div class="form-row form-row-split">
                <span class="form-row-label">{{ t("settings.workspaceDir") }}</span>
                <UButton
                  color="neutral"
                  variant="soft"
                  size="sm"
                  icon="i-lucide-folder-open"
                  :label="t('settings.workspaceDirOpen')"
                  :title="t('settings.workspaceDirOpenTitle')"
                  :disabled="!appStore.workspaceDir"
                  @click="openWorkspaceDir"
                />
              </div>
            </div>
            <p class="form-section-footer workspace-path">{{ appStore.workspaceDir }}</p>
          </section>
        </template>
      </UTabs>

      <UButton
        class="nav-window-close"
        color="neutral"
        variant="ghost"
        square
        icon="i-lucide-x"
        :aria-label="t('settings.close')"
        @click="emit('close')"
      />
    </section>
  </div>
</template>

<style scoped src="../../assets/css/settings-window.css"></style>
