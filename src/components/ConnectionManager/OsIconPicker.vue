<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import OsIconTile from "./OsIconTile.vue";
import { OS_ICON_AUTO, OS_ICON_GROUPS, getOsIcon } from "../../config/osIcons";

const props = defineProps({
  // 编辑已有连接时传入：自动模式下展示「已识别为 xx」，让用户知道自动的结果是什么
  detectedOs: { type: String, default: null },
});
const model = defineModel({ type: String, default: OS_ICON_AUTO });
const { t } = useI18n();
const open = ref(false);

// 自动且尚未识别时没有具体系统可画，用一枚魔法棒表达「交给程序判断」
const AUTO_PLACEHOLDER = { icon: "i-lucide-wand-sparkles", mono: true };

const isAuto = computed(() => !getOsIcon(model.value));
const detectedIcon = computed(() => getOsIcon(props.detectedOs));
const previewIcon = computed(() =>
  (isAuto.value ? detectedIcon.value : getOsIcon(model.value)) ?? AUTO_PLACEHOLDER,
);
const triggerLabel = computed(() => {
  if (!isAuto.value) return iconLabel(getOsIcon(model.value));
  return detectedIcon.value
    ? t("osIcon.autoDetected", { name: iconLabel(detectedIcon.value) })
    : t("osIcon.auto");
});

function iconLabel(item) {
  return item.labelKey ? t(item.labelKey) : item.label;
}

function select(id) {
  model.value = id;
  open.value = false;
}
</script>

<template>
  <UPopover v-model:open="open" :content="{ align: 'start', side: 'bottom', sideOffset: 8, collisionPadding: 16 }">
    <UButton
      class="os-icon-trigger"
      :class="{ open }"
      color="neutral"
      variant="outline"
      size="lg"
      block
      :aria-label="t('osIcon.choose')"
      :title="t('osIcon.choose')"
    >
      <OsIconTile :icon="previewIcon" :size="26" />
      <span class="os-icon-trigger-label">{{ triggerLabel }}</span>
      <UIcon name="i-lucide-chevrons-up-down" class="os-icon-trigger-chevron" />
    </UButton>

    <template #content>
      <div class="os-icon-picker" role="listbox" :aria-label="t('osIcon.choose')">
        <UButton
          class="os-icon-auto"
          :class="{ active: isAuto }"
          color="neutral"
          variant="ghost"
          block
          role="option"
          :aria-selected="isAuto"
          @click="select(OS_ICON_AUTO)"
        >
          <span class="os-icon-auto-mark">
            <UIcon name="i-lucide-wand-sparkles" />
          </span>
          <span class="os-icon-auto-copy">
            <strong>{{ t("osIcon.auto") }}</strong>
            <small>
              {{ detectedIcon ? t("osIcon.detectedAs", { name: iconLabel(detectedIcon) }) : t("osIcon.autoHint") }}
            </small>
          </span>
          <UIcon v-if="isAuto" name="i-lucide-check" class="os-icon-auto-check" />
        </UButton>

        <section v-for="group in OS_ICON_GROUPS" :key="group.id" class="os-icon-section">
          <h4>{{ t(`osIcon.groups.${group.id}`) }}</h4>
          <div class="os-icon-grid">
            <UButton
              v-for="item in group.items"
              :key="item.id"
              class="os-icon-option"
              :class="{ active: model === item.id }"
              color="neutral"
              variant="ghost"
              role="option"
              :aria-selected="model === item.id"
              :title="iconLabel(item)"
              @click="select(item.id)"
            >
              <OsIconTile :icon="item" :size="34" />
              <span>{{ iconLabel(item) }}</span>
            </UButton>
          </div>
        </section>
      </div>
    </template>
  </UPopover>
</template>

<style scoped src="../../assets/css/os-icon-picker.css"></style>
