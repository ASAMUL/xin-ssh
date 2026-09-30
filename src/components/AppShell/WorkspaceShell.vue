<script setup>
import { computed } from "vue";
import { Pane, Splitpanes } from "splitpanes";
import ConnectionForm from "../ConnectionManager/ConnectionForm.vue";
import RemoteFileEditor from "../FileEditor/RemoteFileEditor.vue";
import SessionTabs from "../SessionTabs.vue";
import SettingsPanel from "../Settings/SettingsPanel.vue";
import TerminalWorkspace from "../Terminal/TerminalWorkspace.vue";
import TunnelPanel from "../Tunnels/TunnelPanel.vue";
import HomeDashboard from "../Home/HomeDashboard.vue";
import HostMonitorSidebar from "./HostMonitorSidebar.vue";
import WorkspaceStatusBar from "./WorkspaceStatusBar.vue";
import { useSessionStore } from "../../stores/sessionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import { useWorkspaceNavigationLock } from "../../composables/useWorkspaceNavigationLock";

const sessionStore = useSessionStore();
const settingsStore = useSettingsStore();
const uiStore = useUiStore();
const navigationLocked = useWorkspaceNavigationLock();

const hasActiveSession = computed(() => Boolean(sessionStore.activeSession));
// 沉浸时概览栏整个不渲染（不是只隐藏），splitpanes 才会把宽度全部让给终端
const showMonitor = computed(() => settingsStore.showMonitorSidebar && !uiStore.isWorkspaceImmersive);

// splitpanes 拖拽结束才落盘；兼容事件载荷为数组或 { panes } 两种形态
function persistMonitorPaneSize(event) {
  const panes = Array.isArray(event) ? event : event?.panes ?? [];
  if (!showMonitor.value || panes.length < 2) return;

  settingsStore.applySettingsPatch({ monitorPaneSize: panes[0].size });
}
</script>

<template>
  <section class="workspace-layout" :class="{ 'connection-hub-layout': !hasActiveSession }">
    <div
      v-if="sessionStore.sessions.length"
      v-show="hasActiveSession"
      class="workspace-stage"
      :class="{ immersive: uiStore.isWorkspaceImmersive }"
    >
      <div v-if="!uiStore.isWorkspaceImmersive" class="workspace-session-bar">
        <SessionTabs
          v-liquid-glass="{ scale: 16 }"
          class="liquid-glass"
          :sessions="sessionStore.sessions"
          :active-session-id="sessionStore.activeSessionId"
          :disabled="navigationLocked"
          @select="sessionStore.selectSession"
          @close="sessionStore.closeSession"
          @move="({ id, beforeId }) => sessionStore.moveSession(id, beforeId)"
          @dragging="uiStore.setDraggingSessionTab"
        />
      </div>

      <Splitpanes class="xin-split workspace-split" @resized="persistMonitorPaneSize">
        <Pane
          v-if="showMonitor"
          key="monitor"
          :size="settingsStore.monitorPaneSize"
          min-size="14"
          max-size="44"
        >
          <HostMonitorSidebar />
        </Pane>
        <Pane key="terminal" :size="showMonitor ? 100 - settingsStore.monitorPaneSize : 100">
          <TerminalWorkspace />
        </Pane>
      </Splitpanes>

      <WorkspaceStatusBar v-if="!uiStore.isWorkspaceImmersive" />
    </div>

    <!-- 首页常驻、只切显隐：用 v-if 的话每次从工作区点回「连接」都要重建整页卡片、
         重算样式布局、重新生成玻璃的位移图，切换时能卡上半秒 -->
    <section v-show="!hasActiveSession" class="workspace-main">
      <HomeDashboard />
    </section>

    <ConnectionForm
      v-if="uiStore.isConnectionFormOpen"
      :profile-id="uiStore.editingProfileId"
      @close="uiStore.closeConnectionForm"
    />
    <SettingsPanel v-if="uiStore.isSettingsOpen" @close="uiStore.closeSettings" />
    <TunnelPanel v-if="uiStore.isTunnelPanelOpen" @close="uiStore.closeTunnelPanel" />
    <RemoteFileEditor />
  </section>
</template>

<style scoped src="../../assets/css/workspace-shell.css"></style>
