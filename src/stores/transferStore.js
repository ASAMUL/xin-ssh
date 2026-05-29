import { defineStore } from "pinia";

export const useTransferStore = defineStore("transfers", {
  state: () => ({
    tasks: [],
  }),
});
