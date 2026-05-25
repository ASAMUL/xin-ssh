<script setup>
defineProps({
  sessions: {
    type: Array,
    required: true,
  },
  activeSessionId: {
    type: String,
    required: true,
  },
});

const emit = defineEmits(["select"]);
</script>

<template>
  <header class="session-tabs" aria-label="Open sessions">
    <div class="tabs-strip">
      <button
        v-for="session in sessions"
        :key="session.id"
        type="button"
        class="tab-button"
        :class="{ active: session.id === activeSessionId }"
        @click="emit('select', session.id)"
      >
        <span class="tab-status" :class="session.status"></span>
        <span class="tab-title">{{ session.name }}</span>
        <UIcon name="i-lucide-x" class="tab-close" />
      </button>

      <button type="button" class="new-tab" aria-label="New session">
        <UIcon name="i-lucide-plus" />
      </button>
    </div>

    <div class="window-tools">
      <button type="button" aria-label="Toggle sidebar">
        <UIcon name="i-lucide-panel-left" />
      </button>
      <button type="button" aria-label="Fullscreen">
        <UIcon name="i-lucide-scan" />
      </button>
      <button type="button" aria-label="More">
        <UIcon name="i-lucide-more-horizontal" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.session-tabs {
  height: 64px;
  min-height: 64px;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding: 0 16px 0 24px;
  background: rgba(255, 255, 255, 0.76);
  border-bottom: 1px solid rgba(148, 163, 184, 0.2);
  backdrop-filter: blur(18px);
}

.tabs-strip {
  height: 100%;
  min-width: 0;
  display: flex;
  align-items: flex-end;
}

.tab-button {
  height: 52px;
  min-width: 202px;
  max-width: 244px;
  display: grid;
  grid-template-columns: 10px 1fr 18px;
  align-items: center;
  gap: 10px;
  padding: 0 18px;
  color: #1e293b;
  border: 0;
  border-left: 1px solid rgba(148, 163, 184, 0.18);
  background: rgba(226, 232, 240, 0.44);
  border-radius: 8px 8px 0 0;
}

.tab-button:first-child {
  border-left: 0;
}

.tab-button.active {
  height: 58px;
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 -1px 0 #ffffff inset;
}

.tab-status {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: #94a3b8;
}

.tab-status.online {
  background: #22c55e;
}

.tab-title {
  overflow: hidden;
  font-size: 14px;
  font-weight: 760;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tab-close {
  color: #64748b;
  font-size: 16px;
}

.new-tab,
.window-tools button {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  color: #334155;
  border: 0;
  border-radius: 8px;
  background: transparent;
}

.new-tab {
  margin: 0 10px 10px;
}

.window-tools {
  height: 52px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.new-tab:hover,
.window-tools button:hover {
  background: rgba(148, 163, 184, 0.12);
}

@media (max-width: 980px) {
  .tab-button {
    min-width: 150px;
  }
}
</style>
