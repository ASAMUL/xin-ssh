import { ref, watch } from "vue";
import { resolveTerminalBackgroundUrl } from "../services/backgroundApi";

// 把工作区 backgrounds/ 里的文件名解析成可以塞进 CSS url() 的地址（背景图字节走 IPC 转 Blob URL，
// backgroundApi 按文件名缓存）。换图、清除只是换文件名，这里跟着解析；
// 迟到的旧结果不能覆盖新选的图，所以每次解析前都登记一次作废。
export function useBackgroundUrl(getFile) {
  const url = ref(null);

  watch(
    getFile,
    async (file, _previous, onCleanup) => {
      let stale = false;
      onCleanup(() => { stale = true; });
      if (!file) {
        url.value = null;
        return;
      }

      try {
        const resolved = await resolveTerminalBackgroundUrl(file);
        if (!stale) url.value = resolved;
      } catch (error) {
        console.warn("[Xin SSH] background image load failed", error);
        if (!stale) url.value = null;
      }
    },
    { immediate: true },
  );

  return url;
}
