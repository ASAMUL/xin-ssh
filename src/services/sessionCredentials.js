import { connectionIdentity, sameConnection } from "../utils/connectionIdentity";
import { normalizeAgentConfig, sameAgentConfig } from "../utils/sshAgent";
import { usesSavedCredentials } from "../utils/authentication";

// 凭据只在当前进程内保留，不进入 Pinia 状态、传输历史或可同步配置。
const confirmedCredentials = new Map();
const pendingCredentials = new Map();

export function sessionCredentialScope(session) {
  return {
    ...connectionIdentity(session),
    profileId: session.profileId,
    workspaceDir: session.workspaceDir,
    authMethod: session.authMethod,
    privateKeyPath: session.privateKeyPath || null,
    agent: session.authMethod === "agent" ? normalizeAgentConfig(session.agent) : null,
  };
}

export function sameCredentialScope(left, right) {
  return sameConnection(left, right)
    && left.profileId === right.profileId
    && left.workspaceDir === right.workspaceDir
    && left.authMethod === right.authMethod
    && left.privateKeyPath === right.privateKeyPath
    && sameAgentConfig(left.agent, right.agent);
}

export function readSessionCredentials(session) {
  const cached = confirmedCredentials.get(session.id);
  if (!cached) return null;
  if (!sameCredentialScope(cached.scope, sessionCredentialScope(session))) {
    clearSessionCredentials(session.id);
    return null;
  }
  return { ...cached.credentials };
}

// IPC 返回只表示后端已登记；等待当前尝试的 connected 事件，才能把输入用于重连。
export function stageSessionCredentials(sessionId, attemptId, scope, credentials) {
  if (!usesSavedCredentials(scope.authMethod)) {
    clearSessionCredentials(sessionId);
    return;
  }
  pendingCredentials.set(sessionId, {
    attemptId,
    scope,
    credentials: {
      password: scope.authMethod === "password" ? (credentials.password || null) : null,
      passphrase: scope.authMethod === "privateKey" ? (credentials.passphrase || null) : null,
    },
  });
}

export function confirmSessionCredentials(sessionId, attemptId) {
  const pending = pendingCredentials.get(sessionId);
  if (!pending || pending.attemptId !== attemptId) return;
  confirmedCredentials.set(sessionId, { scope: pending.scope, credentials: pending.credentials });
  pendingCredentials.delete(sessionId);
}

export function discardPendingCredentials(sessionId, attemptId) {
  if (pendingCredentials.get(sessionId)?.attemptId === attemptId) {
    pendingCredentials.delete(sessionId);
  }
}

export function clearSessionCredentials(sessionId) {
  confirmedCredentials.delete(sessionId);
  pendingCredentials.delete(sessionId);
}
