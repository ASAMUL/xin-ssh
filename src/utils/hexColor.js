// xterm 的装饰颜色只认 #RRGGBB，调暗只能在 JS 里先混好，不能交给 CSS 的 color-mix
export function mixHex(foreground, background, weight) {
  const channels = (hex) => [1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16));
  const front = channels(foreground);
  const back = channels(background);
  return `#${front
    .map((value, index) => Math.round(value * weight + back[index] * (1 - weight)))
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("")}`;
}
