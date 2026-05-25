<script setup>
defineProps({
  summary: {
    type: Object,
    required: true,
  },
  tasks: {
    type: Array,
    required: true,
  },
});
</script>

<template>
  <aside class="transfer-toast" aria-label="Transfer queue">
    <div
      v-for="task in tasks"
      :key="task.id"
      class="transfer-metric"
      :class="task.direction"
    >
      <UIcon :name="task.direction === 'upload' ? 'i-lucide-arrow-up' : 'i-lucide-arrow-down'" />
      <div>
        <strong>{{ task.speed }}</strong>
        <span>{{ task.label }}</span>
      </div>
    </div>

    <button type="button" aria-label="Pause transfer">
      <UIcon name="i-lucide-pause" />
    </button>

    <div class="transfer-progress">
      <span :style="{ width: `${summary.progress}%` }"></span>
    </div>

    <small>{{ summary.progress }}%</small>
  </aside>
</template>

<style scoped>
.transfer-toast {
  position: absolute;
  right: 26px;
  bottom: 78px;
  width: min(500px, calc(100% - 52px));
  min-height: 142px;
  display: grid;
  grid-template-columns: 1fr 1fr 54px;
  grid-template-rows: 1fr 22px;
  gap: 14px 18px;
  align-items: center;
  padding: 24px 26px;
  color: #0f172a;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.93);
  box-shadow: 0 24px 60px rgba(15, 23, 42, 0.23);
  backdrop-filter: blur(20px);
}

.transfer-metric {
  display: grid;
  grid-template-columns: 24px 1fr;
  align-items: center;
  gap: 10px;
}

.transfer-metric :deep(svg) {
  color: #0757e6;
  font-size: 23px;
}

.transfer-metric.download :deep(svg) {
  color: #16a34a;
}

.transfer-metric strong {
  display: block;
  font-size: 16px;
  font-weight: 780;
}

.transfer-metric span {
  display: block;
  margin-top: 6px;
  color: #64748b;
  font-size: 12px;
}

.transfer-toast button {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  justify-self: end;
  color: #475569;
  border: 1px solid rgba(148, 163, 184, 0.2);
  border-radius: 50%;
  background: #ffffff;
}

.transfer-progress {
  grid-column: 1 / 3;
  height: 7px;
  overflow: hidden;
  border-radius: 999px;
  background: #e2e8f0;
}

.transfer-progress span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #0757e6, #2f8cff);
}

.transfer-toast small {
  justify-self: end;
  color: #64748b;
  font-size: 12px;
}

@media (max-width: 980px) {
  .transfer-toast {
    right: 18px;
    width: calc(100% - 36px);
  }
}
</style>
