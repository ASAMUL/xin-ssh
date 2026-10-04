// 提示符识别：命令标记上色、文件面板跟随终端目录、生产机保护去掉提示符都靠它
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseShellPrompt } from "../src/utils/shellPrompt.js";

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
