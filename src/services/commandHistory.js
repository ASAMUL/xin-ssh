// 命令联想用的历史。只存在本机 IndexedDB：不进可同步的工作区目录，
// 同步工作区、换电脑都不会把谁敲过什么命令带走。
//
// 每条连接一份、全局一份，都是「最近用过的在前」。联想时先找这台服务器上敲过的，
// 再找别的服务器上敲过的，最后找命令片段——新服务器上什么都没敲过时，片段也能先顶上。
import { createStore, del, get, set } from "idb-keyval";
import { createSerialQueue } from "../utils/serialQueue";

const historyStore = createStore("xin-terminal-command-history", "history");
const HISTORY_KEY = "commands";
const HISTORY_VERSION = 1;
const PROFILE_LIMIT = 2000;
const GLOBAL_LIMIT = 5000;
const MAX_COMMAND_LENGTH = 500;
// 敲命令的节奏很密，攒一秒再落盘
const SAVE_DELAY_MS = 1000;
// 看着像带了密码 / 令牌的命令不记：服务器的 shell 历史里本来就有一份，本机不该再多一份副本
const SECRET_PATTERN = /(pass(word|wd)?|token|secret|api[_-]?key|credential)s?\s*[=:]|--password\b|\s-p\S/i;
const enqueueHistoryWrite = createSerialQueue();

let history = { global: [], profiles: {} };
let loading = null;
let saveTimer = null;

export function loadCommandHistory() {
  loading ??= get(HISTORY_KEY, historyStore)
    .then((stored) => {
      if (stored?.version !== HISTORY_VERSION) return;
      // 加载期间已经记下的命令更新，排在存档前面
      history = {
        global: mergeRecent(history.global, stored.global, GLOBAL_LIMIT),
        profiles: Object.fromEntries(
          [...new Set([...Object.keys(stored.profiles ?? {}), ...Object.keys(history.profiles)])].map((id) => [
            id,
            mergeRecent(history.profiles[id], stored.profiles?.[id], PROFILE_LIMIT),
          ]),
        ),
      };
    })
    .catch((error) => console.warn("[Xin SSH] command history unavailable", error));
  return loading;
}

// command 是提示符后面的原样文字（只去了行尾空白），开头的空格要留给这里判断
export function recordCommand(profileId, command) {
  if (!shouldRemember(command)) return;

  history.global = remember(history.global, command, GLOBAL_LIMIT);
  if (profileId) {
    history.profiles[profileId] = remember(history.profiles[profileId] ?? [], command, PROFILE_LIMIT);
  }
  scheduleSave();
}

export function suggestCommand(profileId, input, snippetCommands = []) {
  const sources = [profileId ? history.profiles[profileId] : null, history.global, snippetCommands];
  for (const commands of sources) {
    const match = commands?.find((command) => command.length > input.length && command.startsWith(input));
    if (match) return match;
  }
  return null;
}

export async function clearCommandHistory() {
  clearTimeout(saveTimer);
  history = { global: [], profiles: {} };
  await enqueueHistoryWrite(() => del(HISTORY_KEY, historyStore));
}

function shouldRemember(command) {
  return command.length >= 2
    && command.length <= MAX_COMMAND_LENGTH
    // bash 的 HISTCONTROL=ignorespace 约定：空格开头的命令是用户明确不想留痕的
    && !/^\s/.test(command)
    && !SECRET_PATTERN.test(command);
}

function remember(commands, command, limit) {
  return [command, ...commands.filter((item) => item !== command)].slice(0, limit);
}

function mergeRecent(recent = [], stored = [], limit) {
  return [...new Set([...recent, ...stored])].slice(0, limit);
}

function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    const snapshot = { version: HISTORY_VERSION, global: history.global, profiles: { ...history.profiles } };
    enqueueHistoryWrite(() => set(HISTORY_KEY, snapshot, historyStore))
      .catch((error) => console.warn("[Xin SSH] command history save failed", error));
  }, SAVE_DELAY_MS);
}
