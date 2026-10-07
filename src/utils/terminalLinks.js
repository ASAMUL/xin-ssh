// 从终端的一行文字里认出可以点的东西：远程路径（可带 :行号:列号）和服务器上的端口。
// 只管「认」，返回字符下标区间；换算成格子、怎么打开，见 terminalLinkProvider.js 和 useTerminalLinks.js。
//
// 宁可漏认也不要误认：每个下划线都在暗示「这里能点」，认错了比认不出更烦人。
//   · 绝对路径、~/ 开头的、带 ./ ../ 的，直接认；
//   · 带斜杠的相对路径，最后一段要有扩展名，或者后面跟着 :行号（grep -rn、编译报错）；
//   · 光秃秃的文件名只在「常见源码 / 配置扩展名 + :行号」时认（main.go:42:7），
//     否则 example.com、v1.2 这类全都会被当成文件。
// URL 交给 xterm 的 WebLinksAddon，这里不碰。

// 光秃秃的文件名只认这些扩展名
const SOURCE_EXTENSIONS = new Set([
  "c", "cc", "cpp", "cs", "css", "go", "h", "hpp", "html", "java", "js", "jsx", "kt", "less", "lua",
  "mjs", "cjs", "php", "pl", "py", "rb", "rs", "scala", "scss", "sh", "sql", "swift", "ts", "tsx", "vue",
  "conf", "cfg", "ini", "json", "log", "md", "properties", "toml", "txt", "xml", "yaml", "yml",
]);

// 端口只认「服务器自己身上」的：回环地址和通配地址。别的 IP 是另一台机器，转发过去不是用户想要的
const LOCAL_HOST = String.raw`localhost|127(?:\.\d{1,3}){3}|0\.0\.0\.0|\[::1?\]|::1?|\*`;

const NAME = String.raw`[\w.@+-]+`;
const PATTERNS = [
  // Python 报错：File "/app/main.py", line 42
  { kind: "path", pattern: /File "([^"\n]+)", line (\d+)/g, path: 1, line: 2 },
  // 引号里带空格的绝对路径：'/var/www/my site/index.html'
  { kind: "path", pattern: /(["'])(\/[^"'\n]*[^"'\s/])\1/g, path: 2 },
  // 一般路径（可带 :行号:列号），具体认不认在 acceptPath 里判断
  {
    kind: "path",
    pattern: new RegExp(String.raw`(?<![\w.~/@+-])((?:~|\.{1,2})?\/?(?:${NAME}\/)*${NAME}\/?)(?::(\d+)(?::(\d+))?)?`, "g"),
    path: 1,
    line: 2,
    column: 3,
  },
  // ss / netstat / 服务日志：0.0.0.0:8080、127.0.0.1:3000、[::]:443、*:80
  { kind: "port", pattern: new RegExp(String.raw`(?<![\w.:\]/])(?:${LOCAL_HOST}):(\d{1,5})\b`, "g"), port: 1 },
  // 服务日志：listening on port 3000、PORT=8080、Listening on :8080
  { kind: "port", pattern: /\bport\b\s*[:=]?\s*(\d{2,5})\b/gi, port: 1 },
  { kind: "port", pattern: /\b(?:on|at|listen(?:ing)?)\s+:(\d{2,5})\b/gi, port: 1 },
];

export function findTerminalLinks(text) {
  const found = [];
  for (const rule of PATTERNS) {
    rule.pattern.lastIndex = 0;
    for (const match of String(text ?? "").matchAll(rule.pattern)) {
      const link = rule.kind === "path" ? pathLink(match, rule) : portLink(match, rule);
      if (link) found.push(link);
    }
  }
  return dropOverlaps(found);
}

function pathLink(match, rule) {
  const raw = match[rule.path];
  // 句末的 . , 不是路径的一部分：see /etc/hosts.
  const path = raw.replace(/[.,]+$/, "");
  const line = rule.line ? Number(match[rule.line]) || null : null;
  const column = rule.column ? Number(match[rule.column]) || null : null;
  if (!acceptPath(path, line)) return null;
  // 提示符紧贴在没换行的输出后面时（/dataroot@web:/home#），输出的「/data」和提示符的「root@web」
  // 连成了一个像路径的东西：带 @、后面紧跟 :/ 或 :~ 的，是提示符不是路径
  const after = match.input.slice(match.index + match[0].length);
  if (path.includes("@") && /^:[~/]/.test(after)) return null;

  const start = match.index + match[0].indexOf(raw);
  // 下划线盖住路径和行列号；引号、"File" 这些不算
  const suffix = line && rule.column ? match[0].slice(match[0].indexOf(raw) + raw.length) : "";
  const end = suffix && raw === path ? start + raw.length + suffix.length : start + path.length;
  return { kind: "path", start, end, path, line, column };
}

function acceptPath(path, line) {
  if (!path || path === "/" || path === "~" || /^\.+\/?$/.test(path)) return false;
  if (path.startsWith("/") || path.startsWith("~/") || /^\.{1,2}\//.test(path)) return true;

  const segments = path.split("/").filter(Boolean);
  const extension = /\.([A-Za-z][\w-]{0,9})$/.exec(segments.at(-1) ?? "")?.[1]?.toLowerCase();
  if (segments.length > 1) return Boolean(extension || line);
  return Boolean(line && extension && SOURCE_EXTENSIONS.has(extension));
}

function portLink(match, rule) {
  const port = Number(match[rule.port]);
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  const end = match.index + match[0].length;
  return { kind: "port", start: end - match[rule.port].length, end, port };
}

// 同一段文字被几条规则认出来时，先出现的、更长的那个说了算
function dropOverlaps(links) {
  const sorted = links.sort((a, b) => a.start - b.start || b.end - a.end);
  const kept = [];
  for (const link of sorted) {
    const last = kept.at(-1);
    if (!last || link.start >= last.end) kept.push(link);
  }
  return kept;
}
