// 终端当前在哪个目录——给「文件面板跟随终端」用。
// 和命令标记一样不碰服务器：只看终端自己显示出来的东西。三种来源，按可靠程度排：
//   1. OSC 7：有的服务器每次出提示符前会报告当前目录（file://主机/路径），最准。见到过一次就只信它；
//   2. 提示符里的完整路径：Debian / Ubuntu / SUSE / Alpine 默认的 user@host:/var/log$；
//   3. 只有目录名的提示符（RHEL 系默认的 [user@host log]$）：读不出在哪一层，
//      于是记下回车执行的 cd 自己算出目标，再拿下一个提示符里的目录名核对——对得上才算数；
//      对不上（cd 失败、pushd、脚本里换了目录）就当不知道在哪，直到回到 ~ 或 / 能重新确认。
// 第一个认出的提示符所在的主机就是登录的这台。之后提示符换了主机名（终端里又 ssh 到别处、
// docker exec 进了容器），文件面板看的还是登录的那台，这时不跟。
import { normalizeRemotePath } from "./remotePath";

// 带分号、管道、变量、命令替换的 cd 算不准，不去猜，交给下一个提示符核对
const SHELL_SYNTAX = /[;&|`$(){}<>*?]/;
const CD_COMMAND = /^\s*cd(?:\s+(?<arg>.*?))?\s*$/;
const DIRECTORY_CHANGERS = /(^|[\s;&|(])(cd|pushd|popd)(\s|$)/;
// 提示符里有 ~、但登录用户的家目录还没查回来：先不下结论，查回来再用同一个提示符重算
const WAITING_FOR_HOME = Symbol("waitingForHome");

export const TERMINAL_CWD_STATUS = {
  following: "following",
  // 认不出提示符，或者跟丢了
  unknown: "unknown",
  // 终端连在另一台机器上
  elsewhere: "elsewhere",
};

export function createTerminalCwd(terminal, { loginUser, resolveLoginHome, onChange }) {
  let homeHost = null;
  let loginHome = null;
  let homeRequest = null;
  let oscSeen = false;
  let state = { path: null, status: TERMINAL_CWD_STATUS.unknown };
  // 上一个确认过的目录：cd - 要回到它；第一次确认（登录时的家目录）不该把文件面板拽走
  let confirmedPath = null;
  let previousPath = null;
  // 回车执行的 cd 解析结果，等下一个提示符出现时用
  let pendingCd = null;
  // 家目录还没查回来时，等它到了要用同一个提示符重算一遍
  let lastPrompt = null;
  let lastCd = null;

  const oscHandler = terminal.parser.registerOscHandler(7, (data) => {
    handleOsc7(data);
    return true;
  });

  function handleCommand(command) {
    pendingCd = parseCd(command);
  }

  function handlePrompt(prompt) {
    lastPrompt = prompt;
    lastCd = pendingCd;
    pendingCd = null;
    evaluate(prompt, lastCd);
  }

  function evaluate(prompt, cd) {
    homeHost ??= prompt.hostName;
    if (prompt.hostName !== homeHost) {
      update(state.path, TERMINAL_CWD_STATUS.elsewhere);
      return;
    }
    // OSC 7 已经报过准确的目录，提示符只用来确认人回到了登录的这台机器
    if (oscSeen) {
      update(state.path, state.path ? TERMINAL_CWD_STATUS.following : TERMINAL_CWD_STATUS.unknown);
      return;
    }

    const result = prompt.pathStyle === "full" ? fromFullPath(prompt) : fromBasename(prompt, cd);
    if (result === WAITING_FOR_HOME) return;
    update(result, result ? TERMINAL_CWD_STATUS.following : TERMINAL_CWD_STATUS.unknown);
  }

  function fromFullPath(prompt) {
    const text = prompt.pathText.trim();
    if (text.startsWith("/")) return normalizeRemotePath(text);
    if (text === "~" || text.startsWith("~/")) {
      const home = homeFor(prompt.userName);
      if (!home) return WAITING_FOR_HOME;
      return normalizeRemotePath(`${home}/${text.slice(1)}`);
    }
    // ~other 这类别人的家目录、PROMPT_DIRTRIM 截断成的 .../dir，都读不出完整路径
    return null;
  }

  function fromBasename(prompt, cd) {
    const name = prompt.pathText.trim();
    if (name === "/") return "/";
    if (name === "~") return homeFor(prompt.userName) ?? WAITING_FOR_HOME;

    const candidate = cd ? resolveCd(cd, prompt.userName) : state.path;
    if (candidate === WAITING_FOR_HOME) return WAITING_FOR_HOME;
    if (candidate && baseName(candidate) === name) return candidate;
    // cd 失败（目录不存在、没权限）人还在原地：目录名和原来的对得上就当没动
    return state.path && baseName(state.path) === name ? state.path : null;
  }

  function resolveCd(cd, user) {
    if (cd.kind === "uncertain") return null;
    if (cd.kind === "previous") return previousPath;
    if (cd.kind === "absolute") return cd.path;
    if (cd.kind === "home") {
      const home = homeFor(user);
      if (!home) return WAITING_FOR_HOME;
      return normalizeRemotePath(`${home}/${cd.path}`);
    }
    return state.path ? normalizeRemotePath(`${state.path}/${cd.path}`) : null;
  }

  // 提示符里的 ~ 跟着提示符上的用户走：sudo -i 之后是 root 的家目录，不是登录用户的
  function homeFor(user) {
    if (!user || user === loginUser) {
      if (!loginHome) requestLoginHome();
      return loginHome;
    }
    return user === "root" ? "/root" : `/home/${user}`;
  }

  function requestLoginHome() {
    if (homeRequest) return;
    homeRequest = resolveLoginHome()
      .then((home) => {
        loginHome = home;
        if (lastPrompt) evaluate(lastPrompt, lastCd);
      })
      .catch(() => {
        // 查不到就当这次不知道在哪；下一个提示符再试
        homeRequest = null;
      });
  }

  function handleOsc7(data) {
    let url;
    try {
      url = new URL(data);
    } catch {
      return;
    }
    if (url.protocol !== "file:") return;

    // 提示符里的 \h 只到第一个点，OSC 7 报的常常是完整主机名
    const host = url.hostname.split(".")[0];
    if (host && host !== "localhost") homeHost ??= host;
    oscSeen = true;
    if (host && host !== "localhost" && host !== homeHost) {
      update(state.path, TERMINAL_CWD_STATUS.elsewhere);
      return;
    }
    let path;
    try {
      path = decodeURIComponent(url.pathname);
    } catch {
      return;
    }
    update(normalizeRemotePath(path), TERMINAL_CWD_STATUS.following);
  }

  function update(path, status) {
    if (path === state.path && status === state.status) return;

    // 跟丢的时候保留不了路径：不知道在哪就是不知道
    const nextPath = status === TERMINAL_CWD_STATUS.unknown ? null : path;
    // 只有「从一个确认过的目录换到另一个」才让文件面板跟过去
    const navigate = status === TERMINAL_CWD_STATUS.following
      && confirmedPath !== null
      && nextPath !== confirmedPath;
    if (status === TERMINAL_CWD_STATUS.following && nextPath !== confirmedPath) {
      previousPath = confirmedPath;
      confirmedPath = nextPath;
    }
    state = { path: nextPath, status };
    onChange?.({ ...state }, { navigate });
  }

  function dispose() {
    oscHandler.dispose();
  }

  return { handleCommand, handlePrompt, dispose };
}

// 只认最简单的一条 cd；别的会换目录的写法记成「不确定」，交给下一个提示符核对
function parseCd(command) {
  if (SHELL_SYNTAX.test(command)) {
    return DIRECTORY_CHANGERS.test(command) ? { kind: "uncertain" } : null;
  }
  const match = CD_COMMAND.exec(command);
  if (!match) return DIRECTORY_CHANGERS.test(command) ? { kind: "uncertain" } : null;

  const arg = unquote(match.groups.arg ?? "");
  if (arg === "" || arg === "~") return { kind: "home", path: "" };
  if (arg === "-") return { kind: "previous" };
  // cd -P / -L 这类选项、~other 别人的家目录：少见，不去猜
  if (arg.startsWith("-") || (arg.startsWith("~") && !arg.startsWith("~/"))) return { kind: "uncertain" };
  if (arg.startsWith("~/")) return { kind: "home", path: arg.slice(2) };
  if (arg.startsWith("/")) return { kind: "absolute", path: normalizeRemotePath(arg) };
  return { kind: "relative", path: arg };
}

// 去掉成对的引号、把反斜杠转义的空格还原；更复杂的引号嵌套交给提示符核对
function unquote(arg) {
  const trimmed = arg.trim();
  const quoted = /^(['"])(.*)\1$/.exec(trimmed);
  if (quoted) return quoted[2];
  return trimmed.replaceAll("\\ ", " ");
}

function baseName(path) {
  return path === "/" ? "/" : path.slice(path.lastIndexOf("/") + 1);
}
