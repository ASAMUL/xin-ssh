<script setup>
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps({
  // SearchAddon 实例（父组件创建并 loadAddon 后传入，非响应式）
  searchAddon: {
    type: Object,
    default: null,
  },
});

const emit = defineEmits(["close"]);

const { t } = useI18n();

const MATCH_LIMIT = 999;

// 高亮色必须是 #RRGGBB：普通匹配深蓝灰、活跃匹配品牌蓝，深浅主题下白色前景都可读
const SEARCH_DECORATIONS = {
  matchBackground: "#3a4f7a",
  activeMatchBackground: "#155eef",
  matchOverviewRuler: "#3a4f7a",
  activeMatchColorOverviewRuler: "#155eef",
};

const searchInput = ref(null);
const searchText = ref("");
const caseSensitive = ref(false);
const wholeWord = ref(false);
const useRegex = ref(false);
const matchState = ref({ total: 0, current: 0, overflow: false });

let disposeResultsListener = null;

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
  const totalLabel = overflow || total > MATCH_LIMIT ? `${MATCH_LIMIT}+` : String(total);
  return current ? `${current}/${totalLabel}` : totalLabel;
});

watch(
  () => props.searchAddon,
  (addon) => {
    disposeResultsListener?.dispose();
    disposeResultsListener = null;
    if (!addon) return;

    disposeResultsListener = addon.onDidChangeResults(({ resultIndex, resultCount }) => {
      matchState.value = {
        total: resultCount,
        current: Math.max(0, resultIndex + 1),
        // resultIndex === -1 且仍有结果 = 超出高亮上限，计数不再精确
        overflow: resultIndex === -1 && resultCount > 0,
      };
    });
  },
  { immediate: true },
);

// 每次搜索都是 scrollback 全扫 + 重建全部匹配装饰，逐键触发会卡输入并让视口连续跳转；
// 输入停顿 150ms 后再执行。开关切换是单次操作，立即执行
const SEARCH_INPUT_DEBOUNCE_MS = 150;
let searchDebounceTimer = null;

function cancelPendingSearch() {
  clearTimeout(searchDebounceTimer);
  searchDebounceTimer = null;
}

watch(searchText, () => {
  clearTimeout(searchDebounceTimer);
  searchDebounceTimer = setTimeout(runSearch, SEARCH_INPUT_DEBOUNCE_MS);
});
watch([caseSensitive, wholeWord, useRegex], () => runSearch());

onBeforeUnmount(() => {
  cancelPendingSearch();
  disposeResultsListener?.dispose();
});

function searchOptions() {
  return {
    regex: useRegex.value && !regexInvalid.value,
    caseSensitive: caseSensitive.value,
    wholeWord: wholeWord.value,
    decorations: SEARCH_DECORATIONS,
  };
}

function runSearch() {
  cancelPendingSearch();
  const addon = props.searchAddon;
  if (!addon) return;

  if (!hasQuery.value || regexInvalid.value) {
    addon.clearDecorations();
    matchState.value = { total: 0, current: 0, overflow: false };
    return;
  }

  // findPrevious：无选区时从底部向上找“最近”的匹配（用户找的通常是刚滚过去的输出），
  // 词条变长时先尝试就地扩展选区；findNext 会从 scrollback 最顶部开始，一搜就跳到最老的输出
  addon.findPrevious(searchText.value, searchOptions());
}

function goNext() {
  cancelPendingSearch();
  if (!hasQuery.value || regexInvalid.value) return;

  props.searchAddon?.findNext(searchText.value, searchOptions());
}

function goPrev() {
  cancelPendingSearch();
  if (!hasQuery.value || regexInvalid.value) return;

  props.searchAddon?.findPrevious(searchText.value, searchOptions());
}

function onSearchKeydown(event) {
  if (event.target?.tagName !== "INPUT") return;

  if (event.key === "Enter" || event.key === "F3") {
    event.preventDefault();
    // 打完字立刻回车：先执行挂起的搜索（就地定位最近匹配），之后的回车才逐个向下；
    // 无论走哪条路都要取消挂起计时器，否则它会在导航后再次移动活跃匹配
    if (searchDebounceTimer && !event.shiftKey) {
      runSearch();
      return;
    }
    cancelPendingSearch();
    if (event.shiftKey) goPrev();
    else goNext();
  }
}

// 打开/重新唤起：可用终端选区预填
function activate(seed) {
  if (seed && seed !== searchText.value) {
    searchText.value = seed;
  } else if (searchText.value) {
    runSearch();
  }
  focusField();
}

function focusField() {
  searchInput.value?.inputRef?.focus();
  searchInput.value?.inputRef?.select();
}

// 会话切换后由父组件调用：在新会话的内容上重算匹配（不抢焦点）
function refresh() {
  runSearch();
}

defineExpose({ activate, refresh });
</script>

<template>
  <div class="terminal-search-panel" role="search" @keydown.esc.prevent="emit('close')">
    <UInput
      ref="searchInput"
      v-model="searchText"
      class="terminal-search-input"
      :color="regexInvalid ? 'error' : 'neutral'"
      :highlight="regexInvalid"
      variant="outline"
      size="sm"
      :placeholder="t('terminal.search.placeholder')"
      :aria-label="t('terminal.search.placeholder')"
      spellcheck="false"
      autocomplete="off"
      :ui="{ base: 'pe-21', trailing: 'pe-1' }"
      @keydown="onSearchKeydown"
    >
      <template #trailing>
        <div class="terminal-search-flags">
          <UTooltip :text="t('editor.search.matchCase')">
            <UButton
              class="terminal-search-flag"
              :class="{ active: caseSensitive }"
              color="neutral"
              variant="ghost"
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
              class="terminal-search-flag"
              :class="{ active: wholeWord }"
              color="neutral"
              variant="ghost"
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
              class="terminal-search-flag"
              :class="{ active: useRegex }"
              color="neutral"
              variant="ghost"
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

    <span class="terminal-search-counter" :class="{ danger: counterDanger }">
      {{ counterText }}
    </span>

    <USeparator class="terminal-search-divider" orientation="vertical" />

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
</template>

<style scoped src="../../assets/css/terminal-search-panel.css"></style>
