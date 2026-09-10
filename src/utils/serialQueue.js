// 配置请求连同响应消费一起排队，避免旧快照后到；一次失败不能阻断后续保存。
export function createSerialQueue() {
  let tail = Promise.resolve();
  return (operation) => {
    const result = tail.then(operation);
    tail = result.catch(() => {});
    return result;
  };
}
