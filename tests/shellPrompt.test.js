// 提示符识别：命令标记上色、文件面板跟随终端目录、生产机保护去掉提示符都靠它
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parsePromptBeforeCursor, parseShellPrompt } from "../src/utils/shellPrompt.js";

// 测试里都是窄字符，第 i 个字符就落在第 i 格
const row = (text) => ({ text, columns: [...Array(text.length + 1).keys()] });
const commandAfterPrompt = (text) => {
  const prompt = parseShellPrompt(row(text));
  return prompt ? text.slice(prompt.end + 1) : null;
};

describe("parseShellPrompt", () => {
  it("认得发行版的默认提示符，并读出用户、主机、路径", () => {
    assert.deepEqual(
      (({ userName, hostName, pathText, pathStyle, isRoot }) => ({ userName, hostName, pathText, pathStyle, isRoot }))(
        parseShellPrompt(row("root@web-01:/var/log# ls")),
      ),
      { userName: "root", hostName: "web-01", pathText: "/var/log", pathStyle: "full", isRoot: true },
    );
    assert.equal(parseShellPrompt(row("[deploy@web-01 log]$ ls")).pathStyle, "basename");
    assert.equal(parseShellPrompt(row("web-01:~# ls")).hostName, "web-01");
  });

  it("去掉提示符后剩下的就是命令，带 (venv) / (base) 前缀的也一样", () => {
    assert.equal(commandAfterPrompt("root@web:~# rm -rf /data"), "rm -rf /data");
    assert.equal(commandAfterPrompt("[root@web ~]# rm -rf /data"), "rm -rf /data");
    assert.equal(commandAfterPrompt("(base) root@web:~# rm -rf /data"), "rm -rf /data");
  });

  // 认不出的提示符宁可漏认、不误认，所以生产机保护不能只靠这里——
  // 它还会从开始按键的那一格往后读（commandMarks 的 readInput）
  it("自定义提示符认不出来", () => {
    for (const text of ["[PROD] root@web:~# rm -rf /data", "➜  ~ rm -rf /data", "❯ rm -rf /data", "web% rm -rf /data"]) {
      assert.equal(parseShellPrompt(row(text)), null, text);
    }
  });
});

describe("parsePromptBeforeCursor", () => {
  // 光标停在提示符后面那个空格之后
  const waiting = (text) => [row(text), text.length];

  it("提示符从行首开始时和 parseShellPrompt 一样", () => {
    const [line, cursor] = waiting("root@web:/home# ");
    assert.equal(parsePromptBeforeCursor(line, cursor).end, parseShellPrompt(line).end);
  });

  it("紧贴在没换行的输出后面也认得出，用认识的用户名分开粘在一起的字", () => {
    const [line, cursor] = waiting("/dataroot@VM-4-9-ubuntu:/home# ");
    const prompt = parsePromptBeforeCursor(line, cursor, ["root"]);
    assert.deepEqual(
      { user: prompt.userName, host: prompt.hostName, path: prompt.pathText, x: prompt.cells.x },
      { user: "root", host: "VM-4-9-ubuntu", path: "/home", x: 5 },
    );
    assert.equal(line.text.slice(0, prompt.cells.x), "/data");
  });

  it("用户名都不认识时取最长的那个", () => {
    const [line, cursor] = waiting("/dataroot@web:/home# ");
    assert.equal(parsePromptBeforeCursor(line, cursor).userName, "dataroot");
  });

  it("RHEL 的方括号提示符本身就分得开", () => {
    const [line, cursor] = waiting("done[root@web ~]# ");
    const prompt = parsePromptBeforeCursor(line, cursor);
    assert.deepEqual({ user: prompt.userName, x: prompt.cells.x }, { user: "root", x: 4 });
  });

  it("提示符后面到光标之间还有字的，不是等待输入的提示符", () => {
    const text = "/dataroot@web:/home# ls";
    assert.equal(parsePromptBeforeCursor(row(text), text.length, ["root"]), null);
  });
});
