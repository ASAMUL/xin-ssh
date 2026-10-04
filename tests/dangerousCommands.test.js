// 生产机保护的危险命令规则。漏拦一次就是生产事故，改规则时先跑这里：pnpm test
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { findDangerousCommand, historySearchCommand, pastedLines } from "../src/utils/dangerousCommands.js";

const kindOf = (command) => findDangerousCommand(command)?.kind ?? null;

describe("findDangerousCommand", () => {
  it("认出各种写法的 rm -rf", () => {
    for (const command of [
      "rm -rf /data",
      "rm -fr /data",
      "rm -Rf /data",
      "rm -r -f /data",
      "rm --recursive --force /data",
      "rm --no-preserve-root -rf /",
      "sudo rm -rf /data",
      "sudo -u deploy rm -rf /data",
      "nohup rm -rf /data &",
      "FOO=1 rm -rf /data",
      "/bin/rm -rf /data",
      "cd /tmp && rm -rf build",
      "ls; rm -rf build",
      "find . -name '*.tmp' | xargs rm -rf",
    ]) {
      assert.equal(kindOf(command), "recursiveDelete", command);
    }
  });

  it("绕开 alias 和加引号的 rm 也拦：\\rm 是 CentOS / RHEL 上跳过 rm -i 的习惯写法", () => {
    for (const command of ["\\rm -rf /data", "'rm' -rf /data", "\"rm\" -rf /data", "sudo \\rm -rf /data", "\\sudo rm -rf /data"]) {
      assert.equal(kindOf(command), "recursiveDelete", command);
    }
  });

  it("只是提到 rm -rf、或者缺了 -r / -f 的，不拦", () => {
    for (const command of [
      "echo 'rm -rf /'",
      "grep 'rm -rf' deploy.log",
      "rm -r build",
      "rm -f app.pid",
      "ls -la",
      "truncate -s 0 /var/log/nginx/access.log",
    ]) {
      assert.equal(kindOf(command), null, command);
    }
  });

  it("关机、格式化、清 crontab、清防火墙、递归改根目录权限", () => {
    assert.equal(kindOf("reboot"), "shutdown");
    assert.equal(kindOf("sudo shutdown -h now"), "shutdown");
    assert.equal(kindOf("systemctl poweroff"), "shutdown");
    assert.equal(kindOf("init 6"), "shutdown");
    assert.equal(kindOf("systemctl restart nginx"), null);
    assert.equal(kindOf("mkfs.ext4 /dev/sdb1"), "formatDisk");
    assert.equal(kindOf("dd if=/dev/zero of=/dev/sda bs=1M"), "formatDisk");
    assert.equal(kindOf("dd if=/dev/zero of=./disk.img bs=1M"), null);
    assert.equal(kindOf("crontab -r"), "clearCrontab");
    assert.equal(kindOf("crontab -l"), null);
    assert.equal(kindOf("iptables -F"), "flushFirewall");
    assert.equal(kindOf("chmod -R 777 /"), "recursiveRootPermission");
    assert.equal(kindOf("chown -R www:www /"), "recursiveRootPermission");
    assert.equal(kindOf("chmod -R 755 /var/www"), null);
  });

  it("SQL 整行搜：mysql 客户端里、mysql -e 的引号里都拦", () => {
    assert.equal(kindOf("drop database prod;"), "dropDatabase");
    assert.equal(kindOf("mysql> DROP TABLE users;"), "dropDatabase");
    assert.equal(kindOf("mysql -uroot -e 'drop database prod'"), "dropDatabase");
    assert.equal(kindOf("truncate table orders;"), "dropDatabase");
    assert.equal(kindOf("select * from drops;"), null);
  });

  it("标出命中的那一段", () => {
    assert.deepEqual(findDangerousCommand("cd /srv && sudo rm -rf old"), {
      kind: "recursiveDelete",
      segment: "sudo rm -rf old",
    });
  });
});

describe("historySearchCommand", () => {
  it("从 bash 的 Ctrl+R 搜索框里取出回车要执行的命令", () => {
    assert.equal(historySearchCommand("(reverse-i-search)`rm': rm -rf /data"), "rm -rf /data");
    assert.equal(historySearchCommand("(failed reverse-i-search)`rmx': rm -rf /data"), "rm -rf /data");
    assert.equal(historySearchCommand("(i-search)`boot': reboot"), "reboot");
    assert.equal(kindOf(historySearchCommand("(reverse-i-search)`rm': rm -rf /data")), "recursiveDelete");
  });

  it("不是搜索框返回 null", () => {
    assert.equal(historySearchCommand("root@web:~# rm -rf /data"), null);
    assert.equal(historySearchCommand(""), null);
  });
});

describe("pastedLines", () => {
  it("不带换行不用确认；带换行逐行预览、去掉末尾空行", () => {
    assert.equal(pastedLines("ls -la"), null);
    assert.deepEqual(pastedLines("cd /srv\r\nrm -rf old\n\n"), ["cd /srv", "rm -rf old"]);
  });
});
