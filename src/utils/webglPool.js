// 终端的 WebGL 上下文池。
//
// 浏览器同时能开的 WebGL 上下文有限（Chromium 是 16 个，超了最老的那个直接丢失），每个上下文也占显存，
// 所以不能每个终端都一直占着。以前的做法是同一时刻只留当前可见的那一个——代价是每切一次标签，
// 新终端都要现建上下文、编译着色器、重建字形图集，旧终端又退回 DOM 渲染器整屏重画，一次切换卡上百毫秒。
// 现在留最近看过的几个：在它们之间来回切不用重建，超出时让最久没看的那个让出来。
// 不可见的终端 xterm 自己会暂停绘制，留着上下文不耗 GPU。
const MAX_CONTEXTS = 4;

// 每个终端一个 holder（{ release }），最近用过的排在最后
const holders = [];

export function touchWebgl(holder) {
  forgetWebgl(holder);
  holders.push(holder);
  while (holders.length > MAX_CONTEXTS) holders.shift().release();
}

export function forgetWebgl(holder) {
  const index = holders.indexOf(holder);
  if (index >= 0) holders.splice(index, 1);
}
