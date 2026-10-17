<div align="center">

<img src="src-tauri/icons/128x128@2x.png" width="120" alt="Xin SSH" />

# Xin SSH

像本地资源管理器一样操作远程服务器。

**简体中文** · [English](README.en.md)

![License](https://img.shields.io/badge/license-MIT-c4627a)
![Tauri](https://img.shields.io/badge/Tauri-2-24c8db)
![Vue](https://img.shields.io/badge/Vue-3-42b883)
![Rust](https://img.shields.io/badge/Rust-russh-b7410e)

</div>

---

市面上的 SSH 工具大致分两种：终端好用，但传文件要另开一个软件的；传文件好用，但界面还停在上个世纪的。

Xin SSH 想试试第三种。

在这里，文件管理不是终端旁边的附属品，而是主角。拖拽、双击、右键——你在自己电脑上怎么管文件，在服务器上就怎么管。终端当然也在，离你只有一次点击。

是的，图标是一个猫耳少女。不，这不影响它连服务器。

## 亮点

### 文件管理是主角，不是配角

- 路径栏、前进后退、多选、右键菜单，几万个文件的目录也不卡（虚拟滚动）
- 拖拽上传，整个目录一起拖也行；同名文件会问你覆盖、跳过还是改名
- 断点续传，传了一半断了不用从头再来
- 双击文件直接在内置编辑器里改，保存就写回服务器
- 权限用勾选框改，不用再心算 `rwxr-xr-x`

> 你当然可以继续用 vim 改配置。我们只是不再要求你这么做。

### 终端和文件面板，终于开始说话了

- 终端里 `cd /var/log`，文件面板跟着过去
- 终端输出里的路径按住 Ctrl / ⌘ 点一下：文件在编辑器里打开并跳到那一行（`main.go:42` 这种报错格外好用），目录在文件面板里打开
- 服务器打印的端口（比如 `listening on :8080`）点一下，自动转发到本机，网页直接在浏览器里打开
- 文件从面板拖到终端，路径自动填进命令行
- 文件从桌面拖到终端，上传到终端当前所在的目录，传完再把路径填进命令行

> 服务器打印的 `http://localhost:5173`，点开以后终于是服务器上的那个 5173 了。

### 生产机保护

给连接打开「生产环境」开关，之后在上面敲 `rm -rf`、`drop database` 这类命令，回车前会先弹个确认。粘贴多行命令也会拦一下。在 mysql 客户端里敲的也拦得住。

> 它拦不住你删库。但它会问一句「你确定吗」。很多事故，差的就是这一句。

### 记性比你好一点

- **命令联想**：打字时把历史里最近一条匹配的命令用灰字补在后面，按 → 采纳，和 fish 一个手感
- **命令标记**：提示符自动上色（`root` 会特别醒目），执行超过 3 秒的命令在右侧显示耗时，Ctrl + ↑ / ↓ 在命令之间跳
- **命令片段**：常用命令存起来，点一下就发
- **恢复会话**：重新打开应用，上次的标签页都还在

> 上周敲过的那条四十个字符的 docker 命令，它还记得。你忘了也没关系，这本来就不该是你的工作。

### 不在你的服务器上动手脚

上面这些全部在客户端完成：不装 agent，不改 `.bashrc`，不往 shell 里注入任何脚本。目录跟随靠的是 OSC 7 和识别提示符，监控靠的是只读命令。

> 你的服务器不会知道，它正被一个很好看的客户端连着。

### 安全上不偷懒

- 认证方式：密码、私钥（ed25519 / RSA / ECDSA，支持口令）、本机 SSH agent、keyboard-interactive 二次验证
- 首次连接记下主机指纹，之后指纹对不上就拒绝连接
- 保存的密码和私钥用 ChaCha20-Poly1305 加密，只存在你自己的电脑上
- 续传前核对已经传过去的部分，下载完、续传完再用 SHA-256 核对整个文件；服务器上有 `sha256sum` 就让服务器自己算，不用把文件再传一遍
- 配置文件原子写入，写到一半出意外也不会写坏

> 传完会算一遍 SHA-256。不是不信任你的网络，是不信任所有网络。

### 看得见服务器在干嘛

侧栏的主机概览：发行版、CPU 走势、内存、磁盘、上下行网速、进程和服务，刷新间隔可调。

> CPU 跑满的时候，你至少可以是第一个知道的人。

### 好看是认真的

- iOS 风格的液态玻璃界面，深色浅色都有，还能换壁纸
- 终端主题：自家的墨樱（Ink & Rose），以及 Catppuccin、Tokyo Night、Rosé Pine、Gruvbox Material、Nord、Vesper、Dracula、Poimandres，其中五套有明暗两版
- 七款等宽字体可选（JetBrains Mono、Fira Code、Geist Mono……），终端用 WebGL 渲染
- 基于 Tauri，用系统自带的 WebView，不额外打包一整个 Chromium

> 我们花在玻璃边缘那一道高光上的时间，比你想的多，也比我们愿意承认的多。

### 还有一些

- SSH 隧道（本地端口转发）
- 导入 `~/.ssh/config`，导出连接配置
- 同一个连接开多个终端
- 连接分组、收藏、最近使用、拖拽排序，自动识别服务器发行版并配上图标
- 断线自动重连
- 关窗保护：还有没保存的文件或正在传的任务时，关窗前先问你
- 中文 / English

> 传到 97% 的时候手滑点了关闭？它会拦住你。不客气。

## 下载

去 [Releases](https://github.com/ASAMUL/xin-ssh/releases/latest) 下载对应系统的安装包：Windows 选 `-setup.exe`；macOS 选 `.dmg`，Apple 芯片选 `aarch64`，Intel 选 `x64`；Linux 有 `AppImage`、`deb` 和 `rpm`。macOS 和 Linux 版是实验性的。

安装包暂时没有数字签名，所以第一次打开时：

- **Windows** 可能会弹出「Windows 已保护你的电脑」。它是好意，只是还不认识我们。点「更多信息」→「仍要运行」就行
- **macOS** 如果提示「已损坏，无法打开」，在终端里运行 `xattr -cr "/Applications/Xin SSH.app"` 再打开

## 现状

Xin SSH 还在早期（0.1.0），功能迭代很快，有 bug 很正常。

目前主要在 Windows 11 上开发和测试。Tauri 是跨平台的，所以 macOS 和 Linux 理论上也能跑。「理论上」这三个字有多重，你懂的。欢迎在 issue 里告诉我们实际情况。

## 从源码运行

需要先装好：

- Node.js 和 [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)（stable）
- Tauri 2 的系统依赖，见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)

```sh
pnpm install
pnpm tauri dev     # 开发模式
pnpm tauri build   # 打包安装程序
pnpm test          # 跑前端单元测试
```

## 技术栈

| 部分 | 用了什么 |
| --- | --- |
| 应用外壳 | Tauri 2 |
| SSH / SFTP | Rust：russh、russh-sftp、tokio |
| 界面 | Vue 3、Nuxt UI 4、Tailwind CSS 4、Pinia、vue-i18n |
| 终端 | xterm.js 6（WebGL 渲染） |
| 编辑器 | CodeMirror 6 |

## 参与贡献

欢迎提 issue 和 PR。

如果你发现了 bug：恭喜，它现在是我们共同的 bug 了。

## 致谢

Xin SSH 的很多代码，是和 [Claude](https://claude.com/claude-code)结对写出来的。这份 README 也是它写的。

## 许可证

[MIT](LICENSE)
