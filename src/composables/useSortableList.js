import { onBeforeUnmount, toValue, watch } from "vue";
import Sortable from "sortablejs";

// SortableJS 只负责拖动反馈，提交前还原节点位置，让 Vue 始终按自己的数据顺序更新 DOM。
export function useSortableList(target, { disabled, onDrop, onStart, onEnd, ...options }) {
  let sortable = null;
  let originalNextSibling = null;

  function destroy() {
    sortable?.destroy();
    sortable = null;
    originalNextSibling = null;
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
        ...options,
        onChoose: ({ item }) => { originalNextSibling = item.nextSibling; },
        onStart,
        onEnd(event) {
          const { item, from, to, oldIndex, newIndex } = event;
          const order = [...to.children]
            .filter((child) => child.matches(options.draggable))
            .map((child) => child.dataset.sortId);
          const beforeId = order[order.indexOf(item.dataset.sortId) + 1] ?? null;
          item.remove();
          const anchor = originalNextSibling?.parentNode === from
            ? originalNextSibling : from.children[oldIndex] ?? null;
          from.insertBefore(item, anchor);
          originalNextSibling = null;
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
