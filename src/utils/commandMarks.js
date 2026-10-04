// 命令标记：不碰服务器、不注入任何脚本，只靠客户端自己看得见的两件事——
// 屏幕上出现了一个认得的提示符，以及用户按下了回车。
// 在这两个时刻给那一行打 xterm 标记，就能做到：
//   · 提示符退后、命令突出：user@host 和分隔符调暗，路径用蓝色，root 用品红提醒；
//   · 命令那一行右侧淡淡地标出回车时间，跑了 3 秒以上再带上耗时；
//   · Ctrl+↑ / Ctrl+↓ 在执行过的命令之间跳转；
//   · 把执行过的命令交给 onCommand（命令联想的历史就从这里来）；
//   · 每出现一个新的、等待输入的提示符就交给 onPrompt（文件面板跟随终端目录从这里来）；
//   · 记下每条输入从哪一格开始（noteKeystroke / readInput），生产机保护据此读出要执行的命令。
// 思路同 Windows Terminal 的 autoMarkPrompts（回车即打标记，不需要 shell 集成）。
import { mixHex } from "./hexColor";
import { parseShellPrompt, readTerminalRow } from "./shellPrompt";

// Powerlevel10k 默认也是 3 秒：短于这个的命令，耗时只是噪音
const DURATION_THRESHOLD_MS = 3000;
// 输出停下来一小会儿再看光标所在行：流式输出途中碰巧有一行长得像提示符，不能当真
const INSPECT_DELAY_MS = 60;
// 右侧标签和命令文字之间至少留这么多格，挤不下就不显示
const LABEL_GAP_CELLS = 3;

export function createCommandMarks(terminal, { onCommand, onPrompt } = {}) {
  const marks = [];
  let palette = null;
  // 回车后在等下一个提示符出现、好算耗时的那条
  let running = null;
  // 回车后等回显落定、再量右侧空位的那条
  let pendingLabel = null;
  // 提示符出现以后用户有没有输入过：回车那一刻回显可能还没回来，不能靠读屏幕判断命令是否为空
  let hasInput = false;
  let inspectTimer = null;
  // 在这一行开始按键时光标停在哪一格（{ marker, x }）——那就是提示符的末尾，不管提示符长什么样。
  // 生产机保护靠它从认不出的提示符（[PROD] root@web:~#、➜ ~、❯）后面读出真正要执行的命令
  let inputStart = null;

  const listeners = [
    terminal.onWriteParsed(scheduleInspect),
    // vim / top 这类全屏程序用的是另一块缓冲区，按行号着色会染到它们的画面上，切过去时先全部撤掉
    terminal.buffer.onBufferChange(repaint),
    terminal.onResize(repaint),
  ];

  function buffer() {
    return terminal.buffer.active;
  }

  function isNormalBuffer() {
    return buffer().type === "normal";
  }

  function cursorLine() {
    return buffer().baseY + buffer().cursorY;
  }

  function findMark(line) {
    return marks.find((mark) => mark.marker.line === line);
  }

  // 长命令、长提示符会折成好几行，这组折行从哪一行开始
  function groupStart(line) {
    while (line > 0 && buffer().getLine(line)?.isWrapped) line -= 1;
    return line;
  }

  // 每次按键、xterm 处理之前调用：光标还在同一组折行上就是同一条输入，起点不动；
  // 换到了新的一组行上（回车后出了新提示符、Ctrl+C 换了行），这一下按键的位置就是新起点。
  // 只认真实按键，不认 onData：服务器查询光标位置时 xterm 的自动应答也走 onData，那时光标未必在提示符后面
  function noteKeystroke() {
    if (!isNormalBuffer()) return;
    const line = cursorLine();
    if (inputStart && !inputStart.marker.isDisposed
      && groupStart(inputStart.marker.line) === groupStart(line)) return;
    inputStart?.marker.dispose();
    const marker = terminal.registerMarker(0);
    inputStart = marker ? { marker, x: buffer().cursorX } : null;
  }

  // 光标所在这组折行里、从开始输入那一格往后的文字；不知道这一行从哪开始输入的返回 null
  function readInput() {
    if (!isNormalBuffer() || !inputStart || inputStart.marker.isDisposed) return null;
    const inputLine = inputStart.marker.line;
    const start = groupStart(cursorLine());
    if (groupStart(inputLine) !== start) return null;

    let text = "";
    let line = start;
    do {
      const row = readTerminalRow(buffer(), line);
      // 起点之前的几行是折行的长提示符，跳过
      if (row && line === inputLine) text += row.text.slice(textIndexAt(row, inputStart.x));
      else if (row && line > inputLine) text += row.text;
      line += 1;
    } while (buffer().getLine(line)?.isWrapped);
    return text.trim();
  }

  function addMark(line, prompt, signature) {
    const marker = terminal.registerMarker(line - cursorLine());
    if (!marker) return null;

    const mark = { marker, prompt, signature, enteredAt: null, duration: null, decorations: [], label: null };
    // 滚出回滚区或被清屏删掉的行，标记会自己作废
    marker.onDispose(() => {
      clearDecorations(mark);
      const index = marks.indexOf(mark);
      if (index >= 0) marks.splice(index, 1);
      if (running === mark) running = null;
      if (pendingLabel === mark) pendingLabel = null;
    });
    marks.push(mark);
    marks.sort((a, b) => a.marker.line - b.marker.line);
    paintPrompt(mark);
    return mark;
  }

  function handleInput(data) {
    // 敲回车是单独一个 \r；粘贴一条带换行的命令时是「命令 + \r」一整块。
    // 多行粘贴会连着执行好几条，分不清哪行对哪条，干脆不记。
    const submits = data.indexOf("\r") === data.length - 1;
    if (data.length > 1) hasInput = true;
    if (!submits) {
      hasInput = true;
      return;
    }
    // 在提示符上直接敲回车只是换一行，不算一条命令
    if (!hasInput || !isNormalBuffer()) return;

    let line = cursorLine();
    // 长命令折行时，提示符在这组折行的第一行
    while (line > 0 && buffer().getLine(line)?.isWrapped) line -= 1;

    let mark = findMark(line);
    if (!mark) {
      const row = readTerminalRow(buffer(), line);
      if (!row?.text.trim()) return;
      // 没认出来的提示符（自定义的、REPL 的）也记时间，只是不上色
      mark = addMark(line, parseShellPrompt(row), row.text.trimEnd());
    }
    if (!mark) return;

    hasInput = false;
    mark.enteredAt = Date.now();
    mark.duration = null;
    running = mark;
    pendingLabel = mark;
  }

  function scheduleInspect() {
    clearTimeout(inspectTimer);
    inspectTimer = setTimeout(inspect, INSPECT_DELAY_MS);
  }

  function inspect() {
    if (!isNormalBuffer()) return;

    dropOverwrittenMarks();
    const line = cursorLine();
    // 回车的回显（换行）回来以后光标才会落到一个「不是折行续行」的新行上；
    // 长命令折行时光标早就在续行上了，只看行号会在最后几个字符回显之前就读走
    if (pendingLabel && line > pendingLabel.marker.line && !buffer().getLine(line)?.isWrapped) {
      const mark = pendingLabel;
      pendingLabel = null;
      // 回显落定后的整行才是这条命令的「指纹」，清屏后同一行被改写就能认出来
      mark.signature = readTerminalRow(buffer(), mark.marker.line)?.text.trimEnd() ?? mark.signature;
      paintLabel(mark);
      const command = mark.prompt && readCommand(mark);
      if (command) onCommand?.(command);
    }
    detectPrompt(line);
  }

  // 光标停在一个认得的提示符末尾、后面什么都没有：这是一个新的、等待输入的提示符
  function detectPrompt(line) {
    const row = readTerminalRow(buffer(), line);
    if (!row || row.isWrapped) return;
    const prompt = parseShellPrompt(row);
    if (!prompt || row.text.slice(prompt.end).trim() || buffer().cursorX < prompt.cells.x + prompt.cells.width) return;

    const existing = findMark(line);
    if (existing && existing.prompt && existing.enteredAt === null) return;
    // 同一行上已经执行过的旧标记：屏幕被清过、提示符又画回了这一行
    if (existing) discardMark(existing);

    const mark = addMark(line, prompt, row.text.slice(0, prompt.end));
    hasInput = false;
    if (running && running !== mark) {
      running.duration = Date.now() - running.enteredAt;
      if (running.duration >= DURATION_THRESHOLD_MS) paintLabel(running);
      running = null;
    }
    onPrompt?.(prompt);
  }

  // 命令可能折成好几行：把这组折行拼回一整行，再去掉前面的提示符
  function readCommand(mark) {
    let text = "";
    let line = mark.marker.line;
    do {
      text += readTerminalRow(buffer(), line)?.text ?? "";
      line += 1;
    } while (buffer().getLine(line)?.isWrapped);
    // 只去行尾：开头的空格有含义（bash 约定空格开头的命令不进历史），留给调用方判断
    return text.slice(mark.prompt.end + 1).trimEnd();
  }

  // clear / Ctrl+L 是原地擦屏，行还在、标记也还在，新内容写上来会被旧颜色染到。
  // 只有屏幕区的行可能被原地改写（回滚区只会被整行删掉），所以只查这一段。
  function dropOverwrittenMarks() {
    const top = buffer().baseY;
    for (const mark of [...marks]) {
      if (mark.marker.line < top) continue;
      const text = readTerminalRow(buffer(), mark.marker.line)?.text ?? "";
      if (!text.startsWith(mark.signature)) discardMark(mark);
    }
  }

  function paintPrompt(mark) {
    const { prompt } = mark;
    if (!prompt || !palette || !isNormalBuffer()) return;

    // 后注册的前景色覆盖先注册的：先把整段调暗，再单独点亮路径和 root
    const layers = [
      { ...prompt.cells, color: palette.dim },
      prompt.path && { ...prompt.path, color: palette.path },
      prompt.isRoot && prompt.user && { ...prompt.user, color: palette.root },
    ].filter(Boolean);
    for (const { x, width, color } of layers) {
      const decoration = terminal.registerDecoration({
        marker: mark.marker,
        x,
        width,
        foregroundColor: color,
      });
      if (decoration) mark.decorations.push(decoration);
    }
  }

  function paintLabel(mark) {
    mark.label?.dispose();
    mark.label = null;
    if (!mark.enteredAt || !palette || !isNormalBuffer()) return;

    const time = formatClock(mark.enteredAt);
    const duration = mark.duration >= DURATION_THRESHOLD_MS ? formatDuration(mark.duration) : "";
    const text = duration ? `${duration}  ${time}` : time;
    const row = readTerminalRow(buffer(), mark.marker.line);
    const used = row ? row.columns[row.text.trimEnd().length] : terminal.cols;
    if (terminal.cols - used < text.length + LABEL_GAP_CELLS) return;

    const decoration = terminal.registerDecoration({
      marker: mark.marker,
      anchor: "right",
      x: 1,
      width: text.length,
    });
    if (!decoration) return;

    // 排版写在自己套的一层里：xterm 每次渲染都会往装饰元素上写内联 display: block，
    // 直接给它设 flex 会被盖掉
    const content = document.createElement("div");
    content.className = "terminal-command-label";
    content.style.color = palette.label;
    content.style.fontFamily = terminal.options.fontFamily;
    // 比正文小一号：它是旁注，不该和命令抢视线
    content.style.fontSize = `${Math.round(terminal.options.fontSize * 0.86)}px`;
    content.append(
      ...(duration ? [labelPart(duration, "duration", palette.duration)] : []),
      labelPart(time, "time"),
    );
    decoration.onRender((element) => {
      if (element.firstChild !== content) element.replaceChildren(content);
    });
    mark.label = decoration;
  }

  // 必须先撤装饰、再销毁标记：xterm 的 Marker.dispose() 会先把 line 改成 -1 再通知装饰销毁，
  // 而装饰服务按 line 在有序列表里找要删的项——找不到就删不掉，还带着 -1 留在列表中间，
  // 把排序搅乱，之后按行号查装饰会漏掉别的行（表现为上一条命令的颜色突然没了）。
  function discardMark(mark) {
    clearDecorations(mark);
    mark.marker.dispose();
  }

  function clearDecorations(mark) {
    mark.decorations.forEach((decoration) => decoration.dispose());
    mark.decorations = [];
    mark.label?.dispose();
    mark.label = null;
  }

  function repaint() {
    for (const mark of marks) {
      clearDecorations(mark);
      paintPrompt(mark);
      paintLabel(mark);
    }
  }

  // 只在执行过的命令之间跳，把目标命令滚到第一行；往下跳过了最后一条就回到底部
  function jump(direction) {
    if (!isNormalBuffer()) return false;

    const top = buffer().viewportY;
    const lines = marks.filter((mark) => mark.enteredAt).map((mark) => mark.marker.line);
    const target = direction < 0
      ? lines.filter((line) => line < top).at(-1)
      : lines.find((line) => line > top);
    if (target === undefined) {
      if (direction > 0) terminal.scrollToBottom();
    } else {
      terminal.scrollToLine(target);
    }
    return true;
  }

  function setTheme(theme) {
    const { foreground, background, blue, magenta } = theme.xterm;
    palette = {
      dim: mixHex(foreground, background, 0.55),
      path: blue,
      root: magenta,
      label: mixHex(foreground, background, 0.42),
      duration: mixHex(foreground, background, 0.72),
    };
    repaint();
  }

  function dispose() {
    clearTimeout(inspectTimer);
    listeners.forEach((listener) => listener.dispose());
    [...marks].forEach(discardMark);
    inputStart?.marker.dispose();
    inputStart = null;
  }

  return { handleInput, noteKeystroke, readInput, jump, setTheme, dispose };
}

// 格子列号 → 这一行文字里的下标（宽字符占两格，两者对不上）
function textIndexAt(row, x) {
  const index = row.columns.findIndex((column) => column >= x);
  return index < 0 ? row.text.length : index;
}

function labelPart(text, kind, color) {
  const span = document.createElement("span");
  span.className = `terminal-command-${kind}`;
  span.textContent = text;
  if (color) span.style.color = color;
  return span;
}

function formatClock(timestamp) {
  const date = new Date(timestamp);
  return [date.getHours(), date.getMinutes(), date.getSeconds()]
    .map((part) => String(part).padStart(2, "0"))
    .join(":");
}

function formatDuration(ms) {
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ${seconds % 60}s`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}
