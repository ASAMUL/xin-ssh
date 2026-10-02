<script setup>
import { computed, onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settingsStore";

// 设置 → 玻璃效果：一条「通透 ↔ 着色」滑块。两档开关不够用——壁纸深浅、背景花不花因人而异，
// 合适的那一档往往在中间，iOS 也是这么给的。
const { t } = useI18n();
const settingsStore = useSettingsStore();
const glassTint = computed(() => settingsStore.glassTint);

const PERSIST_DELAY_MS = 250;
let pendingTint = null;
let persistTimer = null;

// 和壁纸滑块一样：先改内存让整个窗口立刻跟着变，停手 250ms 再落盘，免得每一格都写一次 settings.json
function previewTint(value) {
  pendingTint = value;
  settingsStore.previewSettings({ glassTint: value });
  clearTimeout(persistTimer);
  persistTimer = setTimeout(flushPendingWrite, PERSIST_DELAY_MS);
}

function flushPendingWrite() {
  clearTimeout(persistTimer);
  const next = pendingTint;
  pendingTint = null;
  if (next !== null) settingsStore.setGlassTint(next);
}

// 关设置面板时滑块可能还在防抖里，最后那次调整不能丢
onBeforeUnmount(flushPendingWrite);
</script>

<template>
  <section class="form-section">
    <h4 class="form-section-title">{{ t("settings.glassTint") }}</h4>
    <div class="form-list">
      <div class="form-row form-row-block">
        <div class="glass-tint">
          <div class="glass-tint-scale" aria-hidden="true">
            <span>
              <UIcon class="glass-tint-icon" name="i-lucide-droplet" />
              {{ t("settings.glassClear") }}
            </span>
            <em>{{ glassTint }}%</em>
            <span>
              {{ t("settings.glassTinted") }}
              <UIcon class="glass-tint-icon" name="i-lucide-contrast" />
            </span>
          </div>
          <USlider
            :model-value="glassTint"
            :min="0"
            :max="100"
            size="sm"
            :aria-label="t('settings.glassTint')"
            @update:model-value="previewTint"
          />
        </div>
      </div>
    </div>
    <p class="form-section-footer">{{ t("settings.glassTintHint") }}</p>
  </section>
</template>

<style scoped src="../../assets/css/settings-glass.css"></style>
