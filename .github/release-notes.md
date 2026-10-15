## 更新内容 / What's changed

<!-- 发布前在这里写这一版的改动 / Describe this release before publishing -->

## 下载 / Download

| 系统 / OS | 选这个文件 / Pick this file |
| --- | --- |
| Windows | `*_x64-setup.exe`（推荐 / recommended）或 / or `*_x64_en-US.msi` |
| macOS · Apple 芯片 / Apple silicon（实验性 / experimental） | `*_aarch64.dmg` |
| macOS · Intel（实验性 / experimental） | `*_x64.dmg` |
| Linux（实验性 / experimental） | `*_amd64.AppImage` / `*_amd64.deb` / `*.x86_64.rpm` |

## 首次打开 / First launch

安装包暂时没有数字签名。/ The installers are not code-signed yet.

- **Windows**：如果弹出「Windows 已保护你的电脑」，点「更多信息」→「仍要运行」。
  If SmartScreen says "Windows protected your PC", click "More info" → "Run anyway".
- **macOS**：如果提示「已损坏，无法打开」，在终端里运行下面这行，再打开即可。
  If macOS says the app is damaged, run this in Terminal and open it again:

  ```sh
  xattr -cr "/Applications/Xin SSH.app"
  ```
