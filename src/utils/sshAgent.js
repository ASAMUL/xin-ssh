export function normalizeAgentConfig(agent) {
  const provider = agent?.provider ?? "system";
  return {
    provider,
    socketPath: provider === "system" ? (agent?.socketPath?.trim() || null) : null,
    keyFingerprint: agent?.keyFingerprint?.trim() || null,
  };
}

export function sameAgentConfig(left, right) {
  const previous = normalizeAgentConfig(left);
  const current = normalizeAgentConfig(right);
  return previous.provider === current.provider
    && previous.socketPath === current.socketPath
    && previous.keyFingerprint === current.keyFingerprint;
}
