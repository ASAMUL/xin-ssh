import { onBeforeUnmount, onMounted, toValue } from "vue";
import { useSessionStore } from "../stores/sessionStore";

const OVERLAY_SELECTOR = '.modal-backdrop, [role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]';

// 必须在窗口捕获阶段截住应用保留键，晚于 xterm 的按键处理会把 Ctrl+W 发给远端。
export function useSessionTabShortcuts({ disabled }) {
  const sessionStore = useSessionStore();
  const platform = navigator.userAgentData?.platform ?? navigator.platform ?? "";
  const isMac = platform.toLowerCase().includes("mac");

  function handleKeydown(event) {
    if (event.defaultPrevented || event.isComposing || event.altKey) return;
    const control = event.ctrlKey && !event.metaKey;
    const command = isMac && event.metaKey && !event.ctrlKey;
    const key = event.key.toLowerCase();
    const cycle = control && key === "tab";
    const close = (control || command) && !event.shiftKey && key === "w";
    const number = (control || command) && !event.shiftKey && /^[1-9]$/.test(key)
      ? Number(key) : null;
    if (!cycle && !close && number === null) return;

    // 弹窗或普通输入框中不操作后台标签，但仍阻止 WebView 把关闭键当作关窗命令。
    event.preventDefault();
    event.stopImmediatePropagation();
    if (toValue(disabled) || isEditingText(event.target) || hasOpenOverlay()) return;
    if (cycle) sessionStore.selectAdjacentSession(event.shiftKey ? -1 : 1);
    else if (number !== null) sessionStore.selectSessionByNumber(number);
    else if (!event.repeat) sessionStore.closeSession(sessionStore.activeSessionId);
  }

  onMounted(() => window.addEventListener("keydown", handleKeydown, true));
  onBeforeUnmount(() => window.removeEventListener("keydown", handleKeydown, true));
}

function isEditingText(target) {
  if (!(target instanceof Element)) return false;
  if (target.matches(".xterm-helper-textarea")) return false;
  return Boolean(target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"]'));
}

function hasOpenOverlay() {
  // 同时兼容旧表单遮罩和 Nuxt UI 的 portal；已隐藏或关闭动画中的菜单不接管快捷键。
  return [...document.querySelectorAll(OVERLAY_SELECTOR)].some((element) =>
    element.dataset.state !== "closed" && !element.closest("[inert]") && element.getClientRects().length > 0,
  );
}
