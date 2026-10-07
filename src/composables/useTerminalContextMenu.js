import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { EDITABLE_TARGETS } from "../utils/nativeContextMenu";

// 终端画面的右键菜单。activeTerminal 是当前会话的 TerminalViewport；
// 查找、片段、清空由工作区传进来，和工具栏走同一个入口
export function useTerminalContextMenu(activeTerminal, actions) {
  const { t } = useI18n();
  // xterm 的选区不是响应式的，只能在菜单打开时读一次
  const snapshot = ref({ hasSelection: false, canPaste: false });

  const items = computed(() => [
    [
      {
        label: t("terminal.menu.copy"),
        icon: "i-lucide-copy",
        kbds: ["ctrl", "c"],
        disabled: !snapshot.value.hasSelection,
        onSelect: () => activeTerminal.value?.copySelection(),
      },
      {
        label: t("terminal.menu.paste"),
        icon: "i-lucide-clipboard-paste",
        kbds: ["ctrl", "v"],
        disabled: !snapshot.value.canPaste,
        onSelect: paste,
      },
      {
        label: t("terminal.menu.selectAll"),
        icon: "i-lucide-text-select",
        onSelect: () => activeTerminal.value?.selectAll(),
      },
    ],
    [
      {
        label: t("terminal.menu.find"),
        icon: "i-lucide-search",
        kbds: ["ctrl", "shift", "f"],
        onSelect: actions.find,
      },
      {
        label: t("terminal.menu.snippets"),
        icon: "i-lucide-square-terminal",
        onSelect: actions.openSnippets,
      },
    ],
    [
      {
        label: t("terminal.clear"),
        icon: "i-lucide-eraser",
        onSelect: actions.clear,
      },
    ],
  ]);

  // 挂在捕获阶段，赶在菜单触发器之前决定这次右键归谁。
  // 只有点在终端画面上才弹终端菜单；xterm 右键时会把隐藏输入框挪到鼠标底下，所以先认 .xterm 再认输入框。
  // 拦掉默认行为后 Reka 不会开菜单，WebView 也不会弹自带的；只停冒泡则留给 WebView 自带菜单
  function routeRightClick(event) {
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest(".xterm")) {
      const terminal = activeTerminal.value;
      if (!terminal || terminal.capturesRightClick(event)) event.preventDefault();
      return;
    }
    if (target?.closest(EDITABLE_TARGETS)) event.stopPropagation();
    else event.preventDefault();
  }

  // 等菜单真打开再读选区：Mac 上 xterm 右键会先选中光标下的单词，那一步比捕获阶段晚
  function syncState(open) {
    const terminal = activeTerminal.value;
    if (!open || !terminal) return;
    snapshot.value = { hasSelection: terminal.hasSelection(), canPaste: terminal.canSendData };
  }

  // 剪贴板里是图片或者空的时候插件会报错；和快捷键粘贴一样，什么都不做就好
  async function paste() {
    const terminal = activeTerminal.value;
    try {
      terminal?.pasteText(await readText());
    } catch (error) {
      console.warn("[Xin SSH] clipboard has no text to paste", error);
    }
  }

  // Reka 关菜单时会把焦点还给右键前的元素。查找框这时已经拿到焦点，不能被抢回去；
  // 由工作区决定焦点去向，终端那边打开着查找时自己会让开
  function restoreFocus(event) {
    event.preventDefault();
    actions.focus();
  }

  return { items, routeRightClick, syncState, restoreFocus };
}
