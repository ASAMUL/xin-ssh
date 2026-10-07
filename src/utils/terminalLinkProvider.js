// 把 terminalLinks.js 认出的路径、端口接到 xterm 上：悬停出下划线，Ctrl / ⌘ + 点击才打开。
// 普通点击留给选中文字和聚焦终端——满屏输出里到处是路径，单击就跳走会很烦。
// 长路径会折行，所以按整组折行拼成一行再找，找到的字符区间换算回格子坐标（宽字符占两格）。
import { readTerminalRow } from "./shellPrompt";
import { findTerminalLinks } from "./terminalLinks";

export function registerTerminalLinks(terminal, { activate, hover, leave }) {
  return terminal.registerLinkProvider({
    provideLinks(bufferLineNumber, callback) {
      const buffer = terminal.buffer.active;
      let start = bufferLineNumber - 1;
      while (start > 0 && buffer.getLine(start)?.isWrapped) start -= 1;

      // 每个字符落在哪一格、占几格
      const cells = [];
      let text = "";
      let line = start;
      do {
        const row = readTerminalRow(buffer, line);
        if (!row) break;
        for (let index = 0; index < row.text.length; index += 1) {
          cells.push({ x: row.columns[index], y: line, width: row.columns[index + 1] - row.columns[index] });
        }
        text += row.text;
        line += 1;
      } while (buffer.getLine(line)?.isWrapped);

      const links = findTerminalLinks(text)
        .map((link) => {
          const first = cells[link.start];
          const last = cells[link.end - 1];
          return {
            range: {
              start: { x: first.x + 1, y: first.y + 1 },
              end: { x: last.x + Math.max(last.width, 1), y: last.y + 1 },
            },
            text: text.slice(link.start, link.end),
            activate: (event) => {
              if (event.ctrlKey || event.metaKey) activate(link, first.y);
            },
            hover: (event) => hover?.(link, event),
            leave: () => leave?.(link),
          };
        })
        // xterm 按行来要链接，只交出落在这一行上的
        .filter(({ range }) => range.start.y <= bufferLineNumber && range.end.y >= bufferLineNumber);
      callback(links.length ? links : undefined);
    },
  });
}
