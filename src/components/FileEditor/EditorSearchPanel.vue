<script setup>
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  SearchQuery,
  findNext,
  findPrevious,
  replaceAll,
  replaceNext,
  setSearchQuery,
} from "@codemirror/search";
import { EditorSelection } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

const props = defineProps({
  // EditorView 实例（shallowRef 传入，不能被深度代理）
  view: {
    type: Object,
    required: true,
  },
  // 父组件在文档/选区变化时自增，驱动匹配计数刷新
  tick: {
    type: Number,
    default: 0,
  },
});

const emit = defineEmits(["close"]);

const { t } = useI18n();

const MATCH_LIMIT = 999;

const searchInput = ref(null);
const searchText = ref("");
const replaceText = ref("");
const caseSensitive = ref(false);
const wholeWord = ref(false);
const useRegex = ref(false);
const showReplace = ref(false);
const matchState = ref({ total: 0, current: 0, overflow: false });

let recountScheduled = false;
let disposed = false;

const regexInvalid = computed(() => {
  if (!useRegex.value || !searchText.value) return false;

  try {
    new RegExp(searchText.value);
    return false;
  } catch {
    return true;
  }
});

const hasQuery = computed(() => searchText.value.length > 0);
const hasMatches = computed(() => matchState.value.total > 0);
const counterDanger = computed(() => hasQuery.value && (regexInvalid.value || !hasMatches.value));

const counterText = computed(() => {
  if (!hasQuery.value) return "";
  if (regexInvalid.value) return t("editor.search.invalidRegex");
  if (!hasMatches.value) return t("editor.search.noResults");

  const { total, current, overflow } = matchState.value;
  const totalLabel = overflow ? `${MATCH_LIMIT}+` : String(total);
  return current ? `${current}/${totalLabel}` : totalLabel;
});

const counterTitle = computed(() => {
  const { total, current, overflow } = matchState.value;
  if (!current) return "";

  return t("editor.search.matchPosition", {
    current,
    total: overflow ? `${MATCH_LIMIT}+` : total,
  });
});

watch([searchText, caseSensitive, wholeWord, useRegex], () => syncQuery({ jump: true }));
watch(replaceText, () => syncQuery({ jump: false }));
watch(() => props.tick, scheduleRecount);

onBeforeUnmount(() => {
  disposed = true;
});

function buildQuery() {
  return new SearchQuery({
    search: searchText.value,
    caseSensitive: caseSensitive.value,
    wholeWord: wholeWord.value,
    regexp: useRegex.value && !regexInvalid.value,
    replace: replaceText.value,
  });
}

// 把面板输入同步进 CodeMirror 的搜索状态（驱动全文高亮与 find/replace 命令）
function syncQuery({ jump }) {
  const query = regexInvalid.value ? new SearchQuery({ search: "" }) : buildQuery();
  props.view.dispatch({ effects: setSearchQuery.of(query) });

  if (jump && query.valid) jumpToNearest(query);
  recount();
}

// 输入时就近跳转：从当前选区起点（含）找，找不到再从头绕回，贴近 VS Code 手感
function jumpToNearest(query) {
  const { state } = props.view;
  const from = state.selection.main.from;
  let match = firstMatchFrom(query, state, from);
  if (!match && from > 0) match = firstMatchFrom(query, state, 0);
  if (!match) return;

  props.view.dispatch({
    selection: EditorSelection.single(match.from, match.to),
    effects: EditorView.scrollIntoView(EditorSelection.range(match.from, match.to), {
      y: "center",
    }),
    userEvent: "select.search",
  });
}

function firstMatchFrom(query, state, from) {
  const step = query.getCursor(state, from).next();
  return step.done ? null : step.value;
}

// 选区/文档高频变化时按帧合并计数，避免大文件下逐键全文扫描
function scheduleRecount() {
  if (recountScheduled) return;

  recountScheduled = true;
  requestAnimationFrame(() => {
    recountScheduled = false;
    if (!disposed) recount();
  });
}

function recount() {
  if (!hasQuery.value || regexInvalid.value) {
    matchState.value = { total: 0, current: 0, overflow: false };
    return;
  }

  const { state } = props.view;
  const selection = state.selection.main;
  const iterator = buildQuery().getCursor(state);
  let total = 0;
  let current = 0;

  while (total < MATCH_LIMIT) {
    const step = iterator.next();
    if (step.done) {
      matchState.value = { total, current, overflow: false };
      return;
    }
    total += 1;
    if (step.value.from === selection.from && step.value.to === selection.to) current = total;
  }

  matchState.value = { total, current, overflow: true };
}

function goNext() {
  if (hasMatches.value) findNext(props.view);
}

function goPrev() {
  if (hasMatches.value) findPrevious(props.view);
}

function doReplace() {
  if (hasMatches.value) replaceNext(props.view);
}

function doReplaceAll() {
  if (hasMatches.value) replaceAll(props.view);
}

function onSearchKeydown(event) {
  // 事件从 UInput 根元素透传，#trailing 里开关按钮的按键不做拦截
  if (event.target?.tagName !== "INPUT") return;

  if (event.key === "Enter" || event.key === "F3") {
    event.preventDefault();
    if (event.shiftKey) goPrev();
    else goNext();
  } else if ((event.ctrlKey || event.metaKey) && event.key?.toLowerCase() === "f") {
    event.preventDefault();
    focusField();
  }
}

function onReplaceKeydown(event) {
  if (event.key === "Enter") {
    event.preventDefault();
    doReplace();
  }
}

// 打开/重新唤起面板：可选地用编辑器选区预填，并让查询状态与面板一致
function activate(seed) {
  if (seed && seed !== searchText.value) {
    searchText.value = seed;
  } else {
    syncQuery({ jump: false });
  }
  focusField();
}

function focusField() {
  searchInput.value?.inputRef?.focus();
  searchInput.value?.inputRef?.select();
}

defineExpose({ activate });
</script>

<template>
  <div class="editor-search-panel" role="search" @keydown.esc.prevent="emit('close')">
    <UTooltip :text="t('editor.search.toggleReplace')">
      <UButton
        class="editor-search-expand"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        :icon="showReplace ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
        :aria-expanded="showReplace"
        @mousedown.prevent
        @click="showReplace = !showReplace"
      />
    </UTooltip>

    <div class="editor-search-rows">
      <div class="editor-search-row">
        <UInput
          ref="searchInput"
          v-model="searchText"
          class="editor-search-input"
          :color="regexInvalid ? 'error' : 'neutral'"
          :highlight="regexInvalid"
          variant="outline"
          size="sm"
          :placeholder="t('editor.search.findPlaceholder')"
          :aria-label="t('editor.search.findPlaceholder')"
          spellcheck="false"
          autocomplete="off"
          :ui="{ base: 'pe-21', trailing: 'pe-1' }"
          @keydown="onSearchKeydown"
        >
          <template #trailing>
            <div class="editor-search-flags">
              <UTooltip :text="t('editor.search.matchCase')">
                <UButton
                  :color="caseSensitive ? 'primary' : 'neutral'"
                  :variant="caseSensitive ? 'soft' : 'ghost'"
                  size="xs"
                  square
                  icon="i-lucide-case-sensitive"
                  :aria-pressed="caseSensitive"
                  @mousedown.prevent
                  @click="caseSensitive = !caseSensitive"
                />
              </UTooltip>
              <UTooltip :text="t('editor.search.wholeWord')">
                <UButton
                  :color="wholeWord ? 'primary' : 'neutral'"
                  :variant="wholeWord ? 'soft' : 'ghost'"
                  size="xs"
                  square
                  icon="i-lucide-whole-word"
                  :aria-pressed="wholeWord"
                  @mousedown.prevent
                  @click="wholeWord = !wholeWord"
                />
              </UTooltip>
              <UTooltip :text="t('editor.search.regex')">
                <UButton
                  :color="useRegex ? 'primary' : 'neutral'"
                  :variant="useRegex ? 'soft' : 'ghost'"
                  size="xs"
                  square
                  icon="i-lucide-regex"
                  :aria-pressed="useRegex"
                  @mousedown.prevent
                  @click="useRegex = !useRegex"
                />
              </UTooltip>
            </div>
          </template>
        </UInput>

        <span class="editor-search-counter" :class="{ danger: counterDanger }" :title="counterTitle">
          {{ counterText }}
        </span>

        <USeparator class="editor-search-divider" orientation="vertical" />

        <UTooltip :text="t('editor.search.previous')" :kbds="['shift', 'enter']">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-arrow-up"
            :disabled="!hasMatches"
            @mousedown.prevent
            @click="goPrev"
          />
        </UTooltip>
        <UTooltip :text="t('editor.search.next')" :kbds="['enter']">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-arrow-down"
            :disabled="!hasMatches"
            @mousedown.prevent
            @click="goNext"
          />
        </UTooltip>
        <UTooltip :text="t('editor.search.close')" :kbds="['escape']">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-x"
            @click="emit('close')"
          />
        </UTooltip>
      </div>

      <div v-show="showReplace" class="editor-search-row">
        <UInput
          v-model="replaceText"
          class="editor-search-input"
          color="neutral"
          variant="outline"
          size="sm"
          :placeholder="t('editor.search.replacePlaceholder')"
          :aria-label="t('editor.search.replacePlaceholder')"
          spellcheck="false"
          autocomplete="off"
          @keydown="onReplaceKeydown"
        />
        <UTooltip :text="t('editor.search.replaceOne')" :kbds="['enter']">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-replace"
            :disabled="!hasMatches"
            @mousedown.prevent
            @click="doReplace"
          />
        </UTooltip>
        <UTooltip :text="t('editor.search.replaceAllAction')">
          <UButton
            color="neutral"
            variant="ghost"
            size="sm"
            square
            icon="i-lucide-replace-all"
            :disabled="!hasMatches"
            @mousedown.prevent
            @click="doReplaceAll"
          />
        </UTooltip>
      </div>
    </div>
  </div>
</template>

<style scoped src="../../assets/css/editor-search-panel.css"></style>
