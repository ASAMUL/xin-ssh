<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import CodeMirrorEditor from "./CodeMirrorEditor.vue";
import { TERMINAL_FONTS } from "../../config/terminalThemes";
import { useEditorStore } from "../../stores/editorStore";
import { useSettingsStore } from "../../stores/settingsStore";

const { t, locale } = useI18n();
const editorStore = useEditorStore();
const settingsStore = useSettingsStore();
const editorRef = ref(null);
const editorLanguage = ref("");
const editorCursor = ref({ line: 1, column: 1, selectionLength: 0 });

const editorModalUi = {
  content:
    "remote-editor-modal max-w-[min(1180px,94vw)] sm:max-w-[min(1180px,94vw)] h-[86vh] flex flex-col",
  body: "relative flex-1 min-h-0 p-0 sm:p-0 overflow-hidden flex flex-col",
  footer: "justify-between gap-3",
};

// 编辑器与终端共用等宽字体设置
const editorFontStack = computed(() => {
  const font = TERMINAL_FONTS.find((item) => item.name === settingsStore.terminalFontFamily);
  return (font ?? TERMINAL_FONTS[0]).stack;
});

const loadErrorText = computed(() => localizeEditorError(editorStore.loadError));
const saveErrorText = computed(() => localizeEditorError(editorStore.saveError));
// 大小/编码类错误重试也不会成功，只有连接类错误才给重试按钮
const isLoadRetryable = computed(() => {
  const message = String(editorStore.loadError || "");
  if (!message) return false;

  return !(
    message.includes("too large") ||
    message.includes("binary") ||
    message.includes("UTF-8") ||
    message.includes("is a directory") ||
    message.includes("not a regular file")
  );
});
const savedAtFormatter = computed(
  () =>
    new Intl.DateTimeFormat(locale.value, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }),
);
const savedAtLabel = computed(() =>
  editorStore.lastSavedAt ? savedAtFormatter.value.format(new Date(editorStore.lastSavedAt)) : "",
);
const editorLanguageLabel = computed(() => editorLanguage.value || t("editor.plainText"));
const editorCursorLabel = computed(() =>
  t("editor.cursorPosition", {
    line: editorCursor.value.line,
    column: editorCursor.value.column,
  }),
);
const editorSelectionLabel = computed(() =>
  t("editor.selectionLength", { count: editorCursor.value.selectionLength }),
);

watch(
  () => editorStore.path,
  () => {
    editorLanguage.value = "";
    editorCursor.value = { line: 1, column: 1, selectionLength: 0 };
  },
);

onMounted(() => {
  window.addEventListener("keydown", handleWindowKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWindowKeydown);
});

function localizeEditorError(error) {
  if (!error) return "";

  const message = String(error);
  if (message.includes("too large")) return t("editor.fileTooLarge");
  if (message.includes("binary") || message.includes("UTF-8")) return t("editor.fileNotText");
  if (message.includes("is a directory")) return t("editor.isDirectory");
  if (message.includes("not a regular file")) return t("editor.notRegularFile");
  if (message.includes("not running")) return t("editor.sessionDisconnected");

  return message;
}

function save(options = {}) {
  if (!editorStore.hasDocument || editorStore.isSaving) return;
  if (!editorStore.isDirty && !options.closeAfter) return;

  const content = editorRef.value?.getContent();
  if (content == null) return;

  editorStore.saveFile(content, options);
}

function saveAndClose() {
  if (!editorStore.isDirty) {
    editorStore.close();
    return;
  }

  save({ closeAfter: true });
}

function handleWindowKeydown(event) {
  if (!editorStore.isOpen || event.defaultPrevented) return;

  if ((event.ctrlKey || event.metaKey) && event.key?.toLowerCase() === "s") {
    event.preventDefault();
    save();
    return;
  }

  // 焦点在弹窗其他位置（如底部按钮）时也能用 Ctrl+F 唤起编辑器搜索；
  // 焦点在编辑器内时 CodeMirror 快捷键已 preventDefault，不会走到这里
  if ((event.ctrlKey || event.metaKey) && event.key?.toLowerCase() === "f") {
    event.preventDefault();
    editorRef.value?.openSearch();
    return;
  }

  if (event.key === "Escape") {
    if (editorStore.confirmingClose) {
      editorStore.cancelCloseConfirm();
    } else {
      editorStore.requestClose();
    }
  }
}

// dismissible=false 时 UModal 不会自行关闭，这里兜底外部触发的关闭请求
function handleOpenChange(open) {
  if (!open) editorStore.requestClose();
}

function handleLanguageChange(language) {
  editorLanguage.value = language;
}

function handleCursorChange(position) {
  editorCursor.value = position;
}
</script>

<template>
  <UModal
    :open="editorStore.isOpen"
    :dismissible="false"
    :close="false"
    :ui="editorModalUi"
    @update:open="handleOpenChange"
  >
    <template #header>
      <div class="remote-editor-header">
        <span class="remote-editor-file-icon">
          <UIcon name="i-lucide-file-pen-line" />
        </span>
        <div class="remote-editor-heading">
          <strong class="remote-editor-name">
            {{ editorStore.name }}
            <span
              v-if="editorStore.isDirty"
              class="remote-editor-dirty-dot"
              :title="t('editor.unsavedChanges')"
            ></span>
          </strong>
          <span class="remote-editor-path" :title="editorStore.path">{{ editorStore.path }}</span>
        </div>
        <button
          type="button"
          class="remote-editor-close"
          :aria-label="t('editor.close')"
          @click="editorStore.requestClose()"
        >
          <UIcon name="i-lucide-x" />
        </button>
      </div>
    </template>

    <template #body>
      <div v-if="editorStore.isLoading" class="remote-editor-state">
        <UIcon name="i-lucide-loader-circle" class="spin-icon" />
        <strong>{{ t("editor.loading") }}</strong>
        <span>{{ editorStore.path }}</span>
      </div>

      <div v-else-if="editorStore.loadError" class="remote-editor-state error">
        <UIcon name="i-lucide-circle-alert" />
        <strong>{{ t("editor.loadFailed") }}</strong>
        <span>{{ loadErrorText }}</span>
        <UButton
          v-if="isLoadRetryable"
          color="neutral"
          variant="outline"
          icon="i-lucide-refresh-cw"
          :label="t('editor.retry')"
          @click="editorStore.retryLoad()"
        />
      </div>

      <template v-else>
        <CodeMirrorEditor
          ref="editorRef"
          class="remote-editor-surface"
          :style="{ fontFamily: editorFontStack }"
          :content="editorStore.initialContent"
          :filename="editorStore.name"
          @change="editorStore.markDirty()"
          @cursor-change="handleCursorChange"
          @language-change="handleLanguageChange"
          @save="save()"
        />
        <UAlert
          v-if="editorStore.saveError"
          class="remote-editor-save-alert"
          color="error"
          variant="soft"
          icon="i-lucide-circle-alert"
          :title="t('editor.saveFailed')"
          :description="saveErrorText"
        />
      </template>
    </template>

    <template #footer>
      <template v-if="editorStore.confirmingClose">
        <div class="remote-editor-confirm">
          <UIcon name="i-lucide-triangle-alert" />
          <span>{{ t("editor.dirtyCloseHint") }}</span>
        </div>
        <div class="remote-editor-actions">
          <UButton
            color="neutral"
            variant="outline"
            :disabled="editorStore.isSaving"
            :label="t('editor.discardChanges')"
            @click="editorStore.discardAndClose()"
          />
          <UButton
            color="neutral"
            variant="soft"
            :disabled="editorStore.isSaving"
            :label="t('editor.keepEditing')"
            @click="editorStore.cancelCloseConfirm()"
          />
          <UButton
            color="primary"
            icon="i-lucide-save"
            :loading="editorStore.isSaving"
            :label="t('editor.saveAndClose')"
            @click="saveAndClose"
          />
        </div>
      </template>

      <template v-else>
        <div class="remote-editor-meta">
          <div v-if="editorStore.hasDocument" class="remote-editor-status">
            <span class="remote-editor-status-item remote-editor-language">
              <UIcon name="i-lucide-code-2" />
              {{ editorLanguageLabel }}
            </span>
            <span class="remote-editor-status-item">UTF-8</span>
            <span class="remote-editor-status-item">{{ editorCursorLabel }}</span>
            <span v-if="editorCursor.selectionLength" class="remote-editor-status-item">
              {{ editorSelectionLabel }}
            </span>
          </div>
          <span v-if="editorStore.isSaving" class="remote-editor-save-state">{{ t("editor.saving") }}</span>
          <span v-else-if="editorStore.isDirty" class="remote-editor-save-state">
            {{ t("editor.unsavedChanges") }}
          </span>
          <span v-else-if="savedAtLabel" class="remote-editor-save-state">
            {{ t("editor.savedAt", { time: savedAtLabel }) }}
          </span>
        </div>
        <div class="remote-editor-actions">
          <UButton
            color="neutral"
            variant="outline"
            :label="t('editor.close')"
            @click="editorStore.requestClose()"
          />
          <UButton
            color="primary"
            icon="i-lucide-save"
            :loading="editorStore.isSaving"
            :disabled="!editorStore.hasDocument || !editorStore.isDirty"
            :label="t('editor.save')"
            @click="save()"
          />
        </div>
      </template>
    </template>
  </UModal>
</template>

<style scoped src="../../assets/css/remote-file-editor.css"></style>
