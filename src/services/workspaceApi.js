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

export function readSshConfig(path) {
  return invoke("read_ssh_config", { path });
}

export function exportProfiles(path, profiles) {
  return invoke("export_profiles", { path, profiles });
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

export function setProfileDetectedOs(profileId, detectedOs) {
  return invoke("set_profile_detected_os", { profileId, detectedOs });
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

export function startTunnel(request, authentication) {
  return invoke("start_tunnel", { request, authentication });
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

export function moveProfile(profileId, groupId) {
  return invoke("move_profile", { profileId, groupId });
}

export function reorderProfiles(profileIds) {
  return invoke("reorder_profiles", { profileIds });
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

export function testConnection(request, authentication) {
  return invoke("test_connection", { request, authentication });
}

export function listAgentIdentities(config) {
  return invoke("list_agent_identities", { config });
}

export function startTerminalSession(request, authentication) {
  return invoke("start_terminal_session", { request, authentication });
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

export function copyRemotePath(request) {
  return invoke("copy_remote_path", { request });
}

export function chmodRemotePath(request) {
  return invoke("chmod_remote_path", { request });
}

export function startRemoteDelete(request) {
  return invoke("start_remote_delete", { request });
}

// 删除、压缩解压、占用分析共用一个取消入口
export function cancelFileOperation(taskId) {
  return invoke("cancel_file_operation", { taskId });
}

export function startDiskUsage(request) {
  return invoke("start_disk_usage", { request });
}

export function readClipboard() {
  return invoke("read_clipboard");
}

// 原始字节：前 8 字节是宽高，后面是 RGBA 像素
export function readClipboardImage() {
  return invoke("read_clipboard_image");
}

// 内容按原始字节传；请求头只能放 ASCII，会话和路径都编码一下
export function writeRemoteFileBytes({ sessionId, path, bytes, overwrite }) {
  return invoke("write_remote_file_bytes", bytes, {
    headers: {
      "x-session-id": encodeURIComponent(sessionId),
      "x-remote-path": encodeURIComponent(path),
      "x-overwrite": overwrite ? "1" : "0",
    },
  });
}

export function startRemoteCompress(request) {
  return invoke("start_remote_compress", { request });
}

export function startRemoteExtract(request) {
  return invoke("start_remote_extract", { request });
}

export function streamRemoteDirectory(request) {
  return invoke("stream_remote_directory", { request });
}

export function resolveRemoteHome(sessionId) {
  return invoke("resolve_remote_home", { sessionId });
}

export function statRemotePath(sessionId, path) {
  return invoke("stat_remote_path", { sessionId, path });
}

// 在终端这条 SSH 连接上转发服务器的 127.0.0.1:remotePort，返回本机端口
export function forwardTerminalPort(sessionId, remotePort) {
  return invoke("forward_terminal_port", { sessionId, remotePort });
}

export function stopTerminalForward(sessionId, localPort) {
  return invoke("stop_terminal_forward", { sessionId, localPort });
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

export function pauseTransfer(taskId) {
  return invoke("pause_transfer", { taskId });
}

export function discardPartialFiles(request) {
  return invoke("discard_partial_files", { request });
}

export function measureSessionLatency(request) {
  return invoke("measure_session_latency", { request });
}
