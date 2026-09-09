<script setup>
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import ConnectionCard from "./ConnectionCard.vue";
import { useHomeSortable } from "../../composables/useHomeSortable";

const props = defineProps({
  group: { type: Object, required: true },
  name: { type: String, required: true },
  profiles: { type: Array, required: true },
  statuses: { type: Object, required: true },
  menuItems: { type: Object, required: true },
  disabled: { type: Boolean, default: false },
  forceExpanded: { type: Boolean, default: false },
});
const emit = defineEmits(["open", "edit", "toggleFavorite", "collapse", "move", "dragStart", "dragEnd"]);
const { t } = useI18n();
const list = ref(null);
const expanded = computed(() => props.forceExpanded || !props.group.collapsed);

useHomeSortable(list, {
  group: "home-connections",
  draggable: ".connection-card",
  handle: ".connection-card-drag-handle",
  disabled: () => props.disabled,
  onStart: () => emit("dragStart"),
  onEnd: () => emit("dragEnd"),
  onDrop: (move) => emit("move", move),
});

function setExpanded(open) {
  if (!props.forceExpanded) emit("collapse", props.group.id, !open);
}
</script>

<template>
  <UCollapsible
    class="home-connection-group"
    :open="expanded"
    :disabled="disabled || forceExpanded"
    :unmount-on-hide="false"
    @update:open="setExpanded"
  >
    <UButton
      class="home-group-toggle"
      color="neutral"
      variant="ghost"
      :icon="expanded ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
      :aria-label="t(expanded ? 'sidebar.collapseGroup' : 'sidebar.expandGroup', { name })"
    >
      <span>{{ name }}</span>
      <small>{{ profiles.length }}</small>
    </UButton>
    <template #content>
      <div ref="list" class="home-group-grid" :data-group-id="group.id" :aria-busy="disabled">
        <ConnectionCard
          v-for="profile in profiles"
          :key="profile.id"
          :data-sort-id="profile.id"
          :profile="profile"
          :status="statuses[profile.id] ?? 'notOpen'"
          :menu-items="menuItems[profile.id]"
          :sortable="!disabled"
          @open="emit('open', $event)"
          @edit="emit('edit', $event)"
          @toggle-favorite="emit('toggleFavorite', $event)"
        />
      </div>
      <p v-if="!profiles.length" class="home-group-empty">{{ t("sidebar.empty") }}</p>
    </template>
  </UCollapsible>
</template>

<style scoped src="../../assets/css/home-connection-group.css"></style>
