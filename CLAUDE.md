# Conch — Claude Instructions

## Critical Engineering Standards

### 1. Unit Tests Are Required
Every new function, module, or behavior change MUST have unit tests if at all possible. The project already has `#[cfg(test)]` modules in most files — follow that pattern. If adding a new `.rs` file, add a `#[cfg(test)] mod tests` at the bottom. Pure logic, parsers, config handling, widget building — all testable without a GUI context. Only skip tests for code that truly requires a live Tauri context or OS-level resources.

### 2. Modularity — No Monoliths
Code MUST be broken into small, focused modules. When adding new functionality:
- Extract into its own file/module
- Group related files into subdirectories with `mod.rs` (e.g., `remote/`, `plugins/`)
- Each file should have a single responsibility
- Prefer many small files over few large files
- New features go in new modules, not appended to existing large files

## Git Workflow (STRICT)

- **Claude must never commit or push directly to `main`.**
- The repo owner (`an0nn30`) may push directly to `main` when appropriate.
- Every feature, fix, or change — no matter how small — must go on its own branch.
- Branch naming convention:
  - `feat/short-description` — new features
  - `fix/short-description` — bug fixes
  - `chore/short-description` — docs, config, tooling, cleanup
  - `perf/short-description` — performance improvements
- Before starting any work, check the current branch. If on `main`, create a new branch first.
- Push the branch to origin. Never open PRs unless the user explicitly asks.
- Never use `--force` push.

## Commit Rules

- Never add Co-Authored-By lines to commits.
- Write concise, descriptive commit messages in the imperative mood.
- PRs should be small and focused — one concern per PR.
- This is a public, open-source repo. Be thoughtful about what goes into commits.

## Architecture

### Workspace Structure
```
crates/
  conch_core/         — Config loading, color schemes, persistent state
  conch_plugin_sdk/   — Widget/event types shared with Lua and Java plugins
  conch_plugin/       — Plugin host: message bus, Lua runner, Java runtime, HostApi trait
  conch_tauri/        — The app: Tauri/xterm.js UI, SSH, SFTP, file explorer, tunnels
java-sdk/             — Java Plugin SDK: HostApi, ConchPlugin, Widgets, PluginInfo
editors/
  vscode/             — VS Code extension for Lua plugin development
```

### conch_tauri Module Layout
```
src/
  main.rs             — Entry point, config loading, launches Tauri app
  lib.rs              — Tauri app setup, menu building, Tauri commands, window management
  pty_backend.rs      — Local PTY via portable-pty (raw byte I/O, xterm.js handles emulation)
  ipc.rs              — Unix socket IPC listener (conch msg new-tab/new-window)
  watcher.rs          — File watcher for config/theme hot-reload
  remote/             — Unified SSH + SFTP + file operations
    mod.rs            — Session registry, SSH/SFTP/local FS Tauri commands, auth prompt bridge
    ssh.rs            — SSH connection (russh), auth, proxy, channel I/O loop
    sftp.rs           — SFTP operations (list, stat, read, write, mkdir, rename, remove)
    local_fs.rs       — Local filesystem operations (same interface as sftp.rs)
    config.rs         — Server entries, folders, tunnels, ~/.ssh/config import, persistence
    known_hosts.rs    — OpenSSH known_hosts read/write
    transfer.rs       — Upload/download engine with progress events
    tunnel.rs         — SSH tunnel manager (local port forwarding)
  plugins/            — Plugin integration for Tauri
    mod.rs            — PluginState, plugin discovery, enable/disable, Tauri commands
    tauri_host_api.rs — TauriHostApi implementing the HostApi trait for the webview UI
frontend/
  index.html          — Main HTML/CSS, xterm.js terminal, tab management, layout
  ssh-panel.js        — SSH server tree, quick connect, folder/tunnel management
  files-panel.js      — Dual-pane file explorer (local + remote)
  tunnel-manager.js   — SSH tunnel CRUD dialog
  plugin-widgets.js   — Widget JSON → HTML renderer, plugin dialog handlers
  plugin-manager.js   — Plugin discovery/enable/disable dialog
  toast.js            — Global toast notification system
```

### Plugin Architecture (2 tiers)
- **Java** (Java/Kotlin/Scala): `.jar` files loaded by embedded JVM, communicate via JNI → safe HostApi trait
- **Lua**: single `.lua` files, communicate via mlua → safe HostApi trait
- Both tiers share: declarative widget system (JSON), pub/sub event bus, config persistence, dialog APIs
- Plugin config persistence via `HostApi::get_config`/`set_config` (JSON files per plugin)
- No native/C ABI plugins — SSH, SFTP, and file browsing are built directly into `conch_tauri`

### Key Patterns
- Tauri webview: HTML/CSS/JS frontend, Rust backend communicating via Tauri commands and events
- xterm.js handles all terminal emulation — the backend provides raw byte streams
- Plugin bus: pub/sub event system for plugin↔plugin and plugin↔app communication
- `query_plugin`: direct queries between plugins (JSON over mpsc channels)
- Panel registry: plugins register panels at locations (Left, Right, Bottom)
- SSH sessions reuse the same `pty-output`/`pty-exit` events as local PTY tabs
- Auth prompts use oneshot channels: emit event to frontend, block plugin thread on response
- Theme colors applied via CSS custom properties, loaded from Alacritty .toml theme files

## Style Guide

### Rust
- Use `pub(crate)` for internal visibility, not `pub` (unless it's a library API)
- Prefer `if let` / `match` over `.unwrap()` — handle errors gracefully
- Use `log::error!`/`log::warn!` for recoverable errors, not panics
- `#[serde(default)]` on config structs for backward compatibility
- Keep `unsafe` blocks minimal and well-commented
- No unnecessary `clone()` — borrow where possible

### Frontend (JS)
- Each JS module is a self-contained IIFE exposing a global (e.g., `window.sshPanel`)
- Use the global `toast.js` system for all notifications — no `alert()`
- CSS uses custom properties (`var(--bg)`, `var(--fg)`) loaded from the theme
- Overlay dialogs use the `ssh-overlay` / `ssh-form` CSS pattern

### Config
- User config: `config.toml` (loaded by conch_core)
- Persistent state: `state.toml` (window size, loaded plugins, layout)
- SSH server config: `~/.config/conch/remote/servers.json`
- Plugin config: `~/.config/conch/plugins/{plugin_name}/{key}.json`
- Keyboard shortcuts: configurable in `[conch.keyboard]` section
- Default shortcuts use `cmd+` prefix (maps to Cmd on macOS, Ctrl on Linux/Windows)

### Testing Standards
- `#[cfg(test)] mod tests` at the bottom of each file
- Test pure logic: parsing, config defaults, widget building
- Use `assert_eq!` with descriptive messages
- Test edge cases: empty input, missing fields, boundary values
- Plugin SDK: test widget serialization/deserialization
- Config: test defaults, serde round-trips, backward compat with `serde(default)`
