// CSS 令牌体检。引用一个**未定义**的自定义属性时，CSS 不会报错——整条声明
// 直接作废，界面静默地少一个圆角、少一层描边，很难靠肉眼发现（本仓库就这么
// 藏了一个 --xin-radius-md，从引入到发现隔了两个月）。所以把它做成可重复的检查。
//
// 用法：node scripts/check-css-tokens.mjs

import { readFileSync } from "node:fs";
import { execSync } from "node:child_process";

// 令牌定义可以散在多个文件：全局设计令牌在 main.css，编辑器画布自成一套
const DEFINITION_FILES = ["src/assets/css/main.css", "src/assets/css/code-editor.css"];

const files = execSync("git ls-files src", { encoding: "utf8" })
  .trim()
  .split("\n")
  .filter((file) => /\.(css|vue)$/.test(file));

const defined = new Set();
for (const file of DEFINITION_FILES) {
  for (const match of readFileSync(file, "utf8").matchAll(/^\s*(--xin-[a-z0-9-]+)\s*:/gm)) {
    defined.add(match[1]);
  }
}

const used = new Map();
for (const file of files) {
  const source = readFileSync(file, "utf8");
  for (const match of source.matchAll(/var\((--xin-[a-z0-9-]+)/g)) {
    if (!used.has(match[1])) used.set(match[1], []);
    used.get(match[1]).push(file);
  }
}

const missing = [...used].filter(([token]) => !defined.has(token));

console.log(`已定义 ${defined.size} 个令牌，被引用 ${used.size} 个`);

if (missing.length) {
  console.log("\n✗ 引用了未定义的令牌（这些声明在浏览器里是直接作废的）：");
  for (const [token, where] of missing) {
    console.log(`    ${token}  ←  ${[...new Set(where)].join(", ")}`);
  }
} else {
  console.log("✓ 没有悬空引用");
}

process.exitCode = missing.length ? 1 : 0;
