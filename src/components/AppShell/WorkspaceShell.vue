<script setup>
import { computed } from "vue";
import ConnectionSidebar from "../ConnectionManager/ConnectionSidebar.vue";
import ConnectionForm from "../ConnectionManager/ConnectionForm.vue";
import SettingsPanel from "../Settings/SettingsPanel.vue";
import EmptyWorkspace from "./EmptyWorkspace.vue";
import { useProfileStore } from "../../stores/profileStore";
import { useUiStore } from "../../stores/uiStore";

const profileStore = useProfileStore();
const uiStore = useUiStore();

const hasProfiles = computed(() => profileStore.activeProfiles.length > 0);
</script>

<template>
  <section class="workspace-layout">
    <ConnectionSidebar />

    <section class="workspace-main">
      <div class="workspace-toolbar" data-tauri-drag-region>
        <div class="toolbar-tabs" data-tauri-drag-region>
          <span class="inactive-tab" data-tauri-drag-region>Workspace</span>
        </div>
        <div class="toolbar-actions">
          <button type="button" aria-label="Settings" @click="uiStore.openSettings">
            <UIcon name="i-lucide-settings" />
          </button>
        </div>
      </div>

      <EmptyWorkspace :has-profiles="hasProfiles" @create-connection="uiStore.openConnectionForm" />
    </section>

    <ConnectionForm v-if="uiStore.isConnectionFormOpen" @close="uiStore.closeConnectionForm" />
    <SettingsPanel v-if="uiStore.isSettingsOpen" @close="uiStore.closeSettings" />
  </section>
</template>
