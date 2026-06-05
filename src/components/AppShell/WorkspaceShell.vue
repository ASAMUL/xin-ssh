<script setup>
import { computed } from "vue";
import ConnectionForm from "../ConnectionManager/ConnectionForm.vue";
import SettingsPanel from "../Settings/SettingsPanel.vue";
import TerminalWorkspace from "../Terminal/TerminalWorkspace.vue";
import EmptyWorkspace from "./EmptyWorkspace.vue";
import HostMonitorSidebar from "./HostMonitorSidebar.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useSessionStore } from "../../stores/sessionStore";
import { useUiStore } from "../../stores/uiStore";

const profileStore = useProfileStore();
const sessionStore = useSessionStore();
const uiStore = useUiStore();

const hasActiveSession = computed(() => Boolean(sessionStore.activeSession));
const profileStatuses = computed(() =>
  sessionStore.sessions.reduce((statuses, session) => {
    statuses[session.profileId] = session.status;
    return statuses;
  }, {}),
);

function openProfile(profile) {
  sessionStore.openProfileSession(profile);
}
</script>

<template>
  <section class="workspace-layout" :class="{ 'connection-hub-layout': !hasActiveSession }">
    <HostMonitorSidebar v-if="hasActiveSession" />

    <section class="workspace-main">
      <TerminalWorkspace v-if="hasActiveSession" />
      <EmptyWorkspace
        v-else
        :profile-groups="profileStore.profilesByGroup"
        :profile-statuses="profileStatuses"
        @open-profile="openProfile"
        @edit-profile="uiStore.openConnectionForm"
        @create-connection="uiStore.openConnectionForm"
      />
    </section>

    <ConnectionForm
      v-if="uiStore.isConnectionFormOpen"
      :profile-id="uiStore.editingProfileId"
      @close="uiStore.closeConnectionForm"
    />
    <SettingsPanel v-if="uiStore.isSettingsOpen" @close="uiStore.closeSettings" />
  </section>
</template>
