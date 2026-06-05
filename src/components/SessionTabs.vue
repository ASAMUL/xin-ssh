<script setup>
import { useI18n } from "vue-i18n";

defineProps({
  sessions: {
    type: Array,
    default: () => [],
  },
  activeSessionId: {
    type: String,
    default: null,
  },
});

const emit = defineEmits(["showConnectionHub", "select", "close", "create"]);
const { t } = useI18n();
</script>

<template>
  <nav class="session-tabs" :aria-label="t('workspace.openSessions')">
    <button
      type="button"
      class="home-tab"
      :class="{ active: !activeSessionId }"
      @click.stop="emit('showConnectionHub')"
    >
      <UIcon name="i-lucide-house" />
      <span>{{ t("workspace.connectionHub") }}</span>
    </button>

    <div class="tabs-strip">
      <div
        v-for="session in sessions"
        :key="session.id"
        class="session-tab"
        :class="{ active: session.id === activeSessionId }"
      >
        <button
          type="button"
          class="session-tab-main"
          :aria-label="session.name"
          @click.stop="emit('select', session.id)"
        >
          <span class="tab-status" :class="session.status"></span>
          <span class="tab-title">{{ session.name }}</span>
        </button>
        <button
          type="button"
          class="tab-close"
          :aria-label="t('terminal.closeSession')"
          @click.stop="emit('close', session.id)"
        >
          <UIcon name="i-lucide-x" />
        </button>
      </div>
    </div>

    <UButton
      class="new-connection-tab"
      color="neutral"
      variant="ghost"
      size="sm"
      icon="i-lucide-plus"
      :aria-label="t('sidebar.add')"
      @click.stop="emit('create')"
    />
  </nav>
</template>

<style scoped>
.session-tabs {
  min-width: 0;
  height: 100%;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: end;
  gap: 4px;
  padding-top: 8px;
  overflow: hidden;
}

.home-tab,
.session-tab,
.new-connection-tab {
  border-radius: 8px 8px 0 0;
}

.home-tab {
  height: 34px;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 0 13px;
  color: #475569;
  border: 1px solid transparent;
  border-bottom: 0;
  background: rgba(226, 232, 240, 0.5);
  font-size: 12px;
  font-weight: 760;
}

.home-tab.active {
  color: #0f172a;
  border-color: rgba(148, 163, 184, 0.2);
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 -1px 0 #ffffff inset;
}

.tabs-strip {
  min-width: 0;
  display: flex;
  align-items: end;
  gap: 4px;
  overflow-x: auto;
  scrollbar-width: none;
}

.tabs-strip::-webkit-scrollbar {
  display: none;
}

.session-tab {
  width: clamp(138px, 16vw, 208px);
  height: 34px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 26px;
  align-items: center;
  flex: 0 0 auto;
  color: #334155;
  border: 1px solid transparent;
  border-bottom: 0;
  background: rgba(226, 232, 240, 0.48);
}

.session-tab.active {
  height: 40px;
  color: #0f172a;
  border-color: rgba(148, 163, 184, 0.2);
  background: rgba(255, 255, 255, 0.94);
  box-shadow: 0 -1px 0 #ffffff inset;
}

.session-tab-main {
  min-width: 0;
  height: 100%;
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr);
  align-items: center;
  gap: 7px;
  padding: 0 8px 0 11px;
  color: inherit;
  border: 0;
  background: transparent;
  text-align: left;
}

.tab-status {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #94a3b8;
}

.tab-status.authRequired {
  background: #5b7cfa;
}

.tab-status.connected {
  background: var(--xin-success);
  box-shadow: 0 0 0 4px rgba(97, 191, 134, 0.13);
}

.tab-status.connecting {
  background: var(--xin-warning);
}

.tab-status.error {
  background: #ef4444;
}

.tab-title {
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  font-weight: 780;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tab-close {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  color: #64748b;
  border: 0;
  border-radius: 7px;
  background: transparent;
}

.tab-close:hover {
  color: #0f172a;
  background: rgba(148, 163, 184, 0.14);
}

.new-connection-tab {
  width: 36px;
  height: 34px;
  justify-content: center;
  margin-left: 2px;
  color: #334155;
  background: rgba(226, 232, 240, 0.5);
}

.new-connection-tab:hover {
  color: #0f172a;
  background: rgba(255, 255, 255, 0.86);
}

@media (max-width: 980px) {
  .home-tab span {
    display: none;
  }

  .session-tab {
    width: clamp(118px, 18vw, 160px);
  }
}
</style>
