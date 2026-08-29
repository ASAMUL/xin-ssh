<script setup>
import { computed, nextTick, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "../../stores/sessionStore";
import { useSnippetStore } from "../../stores/snippetStore";
import { useUiStore } from "../../stores/uiStore";
import { localizeError } from "../../utils/appError";

const emit = defineEmits(["close"]);

const { t } = useI18n();
const sessionStore = useSessionStore();
const snippetStore = useSnippetStore();
const uiStore = useUiStore();

// list ⇄ edit 两个视图在同一侧板内切换，不叠加弹窗
const mode = ref("list");
const query = ref("");
const editingSnippetId = ref(null);
const pendingDeleteId = ref(null);
const isSaving = ref(false);
const formError = ref("");
const nameInput = ref(null);
const form = reactive({
  name: "",
  command: "",
});

if (!snippetStore.loaded) {
  snippetStore.loadSnippets();
}

const canSend = computed(() => sessionStore.activeSession?.status === "connected");

const filteredSnippets = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return snippetStore.activeSnippets;

  return snippetStore.activeSnippets.filter((snippet) =>
    [snippet.name, snippet.command].some((value) =>
      String(value ?? "").toLowerCase().includes(keyword),
    ),
  );
});

const isEmpty = computed(() => snippetStore.activeSnippets.length === 0);

// 单击 = 填入命令不回车（防误执行）；「运行」按钮 = 追加回车直接执行
function insertSnippet(snippet) {
  if (!canSend.value) return;

  sessionStore.sendActiveTerminalInput(snippet.command).catch((error) => uiStore.setError(error));
}

function runSnippet(snippet) {
  if (!canSend.value) return;

  sessionStore
    .sendActiveTerminalInput(`${snippet.command}\n`)
    .catch((error) => uiStore.setError(error));
}

async function startCreate() {
  editingSnippetId.value = null;
  form.name = "";
  form.command = "";
  formError.value = "";
  mode.value = "edit";
  await focusNameInput();
}

async function startEdit(snippet) {
  editingSnippetId.value = snippet.id;
  form.name = snippet.name;
  form.command = snippet.command;
  formError.value = "";
  mode.value = "edit";
  await focusNameInput();
}

async function focusNameInput() {
  await nextTick();
  nameInput.value?.inputRef?.focus();
}

function cancelEdit() {
  mode.value = "list";
  formError.value = "";
}

async function saveSnippet() {
  const name = form.name.trim();
  const command = form.command.replace(/\r\n/g, "\n").trimEnd();
  if (!name || !command) {
    formError.value = t("snippets.formRequired");
    return;
  }

  isSaving.value = true;
  formError.value = "";
  try {
    const now = Date.now().toString();
    const existing = snippetStore.snippets.find((item) => item.id === editingSnippetId.value);
    const snippetId = existing?.id ?? crypto.randomUUID();

    await snippetStore.saveSnippet({
      id: snippetId,
      syncId: existing?.syncId ?? snippetId,
      name,
      command,
      sortOrder: existing?.sortOrder ?? snippetStore.activeSnippets.length,
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
      deletedAt: existing?.deletedAt ?? null,
    });
    mode.value = "list";
  } catch (error) {
    formError.value = localizeError(error);
  } finally {
    isSaving.value = false;
  }
}

// 删除两段式：第一次点进入行内确认，确认后真删
function requestDelete(snippetId) {
  pendingDeleteId.value = snippetId;
}

function cancelDelete() {
  pendingDeleteId.value = null;
}

async function confirmDelete(snippetId) {
  try {
    await snippetStore.removeSnippet(snippetId);
  } catch (error) {
    uiStore.setError(error);
  } finally {
    pendingDeleteId.value = null;
  }
}
</script>

<template>
  <aside class="snippet-panel" @click.stop>
    <header class="snippet-head">
      <strong>{{ t("snippets.title") }}</strong>
      <button type="button" :aria-label="t('settings.close')" @click="emit('close')">
        <UIcon name="i-lucide-x" />
      </button>
    </header>

    <div v-if="mode === 'list'" class="snippet-body">
      <div class="snippet-toolbar">
        <UInput
          v-model="query"
          class="snippet-search"
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-lucide-search"
          :placeholder="t('snippets.searchPlaceholder')"
          spellcheck="false"
          autocomplete="off"
        />
      </div>

      <p v-if="!canSend" class="snippet-offline-hint">
        <UIcon name="i-lucide-plug-zap" />
        {{ t("snippets.notConnected") }}
      </p>

      <div class="snippet-list">
        <div v-if="isEmpty" class="snippet-empty">
          <UIcon name="i-lucide-scroll-text" />
          <p>{{ t("snippets.empty") }}</p>
          <span>{{ t("snippets.emptyHint") }}</span>
        </div>

        <p v-else-if="filteredSnippets.length === 0" class="snippet-no-match">
          {{ t("snippets.noMatches") }}
        </p>

        <div
          v-for="snippet in filteredSnippets"
          :key="snippet.id"
          class="snippet-item"
          :class="{ confirming: pendingDeleteId === snippet.id }"
        >
          <template v-if="pendingDeleteId === snippet.id">
            <span class="snippet-confirm-text">
              {{ t("snippets.deleteConfirm", { name: snippet.name }) }}
            </span>
            <div class="snippet-confirm-actions">
              <UButton
                color="error"
                variant="soft"
                size="xs"
                :label="t('snippets.deleteAction')"
                @click="confirmDelete(snippet.id)"
              />
              <UButton
                color="neutral"
                variant="ghost"
                size="xs"
                :label="t('common.cancel')"
                @click="cancelDelete"
              />
            </div>
          </template>

          <template v-else>
            <button
              type="button"
              class="snippet-item-main"
              :disabled="!canSend"
              :title="t('snippets.insertHint')"
              @click="insertSnippet(snippet)"
            >
              <span class="snippet-item-name">{{ snippet.name }}</span>
              <code class="snippet-item-command">{{ snippet.command }}</code>
            </button>
            <div class="snippet-item-actions">
              <UTooltip :text="t('snippets.run')">
                <UButton
                  color="primary"
                  variant="ghost"
                  size="xs"
                  square
                  icon="i-lucide-play"
                  :disabled="!canSend"
                  @click="runSnippet(snippet)"
                />
              </UTooltip>
              <UTooltip :text="t('snippets.edit')">
                <UButton
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  square
                  icon="i-lucide-pencil"
                  @click="startEdit(snippet)"
                />
              </UTooltip>
              <UTooltip :text="t('snippets.deleteAction')">
                <UButton
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  square
                  icon="i-lucide-trash-2"
                  @click="requestDelete(snippet.id)"
                />
              </UTooltip>
            </div>
          </template>
        </div>
      </div>

      <footer class="snippet-foot">
        <UButton
          color="neutral"
          variant="soft"
          size="sm"
          block
          icon="i-lucide-plus"
          :label="t('snippets.create')"
          @click="startCreate"
        />
      </footer>
    </div>

    <form v-else class="snippet-body snippet-form" @submit.prevent="saveSnippet">
      <UFormField :label="t('snippets.nameLabel')" required>
        <UInput
          ref="nameInput"
          v-model="form.name"
          class="w-full"
          size="lg"
          :placeholder="t('snippets.namePlaceholder')"
          spellcheck="false"
          autocomplete="off"
        />
      </UFormField>

      <UFormField :label="t('snippets.commandLabel')" required>
        <UTextarea
          v-model="form.command"
          class="w-full snippet-command-input"
          size="lg"
          autoresize
          :maxrows="8"
          :rows="3"
          :placeholder="t('snippets.commandPlaceholder')"
          spellcheck="false"
          autocomplete="off"
        />
      </UFormField>

      <p v-if="formError" class="snippet-form-error">{{ formError }}</p>

      <div class="snippet-form-actions">
        <UButton
          color="neutral"
          variant="outline"
          size="lg"
          :label="t('common.cancel')"
          @click="cancelEdit"
        />
        <UButton
          type="submit"
          color="primary"
          size="lg"
          class="cta-button"
          :loading="isSaving"
          :label="t('snippets.save')"
        />
      </div>
    </form>
  </aside>
</template>

<style scoped src="../../assets/css/snippet-panel.css"></style>
