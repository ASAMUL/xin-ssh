<div align="center">

<img src="src-tauri/icons/128x128@2x.png" width="120" alt="Xin SSH" />

# Xin SSH

Manage a remote server the way you manage files on your own computer.

[简体中文](README.md) · **English**

![License](https://img.shields.io/badge/license-MIT-c4627a)
![Tauri](https://img.shields.io/badge/Tauri-2-24c8db)
![Vue](https://img.shields.io/badge/Vue-3-42b883)
![Rust](https://img.shields.io/badge/Rust-russh-b7410e)

</div>

---

SSH clients tend to come in two flavours: great terminal, but you need a second app to move files; or great file transfer, wrapped in a UI that remembers the dial-up era fondly.

Xin SSH is trying a third option.

Here, file management isn't a side panel bolted onto a terminal. It's the main event. Drag, double-click, right-click: whatever you do with files on your own machine, you can do on the server. The terminal is still there, one click away.

Yes, the icon is a cat-eared girl. No, this does not affect her ability to connect to servers.

## Highlights

### Files are the main character

- Path bar, back and forward, multi-select, context menus. Directories with tens of thousands of entries stay smooth (virtual scrolling)
- Drag to upload, whole folders included. Name clashes get a polite question: overwrite, skip or rename
- Resumable transfers, so a dropped connection at 80% doesn't mean starting over
- Double-click a file to edit it in the built-in editor. Save, and it's written straight back to the server
- Change permissions with checkboxes instead of decoding `rwxr-xr-x` in your head

> You can keep editing config files in vim. We just no longer insist on it.

### The terminal and the file panel are finally on speaking terms

- `cd /var/log` in the terminal, and the file panel follows
- Ctrl / ⌘ + click a path in the terminal output: files open in the editor at that exact line (lovely for `main.go:42`-style errors), folders open in the file panel
- Click a port the server prints (say, `listening on :8080`) and it's forwarded to your machine and opened in your browser
- Drag a file from the panel onto the terminal and its path lands on the command line
- Drag a file from your desktop onto the terminal and it's uploaded to the terminal's current directory, with the path filled in once the upload finishes

> Click `http://localhost:5173` in the server's output and you get the server's 5173. Not yours. You'd be surprised how rare that is.

### Production guard

Flip a connection's "production" switch, and commands like `rm -rf` or `drop database` ask for confirmation before Enter goes through. Multi-line pastes get a second look too. It even works inside the mysql client.

> It can't stop you from dropping the database. It will, however, ask "are you sure?" first. A surprising number of incidents were one "are you sure?" away from not happening.

### A slightly better memory than yours

- **Suggestions**: as you type, the most recent matching command from your history appears in grey. Press → to accept. Same feel as fish
- **Command marks**: prompts are coloured (`root` stands out on purpose), commands that run longer than 3 seconds show their duration on the right, and Ctrl + ↑ / ↓ jumps between commands
- **Snippets**: save the commands you use all the time and send them with one click
- **Session restore**: reopen the app and your tabs are where you left them

> That 40-character docker command you typed last Tuesday? It remembers. You were never supposed to.

### Hands off your server

Everything above happens on the client. No agent to install, no edits to `.bashrc`, no scripts injected into your shell. Directory tracking reads OSC 7 and recognises your prompt. Monitoring runs read-only commands.

> Your server has no idea it's being connected to by a rather good-looking client.

### Security without shortcuts

- Auth: password, private key (ed25519 / RSA / ECDSA, passphrases supported), your local SSH agent, and keyboard-interactive / 2FA
- Host fingerprints are recorded on first connect. If one changes later, the connection is refused
- Saved passwords and keys are encrypted with ChaCha20-Poly1305 and never leave your computer
- Before resuming, the part already transferred is checked. After a download or a resumed upload, the whole file is checked with SHA-256, computed on the server when it has `sha256sum`, so the file doesn't cross the network twice
- Config files are written atomically, so an untimely crash can't leave them half-written

> Every transfer ends with a SHA-256 check. It's nothing personal against your network. We don't trust any network.

### See what your server is up to

A host overview in the sidebar: distro, CPU history, memory, disk, upload and download speed, processes and services. The refresh interval is up to you.

> When the CPU hits 100%, you can at least be the first to know.

### We take looking good seriously

- An iOS-style liquid glass interface, in light and dark, with wallpapers if you want them
- Terminal themes: our own Ink & Rose, plus Catppuccin, Tokyo Night, Rosé Pine, Gruvbox Material, Nord, Vesper, Dracula and Poimandres. Five of them come in light and dark
- Seven monospace fonts to choose from (JetBrains Mono, Fira Code, Geist Mono, …), with WebGL terminal rendering
- Built on Tauri with the system WebView, so it doesn't ship an entire copy of Chromium

> We spent more time on the highlight along the glass edges than you'd think. More than we'd like to admit, too.

### And a few more things

- SSH tunnels (local port forwarding)
- Import from `~/.ssh/config` (hosts behind a jump host are skipped for now), export your connections
- Multiple terminals on the same connection
- Groups, favourites, recents and drag-to-reorder for connections, with distro icons detected automatically
- Automatic reconnect when the connection drops
- Close protection: unsaved files or running transfers get a confirmation before the window closes
- English / 中文

> Hit close at 97% of a transfer? It'll stop you. Don't mention it.

## Download

Grab the installer for your system from [Releases](https://github.com/ASAMUL/xin-ssh/releases/latest): `-setup.exe` for Windows; `.dmg` for macOS, `aarch64` for Apple silicon and `x64` for Intel; `AppImage`, `deb` or `rpm` for Linux. The macOS and Linux builds are experimental.

The installers aren't code-signed yet, so on first launch:

- **Windows** may say "Windows protected your PC". It means well; it just hasn't met us yet. Click "More info" → "Run anyway"
- **macOS** may claim the app is damaged. Run `xattr -cr "/Applications/Xin SSH.app"` in Terminal, then open it again

## Status

Xin SSH is early (0.1.0) and changing fast. Bugs are to be expected.

It's developed and tested mainly on Windows 11. Tauri is cross-platform, so macOS and Linux should work in theory. You know how much weight "in theory" carries. If you try it, please tell us how it went in an issue.

## Build from source

You'll need:

- Node.js and [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- The Tauri 2 system dependencies, see the [Tauri docs](https://v2.tauri.app/start/prerequisites/)

```sh
pnpm install
pnpm tauri dev     # run in development mode
pnpm tauri build   # build the installer
pnpm test          # run the frontend unit tests
```

## Tech stack

| Part | What it uses |
| --- | --- |
| App shell | Tauri 2 |
| SSH / SFTP | Rust: russh, russh-sftp, tokio |
| UI | Vue 3, Nuxt UI 4, Tailwind CSS 4, Pinia, vue-i18n |
| Terminal | xterm.js 6 (WebGL renderer) |
| Editor | CodeMirror 6 |

## Contributing

Issues and pull requests are welcome.

Found a bug? Congratulations, it's our bug now. Both of ours.

## Acknowledgements

A lot of Xin SSH was pair-programmed with [Claude](https://claude.com/claude-code). Claude wrote this README, too.

## License

[MIT](LICENSE)
