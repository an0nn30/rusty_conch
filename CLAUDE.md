# Conch — Claude Instructions

## Critical Engineering Standards

### 1. Unit Tests Are Required
Every new function, module, or behavior change MUST have unit tests if at all possible. The project already has `#[cfg(test)]` modules in most files — follow that pattern. If adding a new `.rs` file, add a `#[cfg(test)] mod tests` at the bottom. Pure logic, parsers, config handling, widget building, keybinding resolution — all testable without a GUI context. Only skip tests for code that truly requires a live egui context or OS-level resources.

### 2. Modularity — No Monoliths
Code MUST be broken into small, focused modules. `app.rs` is already too large and must not grow further. When adding new functionality:
- Extract into its own file/module (e.g., `shortcuts.rs`, `input.rs`, `icons.rs`)
- Group related files into subdirectories with `mod.rs` (e.g., `host/`, `terminal/`, `menu_bar/`, `platform/`)
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
  conch_app/        — GUI application (egui/eframe), the main binary
  conch_core/       — Config loading, color schemes, shared types
  conch_plugin/     — Plugin host: Lua runner, JVM runtime, native plugin manager, plugin bus
  conch_plugin_sdk/ — SDK for native (Rust) plugins: HostApi, widgets, FFI types
  conch_pty/        — PTY abstraction and connector
plugins/
  conch-ssh/        — Native plugin: SSH sessions, SFTP, server tree
  conch-files/      — Native plugin: dual-pane file explorer with local + SFTP
  test-*/           — Test plugins for development
java-sdk/           — Java Plugin SDK: HostApi, ConchPlugin, Widgets, PluginInfo
examples/plugins/   — Example Lua plugins (tmux-sessions, system-info, etc.)
```

### conch_app Module Layout
```
app.rs              — ConchApp coordinator, eframe::App impl, plugin infrastructure
main.rs             — Entry point, CLI parsing, font loading, window setup
window_state.rs     — Per-window state, render_window(), handle_keyboard(), SharedAppState
input.rs            — KeyBinding parsing, key_to_bytes conversion, ResolvedShortcuts
state.rs            — Session struct, SessionBackend, AppState
sessions.rs         — Session creation (local + plain shell)
notifications.rs    — Toast notification rendering and state
icons.rs            — IconCache, icon loading
ui_theme.rs         — UiTheme struct, font sizes, colors, light/dark mode
context_menu.rs     — Right-click context menus
tab_bar.rs          — Tab bar rendering
ipc.rs              — Unix socket IPC
watcher.rs          — File system watcher (config + theme hot reload)
mouse.rs            — Mouse event handling
host/               — Plugin hosting bridge
  bridge.rs         — HostApi FFI implementation, SFTP registry, PTY write queue, global state
  panel_renderer.rs — Widget rendering (tables, toolbars, trees, buttons, etc.)
  plugin_panels.rs  — Panel layout (left/right/bottom panels with tabs), status bar
  plugin_lifecycle.rs — Plugin discovery, start/stop/reload
  plugin_manager_ui.rs — Plugin manager UI
  session_bridge.rs — Session<->plugin bridge
  dialogs.rs        — Plugin-triggered dialogs (form, prompt, confirm, alert, error)
terminal/           — Terminal rendering
  widget.rs         — Terminal grid rendering, selection, cursor
  color.rs          — ANSI color mapping
  size_info.rs      — Terminal size calculations
menu_bar/           — Menu bar
  mod.rs            — MenuAction enum, MenuBarState, mode resolution
  egui_menu.rs      — In-window egui menu bar rendering
  native_macos.rs   — Native macOS NSMenu integration
platform/           — Platform-specific code
  capabilities.rs   — PlatformCapabilities detection
  macos.rs          — macOS-specific (fullsize content view)
  linux.rs          — Linux-specific
  windows.rs        — Windows-specific (dark title bar DWM API)
```

### Plugin Architecture (3 tiers)
- **Native** (Rust/C/Go): shared libraries (`.dylib`/`.so`/`.dll`), communicate via `HostApi` C ABI vtable
- **Java** (Java/Kotlin/Scala): `.jar` files loaded by embedded JVM, communicate via JNI bridge to HostApi
- **Lua**: single `.lua` files, no compilation, communicate via Lua API wrappers around HostApi
- All tiers share: declarative widget system (JSON), pub/sub event bus, config persistence, dialog APIs
- Cross-plugin FFI: `SftpVtable` pattern for direct function-pointer access between native plugins
- Plugin config persistence via `HostApi::get_config`/`set_config` (JSON files per plugin)

### Key Patterns
- egui immediate-mode: all UI rebuilt every frame, state lives on ConchApp
- Plugin bus: pub/sub event system for plugin<->app and plugin<->plugin communication
- `query_plugin`: IPC between plugins (JSON messages over mpsc channels)
- Panel registry: plugins register panels at locations (Left, Right, Bottom)
- `#[repr(C)]` vtables with manual ref counting for cross-plugin FFI
- Terminal owns keyboard input by default — only divert when a widget explicitly has focus
- Tab key: intercepted in `raw_input_hook` before egui sees it, sent directly to PTY

## Style Guide

### Rust
- Use `pub(crate)` for internal visibility, not `pub` (unless it's a library API)
- Prefer `if let` / `match` over `.unwrap()` — handle errors gracefully
- Use `log::error!`/`log::warn!` for recoverable errors, not panics
- `#[serde(default)]` on config structs for backward compatibility
- Keep `unsafe` blocks minimal and well-commented
- No unnecessary `clone()` — borrow where possible

### Config
- User config: `config.toml` (loaded by conch_core)
- Persistent state: `state.toml` (window size, loaded plugins, layout)
- Plugin config: `{plugin_name}/{key}.json` files via HostApi
- Keyboard shortcuts: configurable in `[conch.keyboard]` section
- Default shortcuts use `cmd+` prefix (maps to Cmd on macOS, Ctrl on Linux/Windows)

### Testing Standards
- `#[cfg(test)] mod tests` at the bottom of each file
- Test pure logic: parsing, config defaults, widget building, keybinding matching
- Use `assert_eq!` with descriptive messages
- Test edge cases: empty input, missing fields, boundary values
- Plugin SDK: test widget serialization/deserialization
- Config: test defaults, serde round-trips, backward compat with `serde(default)`
- Keybindings: test parsing, matching, modifier combos
