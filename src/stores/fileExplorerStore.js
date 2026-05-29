import { defineStore } from "pinia";

export const useFileExplorerStore = defineStore("fileExplorer", {
  state: () => ({
    entries: [],
    currentPath: "/",
    isLoading: false,
  }),
});
