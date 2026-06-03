import { invoke } from "@tauri-apps/api/core";

export function archiveCredentialStore() {
  return invoke("archive_credential_store");
}

export function getAppStatus() {
  return invoke("get_app_status");
}

export function getDeviceVaultPassword() {
  return invoke("get_device_vault_password");
}

export function initWorkspace(workspaceDir) {
  return invoke("init_workspace", { workspaceDir });
}

export function loadWorkspace() {
  return invoke("load_workspace");
}

export function changeWorkspaceDir(workspaceDir) {
  return invoke("change_workspace_dir", { workspaceDir });
}

export function saveProfile(profile) {
  return invoke("save_profile", { profile });
}

export function deleteProfile(profileId) {
  return invoke("delete_profile", { profileId });
}

export function saveGroup(group) {
  return invoke("save_group", { group });
}

export function deleteGroup(groupId) {
  return invoke("delete_group", { groupId });
}

export function saveSettings(settings) {
  return invoke("save_settings", { settings });
}

export function testConnection(request) {
  return invoke("test_connection", { request });
}

export function startTerminalSession(request) {
  return invoke("start_terminal_session", { request });
}

export function sendTerminalInput(request) {
  return invoke("send_terminal_input", { request });
}

export function resizeTerminal(request) {
  return invoke("resize_terminal", { request });
}

export function closeTerminalSession(sessionId) {
  return invoke("close_terminal_session", { sessionId });
}

export function listRemoteDirectory(request) {
  return invoke("list_remote_directory", { request });
}

export function startUploadTransfer(request) {
  return invoke("start_upload_transfer", { request });
}

export function startDownloadTransfer(request) {
  return invoke("start_download_transfer", { request });
}
