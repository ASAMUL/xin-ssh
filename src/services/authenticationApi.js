import { invoke } from "@tauri-apps/api/core";

export function respondAuthentication(challengeId, answers) {
  return invoke("respond_authentication", { request: { challengeId, answers } });
}
