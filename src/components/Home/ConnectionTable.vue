<script setup>
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Star } from "@respeak/lucide-motion-vue";
import MotionIcon from "../MotionIcon.vue";
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

// 和卡片一样写成 user@host，非 22 端口才带上端口：用户名几乎全是 root，单独一列只是重复
function address(profile) {
  const target = `${profile.username}@${profile.host}`;
  return Number(profile.port) === 22 ? target : `${target}:${profile.port}`;
}
</script>

<template>
  <div class="connection-table">
    <table>
      <thead>
        <tr>
          <th class="col-name">{{ t("home.table.name") }}</th>
          <th>{{ t("home.table.address") }}</th>
          <th>{{ t("home.table.group") }}</th>
          <th>{{ t("home.table.lastConnected") }}</th>
          <!-- 按钮悬停才出现，表头写着「操作」底下却空着，像漏了东西；只留给读屏 -->
          <th class="col-actions"><span class="sr-only">{{ t("home.table.actions") }}</span></th>
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
              <button type="button" class="row-name" @click="emit('open', profile)">
                <OsIconTile :icon="resolveProfileOsIcon(profile)" :size="28" />
                <!-- 只有开着会话才亮一颗点（和卡片一样）：一整列灰色的「未打开」全是噪音 -->
                <i
                  v-if="status !== 'notOpen'"
                  class="row-status"
                  role="img"
                  :aria-label="t(`monitor.status.${status}`)"
                  :title="t(`monitor.status.${status}`)"
                ></i>
                <span>{{ profile.name }}</span>
                <!-- 收藏星跟在名字后面，只在收藏了时出现；切换收藏在行尾的悬停按钮里。
                     以前星排在图标前面、平时藏着，每行图标前都空一格，和「名称」表头对不齐 -->
                <UIcon v-if="profile.favorite" class="row-favorite" name="i-lucide-star" aria-hidden="true" />
                <span v-if="profile.production" class="production-badge">{{ t("profile.productionBadge") }}</span>
              </button>
            </td>
            <td class="cell-muted">{{ address(profile) }}</td>
            <td class="cell-muted">{{ groupName ?? t("sidebar.ungrouped") }}</td>
            <td class="cell-muted">{{ formatRelativeTime(profile.lastConnectedAt, t) }}</td>
            <td class="col-actions">
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
              >
                <template #leading="{ ui }">
                  <MotionIcon :icon="Star" :play-on="profile.favorite" :class="ui.leadingIcon()" />
                </template>
              </UButton>
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
