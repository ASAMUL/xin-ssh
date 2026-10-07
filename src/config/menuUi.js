// 文件列表、传输历史、终端这类工作区里的右键菜单共用一套紧凑皮肤；玻璃材质由全局 .liquid-menu 提供
export const compactMenuUi = {
  content: "min-w-40 rounded-lg border border-default bg-default shadow-xl backdrop-blur-xl",
  viewport: "divide-y divide-default",
  group: "p-1",
  separator: "my-1 bg-border",
  item: "min-h-8 items-center gap-2 rounded-md px-2 py-1.5 text-xs font-semibold leading-none",
  itemLeadingIcon: "size-4 w-4 shrink-0 text-muted",
  itemWrapper: "min-w-0 flex-1 justify-center",
  itemLabel: "truncate leading-4",
  itemTrailing: "ms-auto h-4 items-center",
  itemTrailingIcon: "size-4",
  // 默认在 lg 以下藏起快捷键；桌面窗口拖窄了也该看得到
  itemTrailingKbds: "inline-flex items-center shrink-0 ps-4",
};
