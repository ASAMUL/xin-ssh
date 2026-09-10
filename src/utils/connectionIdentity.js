export function connectionIdentity(connection) {
  return {
    host: connection.host,
    port: Number(connection.port ?? 22),
    username: connection.username,
  };
}

export function sameConnection(left, right) {
  return Boolean(left && right && left.host === right.host
    && Number(left.port) === Number(right.port) && left.username === right.username);
}
