<script setup>
import { nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { basicSetup } from "codemirror";
import { EditorView, keymap } from "@codemirror/view";
import { Compartment, EditorState } from "@codemirror/state";
import { indentWithTab } from "@codemirror/commands";
import { closeSearchPanel, openSearchPanel, search } from "@codemirror/search";
import { HighlightStyle, LanguageDescription, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { tags } from "@lezer/highlight";
import EditorSearchPanel from "./EditorSearchPanel.vue";

const props = defineProps({
  content: {
    type: String,
    default: "",
  },
  filename: {
    type: String,
    default: "",
  },
});

const emit = defineEmits(["change", "save", "language-change", "cursor-change"]);

const host = ref(null);
const searchPanel = ref(null);
const searchOpen = ref(false);
const editorView = shallowRef(null);
const editorTick = ref(0);
const languageCompartment = new Compartment();
let view = null;
let languageLoadToken = 0;
let activeLanguageName = null;

const syntaxHighlightTheme = HighlightStyle.define([
  {
    tag: [tags.keyword, tags.controlKeyword, tags.definitionKeyword, tags.moduleKeyword],
    color: "#c792ea",
  },
  {
    tag: [tags.string, tags.special(tags.string), tags.regexp],
    color: "#c3e88d",
  },
  {
    tag: [tags.number, tags.bool, tags.null, tags.atom],
    color: "#f78c6c",
  },
  {
    tag: [tags.comment, tags.lineComment, tags.blockComment, tags.docComment],
    color: "#7383a3",
    fontStyle: "italic",
  },
  {
    tag: [tags.function(tags.variableName), tags.definition(tags.variableName)],
    color: "#82aaff",
  },
  {
    tag: [tags.typeName, tags.className, tags.namespace],
    color: "#ffcb6b",
  },
  {
    tag: [tags.propertyName, tags.attributeName],
    color: "#89ddff",
  },
  {
    tag: [tags.operator, tags.operatorKeyword, tags.punctuation, tags.bracket],
    color: "#9cb4d8",
  },
  {
    tag: [tags.meta, tags.processingInstruction],
    color: "#f07178",
  },
  {
    tag: tags.invalid,
    color: "#ff9b9b",
    backgroundColor: "rgba(239, 68, 68, 0.2)",
  },
]);

const languageByName = new Map(languages.map((description) => [description.name, description]));
const shellConfigFilenames = new Set([
  ".bash_profile",
  ".bashrc",
  ".envrc",
  ".profile",
  ".zprofile",
  ".zshrc",
]);
const propertiesConfigFilenames = new Set([
  "ssh_config",
  "sshd_config",
  "system.conf",
  "user.conf",
]);
const propertiesConfigExtension = /\.(?:cfg|cnf|conf|config|mount|path|service|slice|socket|swap|target|timer)$/i;

onMounted(() => {
  view = new EditorView({
    parent: host.value,
    state: createEditorState(props.content),
  });
  editorView.value = view;
  view.focus();
  emitCursorPosition(view.state);
  loadLanguage(props.filename);
});

onBeforeUnmount(() => {
  languageLoadToken += 1;
  editorView.value = null;
  view?.destroy();
  view = null;
});

// 弹窗保持挂载时切换文件：整体重建 state，同时重置撤销历史
watch(
  [() => props.content, () => props.filename],
  ([content, filename], [previousContent, previousFilename]) => {
    if (!view) return;

    // 换了文件必须重建（即使内容碰巧相同），否则撤销历史和光标会串文件
    if (
      filename !== previousFilename ||
      (content !== previousContent && view.state.doc.toString() !== content)
    ) {
      searchOpen.value = false;
      view.setState(createEditorState(content));
      activeLanguageName = null;
      emitCursorPosition(view.state);
    }

    if (content !== previousContent || filename !== previousFilename) {
      loadLanguage(filename);
    }
  },
);

function createEditorState(doc) {
  return EditorState.create({
    doc,
    extensions: [
      keymap.of([
        {
          key: "Mod-s",
          run: () => {
            emit("save");
            return true;
          },
        },
        {
          key: "Mod-f",
          run: () => openSearch(),
        },
        {
          key: "Escape",
          run: () => {
            if (!searchOpen.value) return false;
            closeSearch();
            return true;
          },
        },
        indentWithTab,
      ]),
      // 查找/替换 UI 用自定义的 EditorSearchPanel：这里只挂一个隐藏锚点，
      // 让 CodeMirror 维持「面板已打开」的内部状态来驱动全文匹配高亮
      search({
        top: true,
        scrollToMatch: (range) => EditorView.scrollIntoView(range, { y: "center" }),
        createPanel: () => {
          const dom = document.createElement("div");
          dom.className = "cm-xin-search-anchor";
          dom.setAttribute("aria-hidden", "true");
          return { dom, top: true };
        },
      }),
      basicSetup,
      syntaxHighlighting(syntaxHighlightTheme),
      languageCompartment.of([]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) emit("change");
        if (update.docChanged || update.selectionSet) {
          emitCursorPosition(update.state);
          editorTick.value += 1;
        }
      }),
    ],
  });
}

// 打开（或重新聚焦）搜索面板；有单行选区时预填为搜索词
function openSearch() {
  if (!view) return true;

  const selection = view.state.selection.main;
  let seed = "";
  if (!selection.empty && selection.to - selection.from <= 200) {
    const text = view.state.sliceDoc(selection.from, selection.to);
    if (!text.includes("\n")) seed = text;
  }

  openSearchPanel(view);
  searchOpen.value = true;
  nextTick(() => searchPanel.value?.activate(seed));
  return true;
}

function closeSearch() {
  searchOpen.value = false;
  if (view) {
    closeSearchPanel(view);
    view.focus();
  }
}

function resolveLanguageDescription(filename) {
  const basename = String(filename || "").split(/[\\/]/).pop();
  if (!basename) return null;

  const matchedLanguage = LanguageDescription.matchFilename(languages, basename);
  if (matchedLanguage) return matchedLanguage;

  const normalizedName = basename.toLowerCase();
  if (normalizedName === ".env" || normalizedName.startsWith(".env.")) {
    return languageByName.get("Properties files") ?? null;
  }
  if (shellConfigFilenames.has(normalizedName)) {
    return languageByName.get("Shell") ?? null;
  }
  if (
    propertiesConfigFilenames.has(normalizedName) ||
    propertiesConfigExtension.test(normalizedName)
  ) {
    return languageByName.get("Properties files") ?? null;
  }

  return null;
}

function emitCursorPosition(state) {
  const selection = state.selection.main;
  const line = state.doc.lineAt(selection.head);

  emit("cursor-change", {
    line: line.number,
    column: selection.head - line.from + 1,
    selectionLength: selection.to - selection.from,
  });
}

// 语言包按需懒加载（@codemirror/language-data 内部是动态 import，Vite 自动分包）
async function loadLanguage(filename) {
  const loadToken = ++languageLoadToken;
  const description = resolveLanguageDescription(filename);
  emit("language-change", description?.name ?? "");

  // 语言没变就不动 compartment，避免「清空再加载」导致的高亮闪烁
  if (!view || description?.name === activeLanguageName) return;

  if (!description) {
    activeLanguageName = null;
    view.dispatch({ effects: languageCompartment.reconfigure([]) });
    return;
  }

  try {
    const support = await description.load();
    if (!view || loadToken !== languageLoadToken) return;

    activeLanguageName = description.name;
    view.dispatch({ effects: languageCompartment.reconfigure(support) });
  } catch (error) {
    if (!view || loadToken !== languageLoadToken) return;

    activeLanguageName = null;
    view.dispatch({ effects: languageCompartment.reconfigure([]) });
    console.warn("[Xin Terminal] editor language load failed", error);
  }
}

defineExpose({
  getContent: () => view?.state.doc.toString() ?? null,
  focus: () => view?.focus(),
  openSearch,
});
</script>

<template>
  <div class="code-editor-shell">
    <div ref="host" class="code-editor-host"></div>
    <Transition name="editor-search-pop">
      <EditorSearchPanel
        v-if="searchOpen && editorView"
        ref="searchPanel"
        :view="editorView"
        :tick="editorTick"
        @close="closeSearch"
      />
    </Transition>
  </div>
</template>

<style scoped src="../../assets/css/code-editor.css"></style>
