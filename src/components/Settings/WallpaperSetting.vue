<script setup>
import { computed, onBeforeUnmount, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useBackgroundUrl } from "../../composables/useBackgroundUrl";
import {
  BUILT_IN_WALLPAPERS,
  MAX_WALLPAPER_BLUR,
  MIN_WALLPAPER_OPACITY,
  WALLPAPER_MODES,
} from "../../config/appWallpaper";
import { importTerminalBackground, pickBackgroundImage } from "../../services/backgroundApi";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { resolvedAppearance } from "../../utils/appearanceMode";

// 设置 → 应用壁纸。浅色 / 深色各一行，各自能换图、恢复默认，但没有「移除」：玻璃底下不能是空的。
// 自定义图和终端背景图走同一条链路：选图 → 复制进 <workspace>/backgrounds/ → settings.json 只记文件名。
const { t } = useI18n();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();
const wallpaper = computed(() => settingsStore.appWallpaper);
const activeMode = resolvedAppearance;
const importingMode = ref(null);

const MODE_LABEL_KEYS = { light: "settings.wallpaperLight", dark: "settings.wallpaperDark" };
// 0.1 * 100 在浮点里是 10.000000000000002，滑块的下限得是整数
const MIN_OPACITY_PERCENT = Math.round(MIN_WALLPAPER_OPACITY * 100);
// reactive 包一层，模板里拿到的就是解包后的地址
const previewUrls = reactive({
  light: useBackgroundUrl(() => wallpaper.value.light, () => BUILT_IN_WALLPAPERS.light),
  dark: useBackgroundUrl(() => wallpaper.value.dark, () => BUILT_IN_WALLPAPERS.dark),
});

const PERSIST_DELAY_MS = 250;
let pendingWallpaper = null;
let persistTimer = null;

async function chooseWallpaper(mode) {
  try {
    const sourcePath = await pickBackgroundImage(t("settings.wallpaperPick"));
    if (!sourcePath) return;

    importingMode.value = mode;
    const file = await importTerminalBackground(sourcePath);
    await commitWallpaper({ ...wallpaper.value, [mode]: file });
  } catch (error) {
    uiStore.setError(error);
  } finally {
    importingMode.value = null;
  }
}

function resetWallpaper(mode) {
  commitWallpaper({ ...wallpaper.value, [mode]: null }).catch((error) => uiStore.setError(error));
}

// 内存里的壁纸已经带着滑块的预览值，整份写下去就顺带落了盘，还在防抖的那次可以丢掉
function commitWallpaper(next) {
  clearTimeout(persistTimer);
  pendingWallpaper = null;
  return settingsStore.setAppWallpaper(next);
}

// 拖滑块时先改内存让窗口立刻跟着变，停手 250ms 再落盘，免得每一格都写一次 settings.json
function patchWallpaper(partial) {
  pendingWallpaper = { ...wallpaper.value, ...partial };
  settingsStore.previewSettings({ appWallpaper: pendingWallpaper });
  clearTimeout(persistTimer);
  persistTimer = setTimeout(flushPendingWrite, PERSIST_DELAY_MS);
}

function flushPendingWrite() {
  clearTimeout(persistTimer);
  const next = pendingWallpaper;
  pendingWallpaper = null;
  if (next) settingsStore.setAppWallpaper(next).catch((error) => uiStore.setError(error));
}

// 关设置面板时滑块可能还在防抖里，最后那次调整不能丢
onBeforeUnmount(flushPendingWrite);
</script>

<template>
  <section class="form-section">
    <h4 class="form-section-title">{{ t("settings.wallpaper") }}</h4>
    <div class="form-list">
      <div v-for="mode in WALLPAPER_MODES" :key="mode" class="form-row form-row-block">
        <div class="wallpaper-setting">
          <div
            class="wallpaper-preview"
            :style="previewUrls[mode] ? { backgroundImage: `url('${previewUrls[mode]}')` } : null"
          ></div>

          <div class="wallpaper-meta">
            <span class="wallpaper-mode">
              {{ t(MODE_LABEL_KEYS[mode]) }}
              <UBadge
                v-if="activeMode === mode"
                color="primary"
                variant="soft"
                size="sm"
                :label="t('settings.wallpaperInUse')"
              />
            </span>
            <small>{{ wallpaper[mode] ? t("settings.wallpaperCustom") : t("settings.wallpaperBuiltIn") }}</small>
          </div>

          <div class="wallpaper-actions">
            <UButton
              color="neutral"
              variant="soft"
              size="sm"
              icon="i-lucide-image-plus"
              :loading="importingMode === mode"
              :label="t('settings.wallpaperReplace')"
              @click="chooseWallpaper(mode)"
            />
            <UButton
              v-if="wallpaper[mode]"
              color="neutral"
              variant="ghost"
              size="sm"
              icon="i-lucide-rotate-ccw"
              :label="t('settings.wallpaperReset')"
              @click="resetWallpaper(mode)"
            />
          </div>
        </div>
      </div>

      <div class="form-row form-row-block">
        <div class="wallpaper-slider">
          <span>
            {{ t("settings.wallpaperOpacity") }}
            <em>{{ Math.round(wallpaper.opacity * 100) }}%</em>
          </span>
          <USlider
            :model-value="Math.round(wallpaper.opacity * 100)"
            :min="MIN_OPACITY_PERCENT"
            :max="100"
            size="sm"
            :aria-label="t('settings.wallpaperOpacity')"
            @update:model-value="patchWallpaper({ opacity: $event / 100 })"
          />
        </div>
      </div>

      <div class="form-row form-row-block">
        <div class="wallpaper-slider">
          <span>
            {{ t("settings.wallpaperBlur") }}
            <em>{{ wallpaper.blur }}px</em>
          </span>
          <USlider
            :model-value="wallpaper.blur"
            :min="0"
            :max="MAX_WALLPAPER_BLUR"
            size="sm"
            :aria-label="t('settings.wallpaperBlur')"
            @update:model-value="patchWallpaper({ blur: $event })"
          />
        </div>
      </div>
    </div>
    <p class="form-section-footer">{{ t("settings.wallpaperHint") }}</p>
  </section>
</template>

<style scoped src="../../assets/css/settings-wallpaper.css"></style>
