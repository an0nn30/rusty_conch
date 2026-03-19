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
- `lib.rs` should delegate to submodules — avoid growing it beyond ~1000 lines

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

### Workspace (4 crates, no egui, no native plugins)
```
crates/
  conch_core/         — Config loading, color schemes, persistent state
  conch_plugin_sdk/   — Widget/event types shared with Lua and Java plugins
  conch_plugin/       — Plugin host: message bus, Lua runner, Java runtime, HostApi trait
  conch_tauri/        — The app: Tauri v2 / xterm.js UI, built-in SSH/SFTP/tunnels
java-sdk/             — Java Plugin SDK: HostApi, ConchPlugin, Widgets, PluginInfo
editors/
  vscode/             — VS Code extension for Lua plugin development
```

### conch_tauri — The App
```
src/
  main.rs             — Entry point, config loading, launches Tauri app
  lib.rs              — Tauri setup, commands, menu building, window management
  theme.rs            — Color theme loading (Alacritty .toml → CSS variables)
  pty_backend.rs      — Local PTY via portable-pty (raw byte I/O for xterm.js)
  ipc.rs              — Unix socket IPC listener (conch msg new-tab/new-window)
  watcher.rs          — File watcher for config/theme hot-reload
  remote/             — Built-in SSH + SFTP + file operations (not a plugin)
    mod.rs            — Session registry, Tauri commands, auth prompt bridge
    ssh.rs            — SSH connection (russh 0.48), auth, proxy, channel I/O
    sftp.rs           — SFTP operations via russh-sftp
    local_fs.rs       — Local filesystem operations (same FileEntry interface)
    config.rs         — Server entries, folders, tunnels, ~/.ssh/config import
    known_hosts.rs    — OpenSSH known_hosts read/write
    transfer.rs       — Upload/download engine with progress events
    tunnel.rs         — SSH tunnel manager (local port forwarding)
  plugins/            — Plugin integration for Tauri
    mod.rs            — PluginState, discovery, enable/disable, dialog responses
    tauri_host_api.rs — TauriHostApi implementing the safe HostApi trait
frontend/
  index.html          — Main HTML/CSS/JS, xterm.js terminal, layout, all inline styles
  utils.js            — Shared utilities (esc, attr, formatSize, formatDate)
  toast.js            — Global toast notification system
  ssh-panel.js        — SSH server tree, quick connect, tunnels, connection forms
  files-panel.js      — Dual-pane file explorer (local + remote)
  tunnel-manager.js   — SSH tunnel CRUD dialog
  plugin-widgets.js   — Widget JSON → HTML renderer, plugin dialog handlers
  plugin-manager.js   — Plugin discovery/enable/disable dialog
  icons/              — PNG icon set (file, folder, server, navigation, etc.)
```

### Plugin System (2 tiers — Lua + Java only)
- **Java** (Java/Kotlin/Scala): `.jar` files loaded by embedded JVM via JNI
- **Lua** (5.4): single `.lua` files loaded by mlua
- Both tiers call the safe `HostApi` Rust trait (no C ABI, no vtables, no unsafe)
- Declarative widget system: plugins return JSON widget trees → rendered as HTML
- Pub/sub event bus + RPC queries for inter-plugin communication
- Blocking dialog APIs (form, prompt, confirm) via oneshot channels
- Plugin config persistence: `~/.config/conch/plugins/{name}/{key}.json`
- Plugins are NOT auto-loaded — managed via Tools > Plugin Manager
- Enabled plugins persisted in `state.toml` and restored on restart

### Built-in Features (not plugins)
SSH sessions, SFTP file browsing, file transfers, SSH tunnels, server
management, `~/.ssh/config` import, and host key verification are all
built directly into `conch_tauri/src/remote/`. They were previously
separate native plugins but were consolidated for reliability.

### Key Patterns
- **Tauri v2 webview**: HTML/CSS/JS frontend, Rust backend via commands + events
- **xterm.js**: handles all terminal emulation; backend provides raw byte streams
- **CSS custom properties**: all colors derived from Alacritty theme files (`var(--bg)`, etc.)
- **Shared JS utilities**: `utils.js` provides `esc()`, `attr()`, `formatSize()`, `formatDate()` — no duplicating these across modules
- **Toast notifications**: all user-facing messages go through `toast.js` — never use `alert()` or `confirm()`
- **Overlay dialogs**: use the `ssh-overlay` / `ssh-form` CSS pattern with Escape to close
- **Auth prompts**: oneshot channels — emit event to frontend, block calling thread on response
- **SSH sessions**: reuse the same `pty-output`/`pty-exit` events as local PTY tabs
- **Plugin menu items**: stored in shared state, native menu rebuilt dynamically after enable
- **State persistence**: window size, panel widths, panel visibility, enabled plugins in `state.toml`
- **Hot-reload**: `watcher.rs` polls config.toml + themes/ every 2s, emits `config-changed` event

## Style Guide

### Rust
- Use `pub(crate)` for internal visibility, not `pub` (unless it's a library API)
- Prefer `if let` / `match` over `.unwrap()` — handle errors gracefully
- Use `log::error!`/`log::warn!` for recoverable errors, not panics
- `#[serde(default)]` on config structs for backward compatibility
- Keep `unsafe` blocks minimal and well-commented
- No unnecessary `clone()` — borrow where possible
- Factory methods for repeated struct construction (e.g., `PluginState::make_host_api()`)

### Frontend (JS)
- Each JS module is a self-contained IIFE exposing a global (e.g., `window.sshPanel`)
- Use `window.utils.esc()` / `window.utils.attr()` — never define local copies
- Use the global `toast.js` system for all notifications — never use `alert()` or `confirm()`
- CSS uses custom properties (`var(--bg)`, `var(--fg)`, `var(--text-secondary)`) — never hardcode hex colors
- Overlay dialogs use the `ssh-overlay` / `ssh-form` CSS pattern
- Escape handlers must use capture phase (`addEventListener(..., true)`) to fire before xterm.js
- Icons: use PNG assets from `frontend/icons/` via `<img>` tags or `iconHtml()` helper

### Config
- User config: `~/.config/conch/config.toml` (loaded by conch_core)
- Persistent state: `~/.config/conch/state.toml` (window size, plugins, layout)
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
- Currently 192 tests across the workspace — keep this growing
