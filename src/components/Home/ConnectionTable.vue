<script setup>
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import OsIconTile from "../ConnectionManager/OsIconTile.vue";
import { resolveProfileOsIcon } from "../../config/osIcons";
import { formatRelativeTime } from "../../utils/relativeTime";
import { useHomeSortable } from "../../composables/useHomeSortable";

const props = defineProps({
  // 行结构：{ profile, groupName, status, menuItems }；默认分组的 groupName 为 null
  rows: {
    type: Array,
    default: () => [],
  },
  sortable: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits(["open", "edit", "toggleFavorite", "reorder"]);
const { t } = useI18n();
const tableBody = ref(null);

// 整行都能拖，但星标、操作按钮要留给点击；行名按钮本身就是「点一下连接」，照样可以按住拖
useHomeSortable(tableBody, {
  draggable: "tr",
  filter: "button:not(.row-name)",
  preventOnFilter: false,
  disabled: () => !props.sortable,
  onDrop: ({ id, order }) => emit("reorder", { id, order }),
});

function statusLabel(status) {
  return status === "notOpen" ? t("workspace.notOpen") : t(`monitor.status.${status}`);
}
</script>

<template>
  <div class="connection-table">
    <table>
      <thead>
        <tr>
          <th class="col-name">{{ t("home.table.name") }}</th>
          <th>{{ t("home.table.host") }}</th>
          <th>{{ t("home.table.username") }}</th>
          <th>{{ t("home.table.group") }}</th>
          <th>{{ t("home.table.status") }}</th>
          <th>{{ t("home.table.lastConnected") }}</th>
          <th class="col-actions">{{ t("home.table.actions") }}</th>
        </tr>
      </thead>
      <tbody ref="tableBody" :class="{ sortable }">
        <UContextMenu
          v-for="{ profile, groupName, status, menuItems } in rows"
          :key="profile.id"
          :items="menuItems"
          size="xs"
        >
          <tr :class="status" :data-sort-id="profile.id">
            <td class="col-name">
              <UButton
                class="row-star"
                :class="{ favorite: profile.favorite }"
                color="neutral"
                variant="ghost"
                size="xs"
                icon="i-lucide-star"
                :aria-label="profile.favorite ? t('home.unfavorite') : t('home.favorite')"
                :title="profile.favorite ? t('home.unfavorite') : t('home.favorite')"
                @click="emit('toggleFavorite', profile.id)"
              />
              <button type="button" class="row-name" @click="emit('open', profile)">
                <OsIconTile :icon="resolveProfileOsIcon(profile)" :size="28" />
                <span>{{ profile.name }}</span>
                <span v-if="profile.production" class="production-badge">{{ t("profile.productionBadge") }}</span>
              </button>
            </td>
            <td class="cell-muted">{{ profile.host }}</td>
            <td class="cell-muted">{{ profile.username }}</td>
            <td class="cell-muted">{{ groupName ?? t("sidebar.ungrouped") }}</td>
            <td>
              <span class="row-status">
                <i></i>
                <span>{{ statusLabel(status) }}</span>
              </span>
            </td>
            <td class="cell-muted">{{ formatRelativeTime(profile.lastConnectedAt, t) }}</td>
            <td class="col-actions">
              <UButton
                color="neutral"
                variant="ghost"
                size="xs"
                icon="i-lucide-play"
                :aria-label="t('home.connect')"
                :title="t('home.connect')"
                @click="emit('open', profile)"
              />
              <UButton
                color="neutral"
                variant="ghost"
                size="xs"
                icon="i-lucide-pencil"
                :aria-label="t('sidebar.editConnection')"
                :title="t('sidebar.editConnection')"
                @click="emit('edit', profile.id)"
              />
              <UDropdownMenu
                v-if="menuItems?.length"
                :items="menuItems"
                size="xs"
                :content="{ align: 'end' }"
              >
                <UButton
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  icon="i-lucide-ellipsis"
                  :aria-label="t('home.moreActions')"
                  :title="t('home.moreActions')"
                />
              </UDropdownMenu>
            </td>
          </tr>
        </UContextMenu>
      </tbody>
    </table>
  </div>
</template>

<style scoped src="../../assets/css/connection-table.css"></style>
