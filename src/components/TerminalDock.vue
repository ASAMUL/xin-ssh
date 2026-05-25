<script setup>
defineProps({
  activePanel: {
    type: String,
    required: true,
  },
  connection: {
    type: Object,
    default: null,
  },
  terminalLines: {
    type: Array,
    required: true,
  },
});

const emit = defineEmits(["updatePanel"]);

const panels = [
  { id: "terminal", label: "Terminal" },
  { id: "logs", label: "Logs" },
];
</script>

<template>
  <section class="terminal-dock" aria-label="Terminal panel">
    <div class="terminal-tabs">
      <div class="terminal-tab-group">
        <button
          v-for="panel in panels"
          :key="panel.id"
          type="button"
          class="terminal-tab"
          :class="{ active: panel.id === activePanel }"
          @click="emit('updatePanel', panel.id)"
        >
          <span v-if="panel.id === 'terminal'" class="active-dot"></span>
          {{ panel.label }}
        </button>
        <button type="button" class="terminal-add" aria-label="New terminal">
          <UIcon name="i-lucide-plus" />
        </button>
      </div>

      <div class="terminal-actions">
        <button type="button" aria-label="Clear terminal">
          <UIcon name="i-lucide-trash-2" />
        </button>
        <button type="button" aria-label="Pop out terminal">
          <UIcon name="i-lucide-copy" />
        </button>
        <button type="button" aria-label="Search terminal">
          <UIcon name="i-lucide-search" />
        </button>
        <button type="button" aria-label="Collapse terminal">
          <UIcon name="i-lucide-chevron-down" />
        </button>
      </div>
    </div>

    <pre class="terminal-body" aria-live="polite"><span
      v-for="line in terminalLines"
      :key="line.id"
      class="terminal-line"
      :class="line.kind"
    >{{ line.value }}</span></pre>
  </section>
</template>

<style scoped>
.terminal-dock {
  height: 367px;
  min-height: 270px;
  display: flex;
  flex-direction: column;
  color: #e2e8f0;
  background:
    radial-gradient(circle at 88% 16%, rgba(59, 130, 246, 0.14), transparent 24%),
    linear-gradient(145deg, #07111f, #030812 58%, #050a12);
  border-top: 1px solid rgba(15, 23, 42, 0.5);
  box-shadow: 0 -18px 44px rgba(15, 23, 42, 0.18);
}

.terminal-tabs {
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid rgba(148, 163, 184, 0.12);
  background: rgba(3, 7, 18, 0.42);
}

.terminal-tab-group {
  height: 100%;
  display: flex;
  align-items: center;
  padding-left: 16px;
}

.terminal-tab,
.terminal-add,
.terminal-actions button {
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: rgba(226, 232, 240, 0.74);
  border: 0;
  border-radius: 8px;
  background: transparent;
}

.terminal-tab {
  min-width: 116px;
  gap: 9px;
  padding: 0 17px;
  font-size: 13px;
  font-weight: 720;
}

.terminal-tab.active {
  color: #ffffff;
  background: rgba(15, 23, 42, 0.44);
  border-bottom: 1px solid #2f8cff;
  border-radius: 8px 8px 0 0;
}

.active-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #2f8cff;
  box-shadow: 0 0 16px rgba(47, 140, 255, 0.75);
}

.terminal-add {
  width: 36px;
  margin-left: 10px;
  border-left: 1px solid rgba(148, 163, 184, 0.1);
  border-radius: 0;
}

.terminal-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-right: 15px;
}

.terminal-actions button {
  width: 34px;
}

.terminal-tab:hover,
.terminal-add:hover,
.terminal-actions button:hover {
  color: #ffffff;
  background: rgba(148, 163, 184, 0.11);
}

.terminal-body {
  flex: 1;
  margin: 0;
  padding: 22px 24px 28px;
  overflow: auto;
  color: #e5e7eb;
  font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
  font-size: 16px;
  line-height: 1.42;
}

.terminal-line {
  display: block;
  white-space: pre-wrap;
}

.terminal-line.prompt {
  color: #22c55e;
}

.terminal-line.directory {
  color: #38bdf8;
}

@media (max-height: 760px) {
  .terminal-dock {
    height: 300px;
  }
}
</style>
