import { useSortableList } from "./useSortableList";

// 将 SortableJS 的临时 DOM 操作收在一处，避免各列表与 Vue 的节点顺序失去同步。
export function useHomeSortable(target, options) {
  useSortableList(target, {
    ghostClass: "home-sort-ghost",
    chosenClass: "home-sort-chosen",
    filter: '[data-sort-locked="true"]',
    preventOnFilter: false,
    onMove: ({ related, willInsertAfter }) =>
      related.dataset.sortLocked !== "true" || willInsertAfter,
    ...options,
  });
}
