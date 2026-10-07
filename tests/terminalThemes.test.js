// xterm 解析不了的装饰颜色（比如 CSS 变量）会让整帧渲染抛错，终端查找就既不高亮也看不到跳转
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { TERMINAL_THEMES, buildSearchDecorations } from "../src/config/terminalThemes.js";

const HEX_COLOR = /^#[0-9a-f]{6}$/i;

describe("buildSearchDecorations", () => {
  for (const theme of TERMINAL_THEMES) {
    for (const [mode, variant] of Object.entries(theme.variants)) {
      it(`${theme.id} ${mode} 的查找高亮都是 #RRGGBB`, () => {
        for (const [key, color] of Object.entries(buildSearchDecorations(variant))) {
          assert.match(color, HEX_COLOR, key);
        }
      });
    }
  }
});
