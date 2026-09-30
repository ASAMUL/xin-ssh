// 命令联想：在提示符后打字时，把历史里最近一条以当前输入开头的命令，
// 剩下那截用灰字画在光标后面；光标在行尾时按 → 或 End 采纳。
// 效果同 fish / zsh-autosuggestions，但全在客户端做，不需要服务器装任何东西。
//
// 灰字只是 xterm 装饰挂上来的一段 DOM，终端缓冲区里并没有这些字符；
// 采纳时把剩下那截当作用户键入发给服务器，由 shell 自己回显，和亲手敲完全一样。
import { mixHex } from "./hexColor";
import { parseShellPrompt, readTerminalRow } from "./shellPrompt";

// 中日韩字符在终端里占两格，灰字要按格子排才能和随后真正回显的字对齐
const WIDE_CHAR = /[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]/;

export function createCommandSuggest(terminal, { suggest }) {
  let enabled = true;
  let color = null;
  // 当前提示符这一行的灰字层。xterm 每注册 / 撤掉一个装饰都会整屏重绘，
  // 所以一行只建一次：从输入起点铺到行尾，打字时只改里面的 DOM，不反复注册
  let ghost = null;
  let frame = null;

  const listeners = [
    // 每次按键都要等服务器回显以后才知道当前输入是什么，所以跟着输出走，而不是跟着按键走
    terminal.onWriteParsed(scheduleUpdate),
    terminal.buffer.onBufferChange(hide),
    terminal.onResize(() => {
      hide();
      scheduleUpdate();
    }),
  ];

  function scheduleUpdate() {
    if (frame === null) frame = requestAnimationFrame(update);
  }

  function update() {
    frame = null;
    const buffer = terminal.buffer.active;
    const line = buffer.baseY + buffer.cursorY;
    if (ghost && ghost.marker.line !== line) hide();

    const next = enabled && color && buffer.type === "normal" ? findSuggestion(buffer, line) : null;
    if (!next) {
      if (ghost) setContent(null);
      return;
    }
    if (ghost && ghost.startX !== next.startX) hide();
    if (!ghost && !createLayer(next)) return;
    setContent(next);
  }

  // 只在「认得的提示符 + 光标停在输入末尾 + 后面全空」时联想。
  // 光标后面有字说明用户正在行中间改，或者服务器自己装了 zsh-autosuggestions 在画灰字——都让开。
  function findSuggestion(buffer, line) {
    const row = readTerminalRow(buffer, line);
    if (!row || row.isWrapped) return null;
    const prompt = parseShellPrompt(row);
    if (!prompt) return null;

    const inputStart = prompt.end + 1;
    const cursorIndex = row.columns.findIndex((x) => x >= buffer.cursorX);
    if (cursorIndex <= inputStart || row.text.slice(cursorIndex).trim()) return null;

    const input = row.text.slice(inputStart, cursorIndex);
    if (!input.trim()) return null;
    const suffix = suggest(input)?.slice(input.length);
    if (!suffix) return null;

    const startX = row.columns[inputStart];
    // 右边留一格，放不下的部分直接截掉
    const chars = fitToCells([...suffix], terminal.cols - 1 - buffer.cursorX);
    return chars.length ? { startX, offset: buffer.cursorX - startX, suffix, chars } : null;
  }

  function createLayer({ startX }) {
    const width = terminal.cols - 1 - startX;
    if (width < 1) return false;
    const marker = terminal.registerMarker(0);
    if (!marker) return false;
    const decoration = terminal.registerDecoration({ marker, x: startX, width });
    if (!decoration) {
      marker.dispose();
      return false;
    }

    // 网格写在自己套的一层里：xterm 每次渲染都会往装饰元素上写内联 display: block，
    // 直接给它设 grid 会被盖掉，灰字就从输入起点开始排、压在已经敲的字上
    const layer = document.createElement("div");
    layer.className = "terminal-command-ghost";
    layer.style.color = color;
    layer.style.fontFamily = terminal.options.fontFamily;
    layer.style.fontSize = `${terminal.options.fontSize}px`;
    // 每个字各占自己的格子：DOM 文字的字宽和 xterm 取整后的格宽差一点点，长命令会越排越歪
    layer.style.gridTemplateColumns = `repeat(${width}, 1fr)`;

    ghost = { marker, decoration, layer, startX, offset: 0, chars: [], suffix: null, stale: false };
    decoration.onRender((element) => {
      if (element.firstChild !== layer) element.replaceChildren(layer);
    });
    return true;
  }

  function setContent(next) {
    ghost.offset = next?.offset ?? 0;
    ghost.chars = next?.chars ?? [];
    ghost.suffix = next?.suffix ?? null;
    ghost.stale = false;
    paint();
  }

  function paint() {
    ghost.layer.replaceChildren(...ghost.chars.map((char, index) => {
      const span = document.createElement("span");
      span.textContent = char;
      // 已经敲进去的那几格空着，灰字从光标所在的格子开始
      if (index === 0) span.style.gridColumnStart = String(ghost.offset + 1);
      if (cellWidth(char) === 2) span.style.gridColumnEnd = "span 2";
      return span;
    }));
  }

  function hide() {
    if (!ghost) return;
    // 先撤装饰再销毁标记，顺序反了会搅乱 xterm 的装饰列表（见 commandMarks.js 的 discardMark）
    ghost.decoration.dispose();
    ghost.marker.dispose();
    ghost = null;
  }

  // 光标在输入末尾、灰字正显示着才接管；返回要补发给服务器的那截，否则 null 让按键照常走
  function accept() {
    if (!ghost?.suffix || ghost.stale) return null;
    const { suffix } = ghost;
    setContent(null);
    return suffix;
  }

  function handleInput(data) {
    // 回车后这一行就定了，灰字不能留在已执行的命令后面
    if (data.includes("\r")) {
      hide();
      return;
    }
    // 刚敲的字还没回显，屏幕上的灰字对应的是旧输入：先别让 → 采纳它，等回显后重新算。
    // 不直接清掉，是为了打字时灰字不一闪一闪
    if (ghost) ghost.stale = true;
  }

  function setTheme(theme) {
    color = mixHex(theme.xterm.foreground, theme.xterm.background, 0.4);
    hide();
    scheduleUpdate();
  }

  function setEnabled(value) {
    enabled = value;
    if (!enabled) hide();
    else scheduleUpdate();
  }

  function dispose() {
    cancelAnimationFrame(frame);
    listeners.forEach((listener) => listener.dispose());
    hide();
  }

  return { accept, handleInput, setTheme, setEnabled, dispose };
}

function cellWidth(char) {
  return WIDE_CHAR.test(char) ? 2 : 1;
}

function fitToCells(chars, room) {
  const fitted = [];
  let used = 0;
  for (const char of chars) {
    used += cellWidth(char);
    if (used > room) break;
    fitted.push(char);
  }
  return fitted;
}
