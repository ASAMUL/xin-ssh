<script setup>
import { computed, ref } from "vue";
import SidebarConnections from "./components/SidebarConnections.vue";
import SessionTabs from "./components/SessionTabs.vue";
import FileExplorer from "./components/FileExplorer.vue";
import TerminalDock from "./components/TerminalDock.vue";
import TransferToast from "./components/TransferToast.vue";
import StatusBar from "./components/StatusBar.vue";
import {
  connections,
  fileEntries,
  pathCrumbs,
  sessions,
  terminalLines,
  transferSummary,
  transferTasks,
} from "./data/workspace";

const selectedConnectionId = ref("prod-web");
const activeSessionId = ref("prod-web");
const selectedFileId = ref("assets");
const activePanel = ref("terminal");

const activeConnection = computed(() =>
  connections.find((connection) => connection.id === selectedConnectionId.value),
);

const activeSession = computed(() =>
  sessions.find((session) => session.id === activeSessionId.value),
);

const selectedFile = computed(() =>
  fileEntries.find((fileEntry) => fileEntry.id === selectedFileId.value),
);

function selectConnection(connectionId) {
  selectedConnectionId.value = connectionId;

  const matchingSession = sessions.find((session) => session.connectionId === connectionId);
  if (matchingSession) {
    activeSessionId.value = matchingSession.id;
  }
}

function selectSession(sessionId) {
  activeSessionId.value = sessionId;

  const matchingSession = sessions.find((session) => session.id === sessionId);
  if (matchingSession) {
    selectedConnectionId.value = matchingSession.connectionId;
  }
}
</script>

<template>
  <UApp>
    <main class="app-shell">
      <SidebarConnections
        :connections="connections"
        :active-connection-id="selectedConnectionId"
        @select="selectConnection"
      />

      <section class="workspace-frame" aria-label="Xin Terminal workspace">
        <SessionTabs
          :sessions="sessions"
          :active-session-id="activeSessionId"
          @select="selectSession"
        />

        <FileExplorer
          :active-session="activeSession"
          :files="fileEntries"
          :path-crumbs="pathCrumbs"
          :selected-file-id="selectedFileId"
          @select-file="selectedFileId = $event"
        />

        <TerminalDock
          :active-panel="activePanel"
          :connection="activeConnection"
          :terminal-lines="terminalLines"
          @update-panel="activePanel = $event"
        />

        <TransferToast
          :summary="transferSummary"
          :tasks="transferTasks"
        />

        <StatusBar
          :connection="activeConnection"
          :selected-file="selectedFile"
          :summary="transferSummary"
        />
      </section>
    </main>
  </UApp>
</template>
