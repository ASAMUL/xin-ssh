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
  // 父组件在文档变化时自增：要重新全文扫描匹配
  docVersion: {
    type: Number,
    default: 0,
  },
  // 父组件在选区变化时自增：只需要重新算「当前是第几个」
  selectionVersion: {
    type: Number,
    default: 0,
  },
});

const emit = defineEmits(["close"]);

const { t } = useI18n();

const MATCH_LIMIT = 999;
// 连续打字时停手这么久才重扫全文：5MB 的文件扫一遍要几十毫秒，逐键扫会卡输入
const RECOUNT_DELAY_MS = 150;

const searchInput = ref(null);
const searchText = ref("");
const replaceText = ref("");
const caseSensitive = ref(false);
const wholeWord = ref(false);
const useRegex = ref(false);
const showReplace = ref(false);
const matchState = ref({ total: 0, current: 0, overflow: false });

// 上一次全文扫描到的匹配（按位置排好，最多 MATCH_LIMIT 个），光标移动时在这里二分找当前是第几个
let matchRanges = [];
let recountTimer = null;
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
watch(() => props.docVersion, scheduleRecount);
watch(() => props.selectionVersion, updateCurrent);

onBeforeUnmount(() => {
  disposed = true;
  clearTimeout(recountTimer);
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

// 文档改了才重扫全文，而且等停手再扫——以前每一帧都扫，大文件开着搜索打字会卡
function scheduleRecount() {
  clearTimeout(recountTimer);
  recountTimer = setTimeout(() => {
    recountTimer = null;
    if (!disposed) recount();
  }, RECOUNT_DELAY_MS);
}

function recount() {
  clearTimeout(recountTimer);
  recountTimer = null;
  if (!hasQuery.value || regexInvalid.value) {
    matchRanges = [];
    matchState.value = { total: 0, current: 0, overflow: false };
    return;
  }

  const iterator = buildQuery().getCursor(props.view.state);
  const ranges = [];
  let overflow = false;
  for (let step = iterator.next(); !step.done; step = iterator.next()) {
    if (ranges.length === MATCH_LIMIT) {
      overflow = true;
      break;
    }
    ranges.push({ from: step.value.from, to: step.value.to });
  }

  matchRanges = ranges;
  matchState.value = { total: ranges.length, current: currentMatchNumber(), overflow };
}

// 光标、选区移动：在上次的结果里二分，不扫全文。
// 文档刚改、重扫还没跑的那一会儿，旧结果的位置已经对不上了，交给重扫去算
function updateCurrent() {
  if (recountTimer || !matchState.value.total) return;
  const current = currentMatchNumber();
  if (current !== matchState.value.current) matchState.value = { ...matchState.value, current };
}

// 选区正好是某个匹配时返回它是第几个（从 1 数），否则 0
function currentMatchNumber() {
  const { from, to } = props.view.state.selection.main;
  let low = 0;
  let high = matchRanges.length - 1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    const range = matchRanges[middle];
    if (range.from < from) low = middle + 1;
    else if (range.from > from) high = middle - 1;
    else return range.to === to ? middle + 1 : 0;
  }
  return 0;
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
