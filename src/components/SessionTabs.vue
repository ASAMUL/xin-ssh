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
  gap: 8px;
  overflow: hidden;
  padding: 0 8px;
}

.home-tab,
.session-tab,
.new-connection-tab {
  height: 38px;
  border-radius: 8px;
}

.home-tab,
.session-tab {
  border: 1px solid rgba(148, 163, 184, 0.16);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.72), rgba(238, 243, 249, 0.72)),
    rgba(238, 242, 247, 0.68);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.88) inset,
    0 8px 20px rgba(15, 23, 42, 0.04);
  transition:
    background 140ms ease,
    border-color 140ms ease,
    box-shadow 140ms ease,
    transform 140ms ease;
}

.home-tab:hover,
.session-tab:hover {
  border-color: rgba(47, 109, 246, 0.22);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.9), rgba(246, 248, 252, 0.82)),
    rgba(255, 255, 255, 0.78);
}

.home-tab.active,
.session-tab.active {
  color: #0f172a;
  border-color: rgba(148, 163, 184, 0.2);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(252, 253, 255, 0.9)),
    rgba(255, 255, 255, 0.92);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.96) inset,
    0 12px 28px rgba(15, 23, 42, 0.09);
  transform: translateY(-1px);
}

.home-tab {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 104px;
  padding: 0 15px;
  color: #475569;
  font-size: 12px;
  font-weight: 760;
  white-space: nowrap;
}

.home-tab :deep(svg) {
  width: 16px;
  height: 16px;
}

.tabs-strip {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  overflow: hidden;
}

.tabs-strip {
  overflow-x: auto;
  scrollbar-width: none;
}

.tabs-strip::-webkit-scrollbar {
  display: none;
}

.session-tab {
  width: clamp(154px, 17vw, 226px);
  display: grid;
  grid-template-columns: minmax(0, 1fr) 30px;
  align-items: center;
  flex: 0 0 auto;
  color: #334155;
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
  box-shadow: 0 0 0 4px rgba(97, 191, 134, 0.15);
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
  width: 38px;
  min-width: 38px;
  justify-content: center;
  color: #334155;
  border: 1px solid rgba(148, 163, 184, 0.16);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.7), rgba(238, 243, 249, 0.74)),
    rgba(238, 242, 247, 0.7);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.9) inset,
    0 8px 20px rgba(15, 23, 42, 0.04);
}

.new-connection-tab:hover {
  color: #0f172a;
  border-color: rgba(47, 109, 246, 0.22);
  background: rgba(255, 255, 255, 0.9);
}

@media (max-width: 980px) {
  .session-tabs {
    gap: 6px;
    padding: 0 6px;
  }

  .home-tab {
    min-width: 38px;
    justify-content: center;
    padding: 0 10px;
  }

  .home-tab span {
    display: none;
  }

  .session-tab {
    width: clamp(126px, 20vw, 168px);
  }
}

@media (max-width: 760px) {
  .session-tab {
    width: 118px;
  }
}
</style>
