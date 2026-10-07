// 终端里可点的路径和端口：认错一个就是一条误导人的下划线
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { findTerminalLinks } from "../src/utils/terminalLinks.js";
import { formatPathsForShell } from "../src/utils/remotePath.js";

const links = (text) => findTerminalLinks(text).map((link) => ({ ...link, text: text.slice(link.start, link.end) }));
const paths = (text) => links(text).filter((link) => link.kind === "path");
const ports = (text) => links(text).filter((link) => link.kind === "port").map((link) => link.port);

describe("路径", () => {
  it("绝对路径、~/、./ ../，带不带行列号都认", () => {
    assert.deepEqual(
      paths("tail -f /var/log/nginx/error.log").map(({ path, line }) => ({ path, line })),
      [{ path: "/var/log/nginx/error.log", line: null }],
    );
    const [withPosition] = paths("    at handler (/srv/app/index.js:42:7)");
    assert.deepEqual(
      { path: withPosition.path, line: withPosition.line, column: withPosition.column, text: withPosition.text },
      { path: "/srv/app/index.js", line: 42, column: 7, text: "/srv/app/index.js:42:7" },
    );
    assert.equal(paths("see ~/logs/app.log")[0].path, "~/logs/app.log");
    assert.equal(paths("run ./deploy.sh now")[0].path, "./deploy.sh");
    assert.equal(paths("cat ../conf/app.yml")[0].path, "../conf/app.yml");
    assert.equal(paths("ls /etc/nginx/")[0].path, "/etc/nginx/");
  });

  it("grep -rn 和编译报错里的相对路径、带行号的文件名", () => {
    const [grep] = paths("src/server/app.js:128:  const port = 8080;");
    assert.deepEqual({ path: grep.path, line: grep.line, text: grep.text }, { path: "src/server/app.js", line: 128, text: "src/server/app.js:128" });
    const [compiler] = paths("main.go:42:7: undefined: foo");
    assert.deepEqual({ path: compiler.path, line: compiler.line, column: compiler.column }, { path: "main.go", line: 42, column: 7 });
  });

  it("Python 报错和引号里带空格的路径", () => {
    const [python] = paths('  File "/srv/app/main.py", line 42, in <module>');
    assert.deepEqual({ path: python.path, line: python.line, text: python.text }, { path: "/srv/app/main.py", line: 42, text: "/srv/app/main.py" });
    assert.equal(paths("cd '/var/www/my site'")[0].path, "/var/www/my site");
  });

  it("提示符紧贴在没换行的输出后面时，不把输出和提示符连成一个路径", () => {
    assert.deepEqual(paths("/dataroot@VM-4-9-ubuntu:/home# cat 1.txt").map((link) => link.path), ["/home"]);
  });

  it("句末标点不算路径", () => {
    assert.equal(paths("edit /etc/hosts.")[0].path, "/etc/hosts");
  });

  it("不是路径的不认：URL、域名、版本号、IP、普通词、斜杠分隔的词", () => {
    for (const text of [
      "https://github.com/user/repo/blob/main/a.js",
      "visit example.com:8080 today",
      "upgrade to v1.2.3",
      "connect 10.0.0.5:3306",
      "and/or TCP/IP km/h",
      "2026/10/05 10:20:30",
      "just some words",
      "/",
    ]) {
      assert.deepEqual(paths(text), [], text);
    }
  });
});

describe("端口", () => {
  it("ss / netstat 里服务器自己监听的端口", () => {
    assert.deepEqual(ports("LISTEN 0 511 0.0.0.0:80 0.0.0.0:*"), [80]);
    assert.deepEqual(ports("tcp6 0 0 :::8080 :::* LISTEN"), [8080]);
    assert.deepEqual(ports("LISTEN 0 128 [::]:443 [::]:*"), [443]);
    assert.deepEqual(ports("LISTEN 0 4096 127.0.0.1:6379 0.0.0.0:*"), [6379]);
    assert.deepEqual(ports("LISTEN 0 511 *:3000 *:*"), [3000]);
  });

  it("服务启动日志", () => {
    assert.deepEqual(ports("Server listening on port 3000"), [3000]);
    assert.deepEqual(ports("PORT=8080 node server.js"), [8080]);
    assert.deepEqual(ports("Listening on :8080"), [8080]);
    assert.deepEqual(ports("ready - started server on 0.0.0.0:3000"), [3000]);
  });

  it("下划线只盖端口号", () => {
    assert.equal(links("Listening on 127.0.0.1:5173")[0].text, "5173");
  });

  it("别的机器、URL 里的、越界的不认", () => {
    assert.deepEqual(ports("connect 10.0.0.5:3306"), []);
    assert.deepEqual(ports("open http://localhost:5173/"), []);
    assert.deepEqual(ports("port 99999"), []);
  });
});

describe("formatPathsForShell", () => {
  it("普通路径原样，带空格和引号的才加引号，末尾留空格", () => {
    assert.equal(formatPathsForShell(["/var/log/app.log"]), "/var/log/app.log ");
    assert.equal(formatPathsForShell(["/srv/my site", "/tmp/it's"]), "'/srv/my site' '/tmp/it'\\''s' ");
    assert.equal(formatPathsForShell([]), "");
  });
});
