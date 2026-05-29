import { defineStore } from "pinia";

export const useSessionStore = defineStore("sessions", {
  state: () => ({
    sessions: [],
    activeSessionId: null,
  }),
});
