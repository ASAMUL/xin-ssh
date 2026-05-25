<script setup>
defineProps({
  connections: {
    type: Array,
    required: true,
  },
  activeConnectionId: {
    type: String,
    required: true,
  },
});

const emit = defineEmits(["select"]);

function statusLabel(status) {
  return status === "offline" ? "Disconnected" : "Connected";
}
</script>

<template>
  <aside class="sidebar" aria-label="Connections">
    <div class="traffic-lights" aria-hidden="true">
      <span class="traffic-light close"></span>
      <span class="traffic-light minimize"></span>
      <span class="traffic-light maximize"></span>
    </div>

    <div class="brand-row">
      <div class="brand-mark">
        <UIcon name="i-lucide-terminal" />
      </div>
      <div class="brand-copy">
        <div class="brand-title">
          Xin Terminal
          <span>Pro</span>
        </div>
      </div>
    </div>

    <label class="search-field">
      <UIcon name="i-lucide-search" />
      <input type="search" placeholder="Search" />
      <kbd>⌘ K</kbd>
    </label>

    <div class="section-heading">
      <span>Connections</span>
      <button type="button" aria-label="Add connection">
        <UIcon name="i-lucide-plus" />
      </button>
    </div>

    <nav class="connection-list">
      <button
        v-for="connection in connections"
        :key="connection.id"
        type="button"
        class="connection-item"
        :class="{ active: connection.id === activeConnectionId }"
        :style="{ '--connection-accent': connection.accent }"
        @click="emit('select', connection.id)"
      >
        <span
          class="status-dot"
          :class="connection.status"
          :aria-label="statusLabel(connection.status)"
        ></span>

        <span class="connection-icon">
          <UIcon :name="connection.icon" />
        </span>

        <span class="connection-copy">
          <span class="connection-name">{{ connection.name }}</span>
          <span class="connection-host">{{ connection.host }}</span>
        </span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <button type="button" aria-label="Settings">
        <UIcon name="i-lucide-settings" />
      </button>
      <button type="button" aria-label="Toggle theme">
        <UIcon name="i-lucide-moon" />
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 326px;
  min-width: 292px;
  height: 100%;
  padding: 22px 18px 22px;
  display: flex;
  flex-direction: column;
  gap: 26px;
  background:
    linear-gradient(160deg, rgba(255, 255, 255, 0.92), rgba(235, 244, 255, 0.82)),
    radial-gradient(circle at 18% 100%, rgba(96, 165, 250, 0.22), transparent 34%);
  border-right: 1px solid rgba(148, 163, 184, 0.28);
  box-shadow: inset -1px 0 0 rgba(255, 255, 255, 0.56);
}

.traffic-lights {
  display: flex;
  gap: 11px;
  padding: 3px 8px;
}

.traffic-light {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  box-shadow: inset 0 -1px 1px rgba(15, 23, 42, 0.12);
}

.traffic-light.close {
  background: #ff5f57;
}

.traffic-light.minimize {
  background: #ffbd2e;
}

.traffic-light.maximize {
  background: #28c840;
}

.brand-row {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 8px 10px 0;
}

.brand-mark {
  width: 46px;
  height: 46px;
  display: grid;
  place-items: center;
  color: #ffffff;
  font-size: 24px;
  border-radius: 8px;
  background: linear-gradient(145deg, #2f8cff, #0757e6);
  box-shadow: 0 12px 24px rgba(37, 99, 235, 0.32);
}

.brand-copy {
  min-width: 0;
}

.brand-title {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #020617;
  font-size: 20px;
  font-weight: 780;
}

.brand-title span {
  padding: 2px 7px;
  color: #0757e6;
  font-size: 11px;
  font-weight: 760;
  border: 1px solid rgba(37, 99, 235, 0.16);
  border-radius: 6px;
  background: rgba(37, 99, 235, 0.07);
}

.search-field {
  height: 46px;
  display: grid;
  grid-template-columns: 22px 1fr auto;
  align-items: center;
  gap: 8px;
  padding: 0 13px;
  color: #64748b;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 8px;
  background: rgba(226, 232, 240, 0.55);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.58);
}

.search-field input {
  width: 100%;
  min-width: 0;
  color: #0f172a;
  font-size: 14px;
  background: transparent;
  border: 0;
  outline: 0;
}

.search-field kbd {
  color: #475569;
  font-size: 13px;
  font-family: inherit;
}

.section-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: -12px;
  padding: 0 8px;
  color: #334155;
  font-size: 12px;
  font-weight: 740;
  text-transform: uppercase;
}

.section-heading button,
.sidebar-footer button {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  color: #475569;
  border: 0;
  border-radius: 8px;
  background: transparent;
}

.section-heading button:hover,
.sidebar-footer button:hover {
  color: #0f172a;
  background: rgba(148, 163, 184, 0.12);
}

.connection-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.connection-item {
  width: 100%;
  height: 74px;
  display: grid;
  grid-template-columns: 18px 46px 1fr;
  align-items: center;
  gap: 12px;
  padding: 0 14px;
  text-align: left;
  border: 0;
  border-radius: 8px;
  color: #0f172a;
  background: transparent;
  transition:
    transform 160ms ease,
    background 160ms ease,
    color 160ms ease,
    box-shadow 160ms ease;
}

.connection-item:hover {
  background: rgba(255, 255, 255, 0.58);
}

.connection-item.active {
  color: #ffffff;
  background: linear-gradient(135deg, #61a5ff, #1463ec);
  box-shadow: 0 18px 38px rgba(37, 99, 235, 0.32);
}

.status-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: #94a3b8;
}

.status-dot.online {
  background: #22c55e;
}

.status-dot.warning {
  background: #f6b600;
}

.connection-icon {
  width: 46px;
  height: 46px;
  display: grid;
  place-items: center;
  color: #ffffff;
  font-size: 23px;
  border-radius: 8px;
  background: linear-gradient(145deg, color-mix(in srgb, var(--connection-accent) 76%, white), var(--connection-accent));
  box-shadow: 0 12px 25px color-mix(in srgb, var(--connection-accent) 30%, transparent);
}

.connection-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.connection-name {
  overflow: hidden;
  font-size: 14px;
  font-weight: 760;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.connection-host {
  overflow: hidden;
  color: #64748b;
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.connection-item.active .connection-host {
  color: rgba(255, 255, 255, 0.82);
}

.sidebar-footer {
  margin-top: auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
}

@media (max-width: 920px) {
  .sidebar {
    width: 280px;
    min-width: 250px;
  }
}
</style>
