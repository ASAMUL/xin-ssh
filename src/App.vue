<script setup>
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import WindowChrome from "./components/AppShell/WindowChrome.vue";
import WorkspaceShell from "./components/AppShell/WorkspaceShell.vue";
import { useAppStore } from "./stores/appStore";

const { t } = useI18n();
const appStore = useAppStore();

const startupError = computed(() => appStore.error);

onMounted(() => {
  appStore.bootstrap();
});
</script>

<template>
  <UApp>
    <main class="desktop-window">
      <WindowChrome />

      <section class="desktop-content" aria-label="Xin Terminal">
        <div v-if="appStore.isLoading" class="startup-state">
          <div class="startup-mark">
            <UIcon name="i-lucide-terminal" />
          </div>
          <p>{{ t("app.loading") }}</p>
        </div>

        <WorkspaceShell v-else />

        <div v-if="startupError" class="error-banner">
          <strong>{{ t("errors.title") }}</strong>
          <span>{{ startupError }}</span>
        </div>
      </section>
    </main>
  </UApp>
</template>
