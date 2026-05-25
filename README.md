# Xin Terminal

Xin Terminal is a modern SSH client focused on visual remote file management. The product direction is: operate a remote server like a local file explorer, while keeping terminal access one click away.

## Current Slice

- Vue 3 desktop workspace shell matching the product reference: connection sidebar, session tabs, file explorer, terminal dock, transfer queue, and status bar.
- Tauri command skeleton for connection profiles, sessions, remote directory listing, transfer tasks, and terminal snapshots.
- Mock data is intentionally isolated so the next phase can replace it with real SSH/SFTP state without reshaping the UI.

## Development

```sh
pnpm install
pnpm dev
pnpm tauri dev
```

See [docs/plan.md](docs/plan.md) for the full product and architecture plan.
