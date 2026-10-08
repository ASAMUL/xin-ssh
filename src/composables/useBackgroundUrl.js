import { ref, watch } from "vue";
import { resolveTerminalBackgroundUrl } from "../services/backgroundApi";

// 把工作区 backgrounds/ 里的文件名解析成可以塞进 CSS url() 的地址（背景图字节走 IPC 转 Blob URL，
// backgroundApi 按文件名缓存）。换图、清除只是换文件名，这里跟着解析；
// 迟到的旧结果不能覆盖新选的图，所以每次解析前都登记一次作废。
// getFallback 给没有文件、或文件读不出来（同步工作区时漏了图）时用的地址，应用壁纸靠它退回内置图。
export function useBackgroundUrl(getFile, getFallback = () => null) {
  const url = ref(null);

  watch(
    [getFile, getFallback],
    async ([file, fallback], _previous, onCleanup) => {
      let stale = false;
      onCleanup(() => { stale = true; });
      if (!file) {
        url.value = fallback;
        return;
      }

      try {
        const resolved = await resolveTerminalBackgroundUrl(file);
        if (!stale) url.value = resolved;
      } catch (error) {
        console.warn("[Xin SSH] background image load failed", error);
        if (!stale) url.value = fallback;
      }
    },
    { immediate: true },
  );

  return url;
}
