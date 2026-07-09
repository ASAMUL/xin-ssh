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
      :title="t('workspace.connectionHub')"
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
          :title="session.name"
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
      :title="t('sidebar.add')"
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
  align-items: center;
  gap: 6px;
  overflow: hidden;
  padding: 0 6px 0 4px;
}

.home-tab,
.session-tab {
  height: 36px;
  border: 1px solid transparent;
  border-radius: 12px;
  background: transparent;
  box-shadow: none;
  color: var(--xin-text-muted);
  transition:
    background 140ms ease,
    border-color 140ms ease,
    color 140ms ease,
    box-shadow 140ms ease;
}

.home-tab:hover,
.session-tab:hover {
  color: var(--xin-text);
  background: rgba(255, 255, 255, 0.4);
}

.home-tab.active,
.session-tab.active {
  color: var(--xin-text);
  border-color: var(--xin-glass-border);
  background: rgba(255, 255, 255, 0.6);
  box-shadow:
    var(--xin-specular),
    0 8px 22px rgba(20, 34, 62, 0.1);
  backdrop-filter: var(--xin-glass-blur);
}

.home-tab {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 104px;
  padding: 0 15px;
  font-size: 12px;
  font-weight: 640;
  white-space: nowrap;
}

.home-tab :deep(svg) {
  width: 16px;
  height: 16px;
}

.tabs-strip {
  min-width: 0;
  height: 100%;
  display: flex;
  align-items: center;
  gap: 6px;
  overflow-x: auto;
  scrollbar-width: none;
}

.tabs-strip::-webkit-scrollbar {
  display: none;
}

.session-tab {
  width: auto;
  min-width: 128px;
  max-width: 226px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 30px;
  align-items: center;
  flex: 0 0 auto;
}

.session-tab-main {
  min-width: 0;
  height: 100%;
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  padding: 0 8px 0 13px;
  color: inherit;
  border: 0;
  background: transparent;
  text-align: left;
}

.tab-status {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #94a3b8;
  box-shadow: 0 0 0 4px rgba(148, 163, 184, 0.12);
}

.tab-status.authRequired {
  background: var(--xin-accent);
  box-shadow: 0 0 0 4px rgba(47, 109, 246, 0.13);
}

.tab-status.connected {
  background: var(--xin-success);
  box-shadow: 0 0 0 4px rgba(53, 184, 113, 0.16);
}

.tab-status.connecting {
  background: var(--xin-warning);
  box-shadow: 0 0 0 4px rgba(215, 166, 65, 0.14);
}

.tab-status.error {
  background: #ef4444;
  box-shadow: 0 0 0 4px rgba(239, 68, 68, 0.12);
}

.tab-title {
  min-width: 0;
  overflow: hidden;
  font-size: 12px;
  font-weight: 640;
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
  border-radius: 8px;
  background: transparent;
}

.tab-close:hover {
  color: #0f172a;
  background: rgba(148, 163, 184, 0.18);
}

.new-connection-tab {
  width: 36px;
  height: 36px;
  min-width: 36px;
  justify-content: center;
  color: var(--xin-text-muted);
  border: 1px solid var(--xin-glass-border);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.5);
  box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8) inset;
}

.new-connection-tab:hover {
  color: var(--xin-text);
  border-color: rgba(47, 109, 246, 0.3);
  background: rgba(255, 255, 255, 0.85);
}

@media (max-width: 980px) {
  .home-tab {
    min-width: 44px;
    justify-content: center;
    padding: 0 10px;
  }

  .home-tab span {
    display: none;
  }

  .session-tab {
    max-width: 168px;
  }
}

@media (max-width: 760px) {
  .session-tab {
    min-width: 108px;
    max-width: 140px;
  }
}
</style>
