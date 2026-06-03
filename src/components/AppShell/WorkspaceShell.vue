<script setup>
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import ConnectionForm from "../ConnectionManager/ConnectionForm.vue";
import ConnectionLauncher from "../ConnectionManager/ConnectionLauncher.vue";
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
const { t } = useI18n();

const hasProfiles = computed(() => profileStore.activeProfiles.length > 0);
const hasActiveSession = computed(() => Boolean(sessionStore.activeSession));

function closeSession(sessionId) {
  sessionStore.closeSession(sessionId);
}
</script>

<template>
  <section class="workspace-layout">
    <HostMonitorSidebar />

    <section class="workspace-main">
      <div class="workspace-toolbar" data-tauri-drag-region>
        <ConnectionLauncher />

        <div class="toolbar-tabs" aria-label="Open sessions">
          <div
            v-for="session in sessionStore.sessions"
            :key="session.id"
            class="workspace-session-tab"
            :class="{ active: session.id === sessionStore.activeSessionId }"
          >
            <button type="button" class="workspace-session-main" @click="sessionStore.selectSession(session.id)">
              <span class="workspace-tab-status" :class="session.status"></span>
              <span>{{ session.name }}</span>
            </button>
            <button
              type="button"
              class="workspace-session-close"
              :aria-label="t('terminal.closeSession')"
              @click="closeSession(session.id)"
            >
              <UIcon name="i-lucide-x" />
            </button>
          </div>
        </div>

        <div class="toolbar-actions">
          <button type="button" aria-label="Settings" @click="uiStore.openSettings">
            <UIcon name="i-lucide-settings" />
          </button>
        </div>
      </div>

      <TerminalWorkspace v-if="hasActiveSession" />
      <EmptyWorkspace v-else :has-profiles="hasProfiles" @create-connection="uiStore.openConnectionForm" />
    </section>

    <ConnectionForm
      v-if="uiStore.isConnectionFormOpen"
      :profile-id="uiStore.editingProfileId"
      @close="uiStore.closeConnectionForm"
    />
    <SettingsPanel v-if="uiStore.isSettingsOpen" @close="uiStore.closeSettings" />
  </section>
</template>
