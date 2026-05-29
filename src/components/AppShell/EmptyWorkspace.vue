<script setup>
import { useI18n } from "vue-i18n";

defineProps({
  hasProfiles: {
    type: Boolean,
    required: true,
  },
});

const emit = defineEmits(["createConnection"]);
const { t } = useI18n();
</script>

<template>
  <section class="empty-workspace">
    <div class="empty-icon">
      <UIcon name="i-lucide-folder-kanban" />
    </div>
    <h2>{{ t("workspace.emptyTitle") }}</h2>
    <p>{{ t("workspace.emptyDescription") }}</p>
    <small>{{ t("workspace.noFakeData") }}</small>
    <button v-if="!hasProfiles" type="button" class="primary-action" @click="emit('createConnection')">
      <UIcon name="i-lucide-plus" />
      <span>{{ t("workspace.createConnection") }}</span>
    </button>
  </section>
</template>
