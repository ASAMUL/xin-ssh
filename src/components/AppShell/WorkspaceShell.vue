<script setup>
import { computed } from "vue";
import { Pane, Splitpanes } from "splitpanes";
import ConnectionForm from "../ConnectionManager/ConnectionForm.vue";
import RemoteFileEditor from "../FileEditor/RemoteFileEditor.vue";
import SettingsPanel from "../Settings/SettingsPanel.vue";
import TerminalWorkspace from "../Terminal/TerminalWorkspace.vue";
import HomeDashboard from "../Home/HomeDashboard.vue";
import HostMonitorSidebar from "./HostMonitorSidebar.vue";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";

const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();

const hasActiveSession = computed(() => Boolean(sessionStore.activeSession));

// splitpanes 拖拽结束才落盘；兼容事件载荷为数组或 { panes } 两种形态
function persistMonitorPaneSize(event) {
  const panes = Array.isArray(event) ? event : event?.panes ?? [];
  if (!settingsStore.showMonitorSidebar || panes.length < 2) return;

  settingsStore.applySettingsPatch({ monitorPaneSize: panes[0].size });
}
</script>

<template>
  <section class="workspace-layout" :class="{ 'connection-hub-layout': !hasActiveSession }">
    <Splitpanes
      v-if="hasActiveSession"
      class="xin-split workspace-split"
      @resized="persistMonitorPaneSize"
    >
      <Pane
        v-if="settingsStore.showMonitorSidebar"
        :size="settingsStore.monitorPaneSize"
        min-size="14"
        max-size="44"
      >
        <HostMonitorSidebar />
      </Pane>
      <Pane :size="settingsStore.showMonitorSidebar ? 100 - settingsStore.monitorPaneSize : 100">
        <TerminalWorkspace />
      </Pane>
    </Splitpanes>

    <section v-else class="workspace-main">
      <HomeDashboard />
    </section>

    <ConnectionForm
      v-if="uiStore.isConnectionFormOpen"
      :profile-id="uiStore.editingProfileId"
      @close="uiStore.closeConnectionForm"
    />
    <SettingsPanel v-if="uiStore.isSettingsOpen" @close="uiStore.closeSettings" />
    <RemoteFileEditor />
  </section>
</template>
