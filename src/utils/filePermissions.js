const READ = 4;
const WRITE = 2;
const EXECUTE = 1;

// 顺序就是 rwxr-xr-x 里三组的顺序
const ROLES = [
  { key: "owner", labelKey: "files.permissionsOwner", shift: 6 },
  { key: "group", labelKey: "files.permissionsGroup", shift: 3 },
  { key: "others", labelKey: "files.permissionsOthers", shift: 0 },
];

const listFormats = new Map();

function formatList(items, locale) {
  if (!listFormats.has(locale)) listFormats.set(locale, new Intl.ListFormat(locale, { type: "conjunction" }));
  return listFormats.get(locale).format(items);
}

function roleBits(mode, role) {
  return (mode >> role.shift) & 7;
}

// 英文的人群名是小写（它们也会出现在句中），只把整句首字母大写；中文不受影响
function capitalize(text, locale) {
  return text.charAt(0).toLocaleUpperCase(locale) + text.slice(1);
}

// 新手看不懂 rwxr-xr-x，列表里只回答「谁能改、谁只能看」；执行位和逐组明细留给悬停提示
export function summarizePermissions(mode, t, locale) {
  if (mode == null) return "--";

  const writers = ROLES.filter((role) => roleBits(mode, role) & WRITE);
  const readOnly = ROLES.filter((role) => (roleBits(mode, role) & (READ | WRITE)) === READ);
  const who = (roles) => formatList(roles.map((role) => t(`files.permissionsWho.${role.key}`)), locale);

  let text;
  if (!writers.length && !readOnly.length) text = t("files.permissionsSummary.none");
  else if (writers.length === ROLES.length) text = t("files.permissionsSummary.everyoneEdit");
  else if (readOnly.length === ROLES.length) text = t("files.permissionsSummary.everyoneRead");
  else if (!readOnly.length) text = t("files.permissionsSummary.onlyEdit", { who: who(writers) });
  else if (!writers.length) text = t("files.permissionsSummary.onlyRead", { who: who(readOnly) });
  else {
    // 三组都能访问时，只读的那部分就是「除了能改的人以外的所有人」，比逐个点名更好懂
    const rest = writers.length + readOnly.length === ROLES.length
      ? t("files.permissionsWho.rest")
      : who(readOnly);
    text = t("files.permissionsSummary.mixed", { writers: who(writers), rest });
  }
  return capitalize(text, locale);
}

// 悬停提示：逐组列出能做什么，末行保留原始的 rwx 和数字权限，老手不丢信息
export function describePermissions(mode, isDirectory, t, locale) {
  if (mode == null) return "";

  const lines = ROLES.map((role) => {
    const bits = roleBits(mode, role);
    const granted = [
      bits & READ && t("files.permissionsRead"),
      bits & WRITE && t("files.permissionsWrite"),
      bits & EXECUTE && t(isDirectory ? "files.permissionsTraverse" : "files.permissionsExecute"),
    ].filter(Boolean);
    return t("files.permissionsOption", {
      role: t(role.labelKey),
      permission: granted.length ? formatList(granted, locale) : t("files.permissionsNone"),
    });
  });
  const symbolic = ROLES.map((role) => {
    const bits = roleBits(mode, role);
    return `${bits & READ ? "r" : "-"}${bits & WRITE ? "w" : "-"}${bits & EXECUTE ? "x" : "-"}`;
  }).join("");
  return [...lines, `${symbolic} · ${mode.toString(8).padStart(3, "0")}`].join("\n");
}
