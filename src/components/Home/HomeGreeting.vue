<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useNow } from "@vueuse/core";

defineProps({
  hostCount: { type: Number, required: true },
  onlineCount: { type: Number, default: 0 },
  sessionCount: { type: Number, default: 0 },
});

const { t } = useI18n();
// 问候语只在跨过时段边界时才变，按分钟刷新足够
const now = useNow({ interval: 60_000 });

// 23 点以后不再说「晚上好」：这个点还开着服务器的人，值得一句不一样的
function periodOf(hour) {
  if (hour < 5 || hour >= 23) return "night";
  if (hour < 11) return "morning";
  if (hour < 13) return "noon";
  if (hour < 18) return "afternoon";
  return "evening";
}

const greeting = computed(() => t(`home.greeting.${periodOf(now.value.getHours())}`));
</script>

<template>
  <header class="home-greeting">
    <h1>{{ greeting }}</h1>
    <p v-if="hostCount" class="home-greeting-summary">
      <span>{{ t("home.summary.hosts", hostCount) }}</span>
      <span v-if="onlineCount" class="home-greeting-online">
        <i aria-hidden="true"></i>{{ t("home.summary.online", onlineCount) }}
      </span>
      <span v-if="sessionCount">{{ t("home.summary.sessions", sessionCount) }}</span>
    </p>
  </header>
</template>

<style scoped src="../../assets/css/home-greeting.css"></style>
