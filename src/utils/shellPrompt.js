// 认出一行里的 shell 提示符，并给出 user / path 各占哪几格，供终端给提示符上色；
// 同时带出用户名、主机名和路径文字，文件面板据此跟随终端目录（terminalCwd.js）。
//
// 只认几个主流发行版的默认形状：认错了会把普通输出染上色，宁可漏认也不要误认。
// 自定义过提示符（oh-my-zsh、starship…）的用户本来就有自己的配色，不需要我们插手。
// 可选的 (venv) / (base) 前缀是 Python 虚拟环境和 conda 加上去的。
const ENV_PREFIX = String.raw`(?<env>\([^()\s]+\)\s+)?`;

// pathStyle：full = 提示符里是完整路径（\w）；basename = 只有当前目录名（\W），读不出在哪一层
const PROMPT_PATTERNS = [
  // Debian / Ubuntu / SUSE：user@host:path$
  {
    pathStyle: "full",
    pattern: new RegExp(String.raw`^${ENV_PREFIX}(?<user>[\w.-]+)@(?<host>[\w.-]+):(?<path>.*?)(?<symbol>[#$>]) `, "d"),
  },
  // RHEL / CentOS / Rocky / Fedora / Arch：[user@host dir]$
  {
    pathStyle: "basename",
    pattern: new RegExp(String.raw`^${ENV_PREFIX}\[(?<user>[\w.-]+)@(?<host>[\w.-]+) (?<path>[^\]]+)\](?<symbol>[#$]) `, "d"),
  },
  // Alpine / BusyBox：host:path#
  {
    pathStyle: "full",
    pattern: new RegExp(String.raw`^${ENV_PREFIX}(?<host>[\w.-]+):(?<path>[~/]\S*)(?<symbol>[#$]) `, "d"),
  },
];

// 读出 buffer 里一整行的文字，并记下每个字符落在第几格。
// 中文目录名这类宽字符占两格，字符串下标和格子列号对不上，上色时必须换算。
export function readTerminalRow(buffer, y) {
  const line = buffer.getLine(y);
  if (!line) return null;

  const cell = buffer.getNullCell();
  const columns = [];
  let text = "";
  for (let x = 0; x < line.length; x += 1) {
    line.getCell(x, cell);
    // 宽字符的后半格宽度为 0，内容已经记在前半格
    if (cell.getWidth() === 0) continue;
    const chars = cell.getChars() || " ";
    for (let index = 0; index < chars.length; index += 1) columns.push(x);
    text += chars;
  }
  columns.push(line.length);
  return { text, columns, isWrapped: line.isWrapped };
}

// 返回 null 表示这一行不是认得的提示符；否则各区间都已换算成格子（x, width）。
export function parseShellPrompt(row) {
  for (const { pattern, pathStyle } of PROMPT_PATTERNS) {
    const match = pattern.exec(row.text);
    if (!match) continue;

    const { user, path, symbol } = match.indices.groups;
    // 提示符到符号为止，后面那个空格不算
    const end = symbol[1];
    const toCells = ([start, stop]) => ({
      x: row.columns[start],
      width: row.columns[stop] - row.columns[start],
    });
    return {
      end,
      cells: toCells([0, end]),
      user: user ? toCells(user) : null,
      path: path[1] > path[0] ? toCells(path) : null,
      isRoot: match.groups.user === "root",
      // Alpine 的提示符不带用户名
      userName: match.groups.user ?? null,
      hostName: match.groups.host,
      pathText: match.groups.path,
      pathStyle,
    };
  }
  return null;
}
