import { computed } from "vue";
import { useAppStore } from "../stores/appStore";
import { useAuthenticationStore } from "../stores/authenticationStore";
import { useEditorStore } from "../stores/editorStore";
import { useUiStore } from "../stores/uiStore";

// 弹窗、认证挑战、文件编辑器打开期间冻结「连接 / 工作区」切换和会话标签：
// 这些浮层都绑定在当前会话上，背后的会话被切走后它们的上下文就错位了。
export function useWorkspaceNavigationLock() {
  const appStore = useAppStore();
  const authenticationStore = useAuthenticationStore();
  const editorStore = useEditorStore();
  const uiStore = useUiStore();

  return computed(() => appStore.isLoading || uiStore.isConnectionFormOpen
    || uiStore.isSettingsOpen || uiStore.isTunnelPanelOpen || uiStore.isWindowCloseOpen
    || editorStore.isOpen || Boolean(authenticationStore.activeChallenge));
}
