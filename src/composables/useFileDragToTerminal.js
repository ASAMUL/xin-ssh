import { onBeforeUnmount, reactive } from "vue";

// 按下后挪过这么远才算拖拽：手抖一下仍是单击 / 双击
const DRAG_THRESHOLD_PX = 6;

// 坐标下面是不是终端。同一时刻只有当前会话的终端可见，命中的就是它
export function terminalDropTargetAt(x, y) {
  return document.elementFromPoint(x, y)?.closest("[data-terminal-drop]") ?? null;
}

let highlightedTerminal = null;

// 给终端描上「松手会怎样」的提示（样式见 terminal-viewport.css 的 .drop-target）；传 null 清掉
export function highlightTerminalDrop(target, hint = "") {
  if (highlightedTerminal && highlightedTerminal !== target) {
    highlightedTerminal.classList.remove("drop-target");
    delete highlightedTerminal.dataset.dropHint;
  }
  highlightedTerminal = target;
  if (!target) return;
  target.classList.add("drop-target");
  target.dataset.dropHint = hint;
}

// 文件面板里的行拖到终端上 = 把远程路径插到命令行。
// Tauri 在 Windows 上接管了原生拖放（为了接住从桌面拖进来的文件），网页自己的 HTML5 拖拽用不了，
// 所以和标签排序（SortableJS forceFallback）一样，用指针事件自己跟：跟手的小标签 + 松手时看指针下面是不是终端。
//   resolve(entry) → { paths, label, icon, hint }：按下的那一行（多选时是整组）要插哪些路径、标签怎么写
export function useFileDragToTerminal({ resolve, onDrop }) {
  const chip = reactive({ visible: false, x: 0, y: 0, label: "", icon: "" });
  let pressed = null;
  let dragging = null;

  function start(entry, event) {
    if (event.button !== 0 || event.pointerType === "touch") return;
    pressed = { entry, x: event.clientX, y: event.clientY };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("keydown", cancelOnEscape, true);
  }

  function move(event) {
    if (!pressed) return;
    if (!dragging) {
      if (Math.hypot(event.clientX - pressed.x, event.clientY - pressed.y) < DRAG_THRESHOLD_PX) return;
      dragging = resolve(pressed.entry);
      if (!dragging?.paths.length) {
        cancel();
        return;
      }
      chip.label = dragging.label;
      chip.icon = dragging.icon;
      chip.visible = true;
      document.body.classList.add("xin-dragging");
    }
    chip.x = event.clientX;
    chip.y = event.clientY;
    highlightTerminalDrop(terminalDropTargetAt(event.clientX, event.clientY), dragging.hint);
  }

  function finish(event) {
    const target = dragging ? terminalDropTargetAt(event.clientX, event.clientY) : null;
    const paths = dragging?.paths;
    cancel();
    if (target && paths) onDrop(paths);
  }

  function cancelOnEscape(event) {
    if (event.key !== "Escape" || !dragging) return;
    event.preventDefault();
    event.stopPropagation();
    cancel();
  }

  function cancel() {
    pressed = null;
    dragging = null;
    chip.visible = false;
    document.body.classList.remove("xin-dragging");
    highlightTerminalDrop(null);
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", finish);
    window.removeEventListener("pointercancel", cancel);
    window.removeEventListener("keydown", cancelOnEscape, true);
  }

  onBeforeUnmount(cancel);

  return { chip, start };
}
