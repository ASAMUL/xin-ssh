<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { basicSetup } from "codemirror";
import { EditorView, keymap } from "@codemirror/view";
import { Compartment, EditorState } from "@codemirror/state";
import { indentWithTab } from "@codemirror/commands";
import { LanguageDescription } from "@codemirror/language";
import { languages } from "@codemirror/language-data";

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

const emit = defineEmits(["change", "save"]);

const host = ref(null);
const languageCompartment = new Compartment();
let view = null;

onMounted(() => {
  view = new EditorView({
    parent: host.value,
    state: createEditorState(props.content),
  });
  view.focus();
  loadLanguage(props.filename);
});

onBeforeUnmount(() => {
  view?.destroy();
  view = null;
});

// 弹窗保持挂载时切换文件：整体重建 state，同时重置撤销历史
watch(
  () => props.content,
  (content) => {
    if (!view || view.state.doc.toString() === content) return;

    view.setState(createEditorState(content));
    loadLanguage(props.filename);
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
        indentWithTab,
      ]),
      basicSetup,
      languageCompartment.of([]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) emit("change");
      }),
    ],
  });
}

// 语言包按需懒加载（@codemirror/language-data 内部是动态 import，vite 自动分包）
async function loadLanguage(filename) {
  if (!view || !filename) return;

  const description = LanguageDescription.matchFilename(languages, filename);
  if (!description) return;

  try {
    const support = await description.load();
    view?.dispatch({ effects: languageCompartment.reconfigure(support) });
  } catch (error) {
    console.warn("[Xin Terminal] editor language load failed", error);
  }
}

defineExpose({
  getContent: () => view?.state.doc.toString() ?? null,
  focus: () => view?.focus(),
});
</script>

<template>
  <div ref="host" class="code-editor-host"></div>
</template>
