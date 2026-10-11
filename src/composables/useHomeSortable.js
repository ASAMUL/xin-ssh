import { useSortableList } from "./useSortableList";

// 首页整张卡/整行都能按住拖，而它们本身又是「点一下就连接」的按钮。
// 松手那次 click 仍会落到被拖的卡上，必须吞掉，否则拖完顺手就连上了。
export function useHomeSortable(target, { onEnd, ...options }) {
  useSortableList(target, {
    ghostClass: "home-sort-ghost",
    chosenClass: "home-sort-chosen",
    fallbackClass: "home-sort-fallback",
    ...options,
    onEnd() {
      swallowNextClick();
      onEnd?.();
    },
  });
}

function swallowNextClick() {
  const swallow = (event) => {
    event.preventDefault();
    event.stopPropagation();
  };
  window.addEventListener("click", swallow, { capture: true, once: true });
  // click 与 mouseup 同属一次输入派发；这一轮没等来 click（松手时已不在原卡上）就撤掉，免得误吞下一次正常点击
  setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
}
