import { invoke } from "@tauri-apps/api/core";

export function getAppStatus() {
  return invoke("get_app_status");
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

export function markProfileConnected(profileId) {
  return invoke("mark_profile_connected", { profileId });
}

export function listTunnelConfigs() {
  return invoke("list_tunnel_configs");
}

export function saveTunnelConfig(tunnel) {
  return invoke("save_tunnel_config", { tunnel });
}

export function deleteTunnelConfig(tunnelId) {
  return invoke("delete_tunnel_config", { tunnelId });
}

export function startTunnel(request) {
  return invoke("start_tunnel", { request });
}

export function stopTunnel(tunnelId, attemptId) {
  return invoke("stop_tunnel", { tunnelId, attemptId });
}

export function listSnippets() {
  return invoke("list_snippets");
}

export function saveSnippet(snippet) {
  return invoke("save_snippet", { snippet });
}

export function deleteSnippet(snippetId) {
  return invoke("delete_snippet", { snippetId });
}

export function saveGroup(group) {
  return invoke("save_group", { group });
}

export function deleteGroup(groupId, deleteProfiles = false) {
  return invoke("delete_group", { groupId, deleteProfiles });
}

export function reorderGroups(groupIds) {
  return invoke("reorder_groups", { groupIds });
}

export function moveProfile(profileId, groupId, beforeProfileId = null) {
  return invoke("move_profile", { profileId, groupId, beforeProfileId });
}

export function retryDeletedCredentialCleanup() {
  return invoke("retry_deleted_credential_cleanup");
}

export function saveSettings(patch, workspaceDir) {
  return invoke("save_settings", { patch, workspaceDir });
}

export function setProfileBackground(profileId, terminalBackground, workspaceDir) {
  return invoke("set_profile_background", { profileId, terminalBackground, workspaceDir });
}

export function saveCredential(request) {
  return invoke("save_credential", { request });
}

export function readCredential(request) {
  return invoke("read_credential", { request });
}

export function removeCredential(request) {
  return invoke("remove_credential", { request });
}

export function testConnection(request) {
  return invoke("test_connection", { request });
}

export function listAgentIdentities(config) {
  return invoke("list_agent_identities", { config });
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

export function trustHostKey(request) {
  return invoke("trust_host_key", { request });
}

export function createRemoteDirectory(request) {
  return invoke("create_remote_directory", { request });
}

export function createRemoteFile(request) {
  return invoke("create_remote_file", { request });
}

export function listRemoteDirectory(request) {
  return invoke("list_remote_directory", { request });
}

export function renameRemotePath(request) {
  return invoke("rename_remote_path", { request });
}

export function startRemoteDelete(request) {
  return invoke("start_remote_delete", { request });
}

export function cancelRemoteDelete(taskId) {
  return invoke("cancel_remote_delete", { taskId });
}

export function streamRemoteDirectory(request) {
  return invoke("stream_remote_directory", { request });
}

export function readRemoteFileText(request) {
  return invoke("read_remote_file_text", { request });
}

export function writeRemoteFileText(request) {
  return invoke("write_remote_file_text", { request });
}

export function getRemoteSystemInfo(request) {
  return invoke("get_remote_system_info", { request });
}

export function startUploadTransfer(request) {
  return invoke("start_upload_transfer", { request });
}

export function startDownloadTransfer(request) {
  return invoke("start_download_transfer", { request });
}

export function cancelTransfer(taskId) {
  return invoke("cancel_transfer", { taskId });
}
