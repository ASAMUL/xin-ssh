import { i18n } from "../i18n";

// 后端所有 command / 事件的错误都是 { code, message, params } 结构（Rust 侧 AppError）。
// 前端只认 code：展示走 i18n，判断走 code 比较，不再匹配英文文案。
// message 仅作为未知 code 的兜底与日志用。

const UNKNOWN_CODE = "internal.unknown";

// 前端会据此分支的错误码；来自后端的码须与 src-tauri/src/error.rs 保持一致。
// 只列出真正被判断的码；纯展示用的码不需要出现在这里。
export const ERROR_CODES = Object.freeze({
  hostKeyChanged: "hostKey.changed",
  sessionNotRunning: "session.notRunning",
  sessionNotAccepting: "session.notAccepting",
  sessionRequestCanceled: "session.requestCanceled",
  sessionConnectionChanged: "session.connectionChanged",
  authFailed: "auth.failed",
  authKeyFile: "auth.keyFile",
  sshDisconnected: "ssh.disconnected",
  remoteTargetExists: "remote.targetExists",
  remoteIsDirectory: "remote.isDirectory",
  remoteNotRegularFile: "remote.notRegularFile",
  localTargetExists: "local.targetExists",
  localDuplicateTarget: "local.duplicateTarget",
  transferDuplicateUploadTarget: "transfer.duplicateUploadTarget",
  fileTooLarge: "file.tooLarge",
  fileBinary: "file.binary",
  fileNotUtf8: "file.notUtf8",
  fileOperationCanceled: "fileOperation.canceled",
  credentialEncryptFailed: "credential.encryptFailed",
  credentialDecryptFailed: "credential.decryptFailed",
  credentialKeyFailed: "credential.keyFailed",
  credentialCorrupt: "credential.corrupt",
});

// 说明会话已经断了、重试当前操作没有意义的错误码
export const CONNECTION_LOST_CODES = Object.freeze([
  ERROR_CODES.sessionNotRunning,
  ERROR_CODES.sessionNotAccepting,
  ERROR_CODES.sessionRequestCanceled,
  ERROR_CODES.sshDisconnected,
]);

// 目标已存在类错误：传输面板据此弹「覆盖 / 跳过 / 重命名 / 续传」对话框
export const TRANSFER_CONFLICT_CODES = Object.freeze([
  ERROR_CODES.remoteTargetExists,
  ERROR_CODES.localTargetExists,
  ERROR_CODES.localDuplicateTarget,
  ERROR_CODES.transferDuplicateUploadTarget,
]);

// 这几类 param 本身是枚举值，也要翻译；查不到就退回原值
const TRANSLATED_PARAM_GROUPS = {
  field: "fields",
  operation: "operations",
  kind: "operations",
};

// 把任意来源的错误收敛成统一形状：
// Tauri reject 的结构化错误、JS Error、字符串、事件载荷里的 error 都能进。
export function toAppError(error) {
  if (error && typeof error === "object" && typeof error.code === "string") {
    return {
      code: error.code,
      message: typeof error.message === "string" ? error.message : "",
      params: error.params && typeof error.params === "object" ? error.params : {},
    };
  }

  const message = error instanceof Error ? error.message : String(error ?? "");
  return { code: UNKNOWN_CODE, message, params: {} };
}

export function errorCode(error) {
  return toAppError(error).code;
}

export function hasErrorCode(error, codes) {
  return codes.includes(errorCode(error));
}

// 结构化错误 → 当前语言的可读文案。未登记的 code 退回后端英文 message。
export function localizeError(error) {
  if (!error) return "";

  const { code, message, params } = toAppError(error);
  const key = `errors.codes.${code}`;
  if (!i18n.global.te(key)) {
    return message || code;
  }

  const localized = i18n.global.t(key, localizeParams(params));
  return params.recoveryPath
    ? `${localized} ${i18n.global.t("errors.recoveryHint", { path: params.recoveryPath })}`
    : localized;
}

function localizeParams(params) {
  const localized = { ...params };

  for (const [name, group] of Object.entries(TRANSLATED_PARAM_GROUPS)) {
    const value = params[name];
    if (!value) continue;

    const key = `errors.${group}.${value}`;
    localized[name] = i18n.global.te(key) ? i18n.global.t(key) : value;
  }

  return localized;
}
