import { onBeforeUnmount, toValue, watch } from "vue";
import Sortable from "sortablejs";

// 将 SortableJS 的临时 DOM 操作收在一处，避免各列表与 Vue 的节点顺序失去同步。
export function useHomeSortable(target, { disabled, onDrop, onStart, onEnd, ...options }) {
  let sortable = null;

  function destroy() {
    sortable?.destroy();
    sortable = null;
  }

  watch([target, () => toValue(disabled)], ([element, isDisabled]) => {
    if (sortable?.el !== element) {
      destroy();
      if (!element) return;
      sortable = Sortable.create(element, {
        animation: 150,
        forceFallback: true,
        fallbackOnBody: true,
        fallbackTolerance: 5,
        ghostClass: "home-sort-ghost",
        chosenClass: "home-sort-chosen",
        filter: '[data-sort-locked="true"]',
        preventOnFilter: false,
        ...options,
        onMove: ({ related, willInsertAfter }) =>
          related.dataset.sortLocked !== "true" || willInsertAfter,
        onStart,
        onEnd(event) {
          const { item, from, to, oldIndex, newIndex } = event;
          const order = [...to.children]
            .filter((child) => child.matches(options.draggable))
            .map((child) => child.dataset.sortId);
          const beforeId = order[order.indexOf(item.dataset.sortId) + 1] ?? null;
          // Vue 仍持有旧的节点顺序，先还原 DOM，再让成功的配置快照驱动渲染。
          item.remove();
          from.insertBefore(item, from.children[oldIndex] ?? null);
          onEnd?.();
          if (from === to && oldIndex === newIndex) return;
          onDrop({ id: item.dataset.sortId, groupId: to.dataset.groupId, beforeId, order });
        },
      });
    }
    sortable?.option("disabled", Boolean(isDisabled));
  }, { flush: "post" });

  onBeforeUnmount(destroy);
}
