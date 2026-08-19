import { defineStore } from "pinia";
import { deleteSnippet, listSnippets, saveSnippet } from "../services/workspaceApi";

export const useSnippetStore = defineStore("snippets", {
  state: () => ({
    snippets: [],
    loaded: false,
  }),
  getters: {
    activeSnippets: (state) =>
      state.snippets
        .filter((snippet) => !snippet.deletedAt)
        .sort((left, right) => left.sortOrder - right.sortOrder),
  },
  actions: {
    // 面板首次打开时按需加载；浏览器 dev 无后端时静默
    async loadSnippets() {
      try {
        this.snippets = await listSnippets();
        this.loaded = true;
      } catch (error) {
        console.warn("[Xin Terminal] snippets unavailable", error);
      }
    },
    async saveSnippet(snippet) {
      this.snippets = await saveSnippet(snippet);
    },
    async removeSnippet(snippetId) {
      this.snippets = await deleteSnippet(snippetId);
    },
  },
});
