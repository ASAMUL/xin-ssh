// 可编辑的地方留着 WebView 自带菜单：剪切 / 复制 / 粘贴在那里有用
export const EDITABLE_TARGETS = "input, textarea, select, [contenteditable]:not([contenteditable='false'])";

// WebView 自带的右键菜单里有「刷新」「返回」「打印」，误点「刷新」整个前端重载，所有会话跟着丢。
// 其余地方要么由 UContextMenu 接管，要么什么都不弹。
// 挂在 window 的冒泡阶段：Reka 的菜单触发器要等一个微任务才 preventDefault，而浏览器在相邻两个监听器之间
// 会清空微任务，轮到这里时菜单开没开已经看得出来
export function suppressNativeContextMenu() {
  window.addEventListener("contextmenu", (event) => {
    if (event.defaultPrevented) return;
    if (event.target instanceof Element && event.target.closest(EDITABLE_TARGETS)) return;
    event.preventDefault();
  });
}
