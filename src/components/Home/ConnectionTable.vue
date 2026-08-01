<script setup>
import { useI18n } from "vue-i18n";
import { formatRelativeTime } from "../../utils/relativeTime";

defineProps({
  // 行结构：{ profile, groupName, status }
  rows: {
    type: Array,
    default: () => [],
  },
});

const emit = defineEmits(["open", "edit", "toggleFavorite"]);
const { t } = useI18n();

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
      <tbody>
        <tr v-for="{ profile, groupName, status } in rows" :key="profile.id" :class="status">
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
              {{ profile.name }}
            </button>
          </td>
          <td class="cell-muted">{{ profile.host }}</td>
          <td class="cell-muted">{{ profile.username }}</td>
          <td class="cell-muted">{{ groupName }}</td>
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
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped src="../../assets/css/connection-table.css"></style>
