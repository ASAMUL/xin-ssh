<script setup>
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useBackgroundUrl } from "../../composables/useBackgroundUrl";
import { importTerminalBackground, pickBackgroundImage } from "../../services/backgroundApi";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";

// 设置 → 应用壁纸。图片和终端背景图走同一条链路：选图 → 复制进 <workspace>/backgrounds/ → settings.json 只记文件名。
const { t } = useI18n();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();
const wallpaper = computed(() => settingsStore.appWallpaper);
const previewUrl = useBackgroundUrl(() => wallpaper.value?.file ?? null);
const isImporting = ref(false);

// 新图默认完全可见、不模糊；换图时沿用已经调好的可见度和模糊
const DEFAULT_WALLPAPER = { opacity: 1, blur: 0, fit: "cover" };
const PERSIST_DELAY_MS = 250;
let pendingWallpaper = null;
let persistTimer = null;

async function chooseWallpaper() {
  flushPendingWrite();
  try {
    const sourcePath = await pickBackgroundImage(t("settings.wallpaperPick"));
    if (!sourcePath) return;

    isImporting.value = true;
    const file = await importTerminalBackground(sourcePath);
    await settingsStore.setAppWallpaper({ ...DEFAULT_WALLPAPER, ...wallpaper.value, file });
  } catch (error) {
    uiStore.setError(error);
  } finally {
    isImporting.value = false;
  }
}

// 拖滑块时先改内存让窗口立刻跟着变，停手 250ms 再落盘，免得每一格都写一次 settings.json
function patchWallpaper(partial) {
  if (!wallpaper.value) return;

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

function removeWallpaper() {
  clearTimeout(persistTimer);
  pendingWallpaper = null;
  settingsStore.setAppWallpaper(null).catch((error) => uiStore.setError(error));
}

// 关设置面板时滑块可能还在防抖里，最后那次调整不能丢
onBeforeUnmount(flushPendingWrite);
</script>

<template>
  <section class="form-section">
    <h4 class="form-section-title">{{ t("settings.wallpaper") }}</h4>
    <div class="form-list">
      <div class="form-row form-row-block">
        <div class="wallpaper-setting">
          <div
            class="wallpaper-preview"
            :class="{ empty: !previewUrl }"
            :style="previewUrl ? { backgroundImage: `url('${previewUrl}')` } : null"
          >
            <UIcon v-if="!previewUrl" name="i-lucide-image" />
          </div>

          <div class="wallpaper-actions">
            <UButton
              color="neutral"
              variant="soft"
              size="sm"
              icon="i-lucide-image-plus"
              :loading="isImporting"
              :label="wallpaper ? t('settings.wallpaperReplace') : t('settings.wallpaperChoose')"
              @click="chooseWallpaper"
            />
            <UButton
              v-if="wallpaper"
              color="neutral"
              variant="ghost"
              size="sm"
              icon="i-lucide-trash-2"
              :label="t('settings.wallpaperRemove')"
              @click="removeWallpaper"
            />
          </div>
        </div>
      </div>

      <template v-if="wallpaper">
        <div class="form-row form-row-block">
          <div class="wallpaper-slider">
            <span>
              {{ t("settings.wallpaperOpacity") }}
              <em>{{ Math.round(wallpaper.opacity * 100) }}%</em>
            </span>
            <USlider
              :model-value="Math.round(wallpaper.opacity * 100)"
              :min="10"
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
              :max="30"
              size="sm"
              :aria-label="t('settings.wallpaperBlur')"
              @update:model-value="patchWallpaper({ blur: $event })"
            />
          </div>
        </div>
      </template>
    </div>
    <p class="form-section-footer">{{ t("settings.wallpaperHint") }}</p>
  </section>
</template>

<style scoped src="../../assets/css/settings-wallpaper.css"></style>
