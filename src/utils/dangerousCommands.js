// 生产机上的危险命令：回车发给服务器之前先认一遍，命中了就先弹确认（见 useTerminalGuard）。
//
// 按「命令」认，而不是在整行里搜字符串：一行按 ; && || | & 拆成几段，每段去掉 sudo / nohup / env 这类前缀，
// 再看真正执行的是哪个程序、带了什么参数。这样 grep "rm -rf" log、echo reboot 不会误报。
// SQL 是例外：它常常出现在 mysql 客户端里或 mysql -e "..." 的引号里，整行搜。
// 规则宁可多拦：误报的代价只是多点一下确认，漏报的代价是生产事故。

// 只是给后面的命令加个壳、自己不干活的前缀
const WRAPPERS = new Set(["sudo", "doas", "nohup", "time", "env", "exec", "command", "builtin", "nice", "xargs"]);
// sudo 这些选项后面跟一个参数（sudo -u deploy rm …），参数不能当成命令
const WRAPPER_OPTIONS_WITH_VALUE = new Set(["-u", "-g", "-C", "-h", "-p", "-U", "-n", "-I", "-P", "-L", "-s"]);

const SQL_PATTERN = /\b(drop\s+(database|schema|table)|truncate\s+(table\s+)?[\w`"]+)/i;

const COMMAND_RULES = [
  { kind: "recursiveDelete", test: isRecursiveForceDelete },
  { kind: "shutdown", test: isShutdown },
  { kind: "formatDisk", test: (name, args) => name === "mkfs" || name.startsWith("mkfs.") || (name === "dd" && args.some((arg) => arg.startsWith("of=/dev/"))) },
  { kind: "clearCrontab", test: (name, args) => name === "crontab" && hasShortFlag(args, "r") },
  { kind: "flushFirewall", test: (name, args) => (name === "iptables" || name === "ip6tables") && (hasShortFlag(args, "F") || args.includes("--flush")) },
  { kind: "recursiveRootPermission", test: (name, args) => (name === "chmod" || name === "chown") && (hasShortFlag(args, "R") || args.includes("--recursive")) && args.includes("/") },
];

// 返回 null 表示没认出危险操作；否则 { kind, segment }，segment 是命中的那一段，确认框里会标出来
export function findDangerousCommand(command) {
  const text = String(command ?? "");
  if (!text.trim()) return null;

  for (const segment of splitSegments(text)) {
    const tokens = stripWrappers(segment.trim().split(/\s+/).filter(Boolean));
    if (!tokens.length) continue;
    const name = commandName(tokens[0]);
    const args = tokens.slice(1);
    const rule = COMMAND_RULES.find(({ test }) => test(name, args));
    if (rule) return { kind: rule.kind, segment: segment.trim() };
  }

  const sql = SQL_PATTERN.exec(text);
  return sql ? { kind: "dropDatabase", segment: sql[0] } : null;
}

function splitSegments(text) {
  return text.split(/&&|\|\||[;|&\n]/);
}

function stripWrappers(tokens) {
  let index = 0;
  while (index < tokens.length) {
    const token = tokens[index];
    // FOO=bar cmd 这种临时环境变量
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(token)) {
      index += 1;
      continue;
    }
    if (!WRAPPERS.has(commandName(token))) break;
    index += 1;
    while (index < tokens.length && tokens[index].startsWith("-")) {
      index += WRAPPER_OPTIONS_WITH_VALUE.has(tokens[index]) ? 2 : 1;
    }
  }
  return tokens.slice(index);
}

// shell 真正执行的程序名。\rm 是绕开 alias 的写法——CentOS / RHEL 的 root 默认 alias rm='rm -i'，
// 运维习惯敲 \rm -rf 跳过确认，恰恰是最该拦的那种；'rm'、"rm" 加引号同样是在执行 rm；/bin/rm 只看最后一段
function commandName(token) {
  return token
    .replace(/^\\/, "")
    .replace(/^(['"])(.*)\1$/, "$2")
    .replace(/^.*\//, "");
}

// -rf / -fr / -Rf / -r -f / --recursive --force 都算
function isRecursiveForceDelete(name, args) {
  if (name !== "rm") return false;
  const recursive = hasShortFlag(args, "r") || hasShortFlag(args, "R") || args.includes("--recursive");
  const force = hasShortFlag(args, "f") || args.includes("--force");
  return recursive && force;
}

function isShutdown(name, args) {
  if (["reboot", "shutdown", "poweroff", "halt"].includes(name)) return true;
  if (name === "init" || name === "telinit") return args[0] === "0" || args[0] === "6";
  if (name === "systemctl") return args.some((arg) => ["reboot", "poweroff", "halt", "kexec"].includes(arg));
  return false;
}

// 短选项可以合写（-rf）也可以分开写（-r -f），-- 之后的都是参数
function hasShortFlag(args, flag) {
  for (const arg of args) {
    if (arg === "--") return false;
    if (/^-[A-Za-z]+$/.test(arg) && arg.includes(flag)) return true;
  }
  return false;
}

// bash 的 Ctrl+R / Ctrl+S 把整行改写成搜索框：(reverse-i-search)`关键字': 命令，
// 这时回车执行的是冒号后面那段，提示符已经不在行上了。不是搜索框返回 null
const HISTORY_SEARCH_PATTERN = /^\((?:failed )?(?:reverse-|bck-|fwd-)?i-search\)`.*?': ?(.*)$/;

export function historySearchCommand(line) {
  return HISTORY_SEARCH_PATTERN.exec(String(line ?? "").trim())?.[1].trim() || null;
}

// 粘贴的内容带换行：老版本 bash（没开括号粘贴模式）会把每一行立刻执行。
// 返回要预览的行（去掉末尾空行）；不带换行返回 null，不用确认
export function pastedLines(text) {
  const normalized = String(text ?? "").replace(/\r\n?/g, "\n");
  if (!normalized.includes("\n")) return null;
  return normalized.replace(/\n+$/, "").split("\n");
}
