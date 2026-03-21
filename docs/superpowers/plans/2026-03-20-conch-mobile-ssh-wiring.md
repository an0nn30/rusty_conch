# Conch Mobile SSH Wiring — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire SSH terminal sessions into the Conch Mobile iOS app — quick connect opens a working xterm.js terminal over SSH, with accessory bar and session management.

**Architecture:** Follow the desktop pattern from `conch_tauri/src/remote/mod.rs`: add `conch_remote` as a dependency, create `MobileRemoteCallbacks` (bridging auth prompts to Tauri events), store sessions in `MobileState`, expose thin `#[tauri::command]` wrappers, and build a `terminal.js` frontend module with xterm.js (canvas renderer) + accessory bar. The Connections tab's quick connect button triggers `ssh_quick_connect`, which opens the terminal view.

**Tech Stack:** Rust (conch_remote, Tauri v2), xterm.js 5.5 (canvas renderer), HTML/CSS/JS

**Spec:** `docs/superpowers/specs/2026-03-20-conch-mobile-app-design.md` and `docs/superpowers/specs/2026-03-20-mobile-ios-ssh-client-design.md` — Phase 2 (SSH + Terminal)

**Reference:** Desktop implementation at `crates/conch_tauri/src/remote/mod.rs` — follow the same patterns (TauriRemoteCallbacks, RemoteState, spawn_output_forwarder, channel_loop, auth prompt events).

---

## File Map

### New files (Rust)

| File | Responsibility |
|------|---------------|
| `crates/conch_mobile/src/state.rs` | `MobileState` — sessions, pending prompts, paths, config |
| `crates/conch_mobile/src/callbacks.rs` | `MobileRemoteCallbacks` — bridges RemoteCallbacks to Tauri events |
| `crates/conch_mobile/src/commands.rs` | Tauri commands: ssh_connect, ssh_write, ssh_resize, ssh_disconnect, auth responses, quick connect |

### New files (frontend)

| File | Responsibility |
|------|---------------|
| `crates/conch_mobile/frontend-mobile/terminal.js` | Terminal view: xterm.js setup, accessory bar, session header, output/exit event handling |
| `crates/conch_mobile/frontend-mobile/styles/terminal.css` | Terminal-specific styles: full-screen layout, accessory bar keys, session header |

### Modified files

| File | Change |
|------|--------|
| `crates/conch_mobile/Cargo.toml` | Add `conch_remote`, `conch_core`, `parking_lot`, `tokio`, `uuid`, `async-trait`, `dirs` |
| `crates/conch_mobile/src/lib.rs` | Add modules, register Tauri commands, manage state |
| `crates/conch_mobile/frontend-mobile/connections.js` | Wire quick connect to actually call `ssh_quick_connect` command |
| `crates/conch_mobile/frontend-mobile/tab-bar.js` | Add ability to show/hide terminal view (full-screen overlay above tabs) |
| `crates/conch_mobile/frontend-mobile/index.html` | Add xterm.js CDN script, terminal.css, terminal.js |

---

## Task 1: Add Rust dependencies

**Files:**
- Modify: `crates/conch_mobile/Cargo.toml`

- [ ] **Step 1: Add dependencies**

Add these to `[dependencies]`:
```toml
conch_core = { workspace = true }
conch_remote = { workspace = true }
parking_lot = { workspace = true }
tokio = { workspace = true }
uuid = { workspace = true }
async-trait = "0.1"
dirs = { workspace = true }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p conch_mobile`
Expected: compiles (warnings about unused deps are fine)

- [ ] **Step 3: Commit**

```bash
git add crates/conch_mobile/Cargo.toml Cargo.lock
git commit -m "Add conch_remote and SSH dependencies to conch_mobile"
```

---

## Task 2: Create state module

**Files:**
- Create: `crates/conch_mobile/src/state.rs`

This mirrors `conch_tauri`'s `RemoteState` and `SshSession`, adapted for mobile (no local PTY, no `~/.ssh/config` import on iOS, app-sandbox paths).

- [ ] **Step 1: Write state.rs**

```rust
//! App state for Conch Mobile — SSH sessions, config, auth prompts.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc;

use conch_remote::callbacks::RemotePaths;
use conch_remote::config::{SshConfig, ServerEntry};
use conch_remote::handler::ConchSshHandler;
use conch_remote::ssh::ChannelInput;

/// A live SSH session.
pub struct SshSession {
    pub input_tx: mpsc::UnboundedSender<ChannelInput>,
    pub ssh_handle: Arc<conch_remote::russh::client::Handle<ConchSshHandler>>,
    pub host: String,
    pub user: String,
    pub port: u16,
}

/// Pending auth prompts waiting for frontend responses.
pub struct PendingPrompts {
    pub host_key: HashMap<String, tokio::sync::oneshot::Sender<bool>>,
    pub password: HashMap<String, tokio::sync::oneshot::Sender<Option<String>>>,
}

impl PendingPrompts {
    pub fn new() -> Self {
        Self {
            host_key: HashMap::new(),
            password: HashMap::new(),
        }
    }
}

/// Shared state for all remote operations.
pub struct MobileState {
    /// SSH sessions keyed by session ID (e.g., "session-0", "session-1").
    pub sessions: HashMap<String, SshSession>,
    /// Next session ID counter.
    pub next_session_id: u32,
    /// Server configuration.
    pub config: SshConfig,
    /// Pending auth prompts.
    pub pending_prompts: Arc<Mutex<PendingPrompts>>,
    /// Platform-specific paths.
    pub paths: RemotePaths,
}

impl MobileState {
    pub fn new() -> Self {
        let paths = mobile_remote_paths();
        let config = conch_remote::config::load_config(&paths.config_dir);
        Self {
            sessions: HashMap::new(),
            next_session_id: 0,
            config,
            pending_prompts: Arc::new(Mutex::new(PendingPrompts::new())),
            paths,
        }
    }

    /// Allocate a new session ID.
    pub fn alloc_session_id(&mut self) -> String {
        let id = format!("session-{}", self.next_session_id);
        self.next_session_id += 1;
        id
    }
}

/// Build `RemotePaths` for iOS.
///
/// On iOS, there is no `~/.ssh/` directory. Keys are imported via
/// the iOS Files app and stored in the app's documents directory.
/// Known hosts are stored in the app's config directory.
fn mobile_remote_paths() -> RemotePaths {
    // On iOS, use the app's documents/config directory.
    // For now, use dirs::config_dir() which maps to the app sandbox.
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("conch")
        .join("remote");
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("conch");

    RemotePaths {
        known_hosts_file: data_dir.join("known_hosts"),
        config_dir,
        // No default key paths on iOS — keys come from explicit import.
        default_key_paths: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_state_new_has_no_sessions() {
        let state = MobileState::new();
        assert!(state.sessions.is_empty());
        assert_eq!(state.next_session_id, 0);
    }

    #[test]
    fn alloc_session_id_increments() {
        let mut state = MobileState::new();
        assert_eq!(state.alloc_session_id(), "session-0");
        assert_eq!(state.alloc_session_id(), "session-1");
        assert_eq!(state.alloc_session_id(), "session-2");
    }

    #[test]
    fn pending_prompts_new_is_empty() {
        let p = PendingPrompts::new();
        assert!(p.host_key.is_empty());
        assert!(p.password.is_empty());
    }

    #[test]
    fn mobile_remote_paths_has_empty_key_paths() {
        let paths = mobile_remote_paths();
        assert!(paths.default_key_paths.is_empty());
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

Add `mod state;` to `lib.rs` (after the `ios_native` module).

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p conch_mobile`
Expected: 5 tests pass (1 existing + 4 new)

- [ ] **Step 4: Commit**

```bash
git add crates/conch_mobile/src/state.rs crates/conch_mobile/src/lib.rs
git commit -m "Add MobileState with session tracking and iOS paths"
```

---

## Task 3: Create callbacks module

**Files:**
- Create: `crates/conch_mobile/src/callbacks.rs`

Mirrors `TauriRemoteCallbacks` from the desktop app.

- [ ] **Step 1: Write callbacks.rs**

```rust
//! MobileRemoteCallbacks — bridges RemoteCallbacks to Tauri events.
//!
//! When the SSH handler needs user interaction (host key confirmation,
//! password entry), this emits a Tauri event and waits on a oneshot
//! channel that the frontend resolves via auth_respond commands.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::Emitter;

use conch_remote::callbacks::RemoteCallbacks;
use crate::state::PendingPrompts;

#[derive(Clone, Serialize)]
pub struct HostKeyPromptEvent {
    pub prompt_id: String,
    pub message: String,
    pub detail: String,
}

#[derive(Clone, Serialize)]
pub struct PasswordPromptEvent {
    pub prompt_id: String,
    pub message: String,
}

/// Bridges `RemoteCallbacks` to Tauri events + oneshot channels.
pub struct MobileRemoteCallbacks {
    pub app: tauri::AppHandle,
    pub pending_prompts: Arc<Mutex<PendingPrompts>>,
}

#[async_trait::async_trait]
impl RemoteCallbacks for MobileRemoteCallbacks {
    async fn verify_host_key(&self, message: &str, fingerprint: &str) -> bool {
        let prompt_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_prompts
            .lock()
            .host_key
            .insert(prompt_id.clone(), tx);
        let _ = self.app.emit(
            "ssh-host-key-prompt",
            HostKeyPromptEvent {
                prompt_id,
                message: message.to_string(),
                detail: fingerprint.to_string(),
            },
        );
        rx.await.unwrap_or(false)
    }

    async fn prompt_password(&self, message: &str) -> Option<String> {
        let prompt_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_prompts
            .lock()
            .password
            .insert(prompt_id.clone(), tx);
        let _ = self.app.emit(
            "ssh-password-prompt",
            PasswordPromptEvent {
                prompt_id,
                message: message.to_string(),
            },
        );
        rx.await.unwrap_or(None)
    }

    fn on_transfer_progress(&self, _transfer_id: &str, _bytes: u64, _total: Option<u64>) {
        // Transfer progress handled separately — not needed for SSH wiring.
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

Add `mod callbacks;` to `lib.rs`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p conch_mobile`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/conch_mobile/src/callbacks.rs crates/conch_mobile/src/lib.rs
git commit -m "Add MobileRemoteCallbacks bridging auth prompts to Tauri events"
```

---

## Task 4: Create commands module

**Files:**
- Create: `crates/conch_mobile/src/commands.rs`

The Tauri commands for SSH. Follows the desktop pattern exactly — `ssh_quick_connect`, `ssh_write`, `ssh_resize`, `ssh_disconnect`, `auth_respond_host_key`, `auth_respond_password`, `get_sessions`.

- [ ] **Step 1: Write commands.rs**

This is a large file. Key points:
- `ssh_quick_connect` parses `user@host:port`, creates a `ServerEntry`, connects via `conch_remote::ssh::connect_and_open_shell`, spawns channel loop and output forwarder, returns the session ID.
- `ssh_write` / `ssh_resize` / `ssh_disconnect` find the session by ID and send `ChannelInput`.
- `auth_respond_host_key` / `auth_respond_password` resolve oneshot channels in `PendingPrompts`.
- `get_sessions` returns a list of active session IDs with host/user info.
- Output forwarder emits `pty-output` events (same event name as desktop — xterm.js doesn't care).
- Channel loop cleanup emits `pty-exit` on natural exit.

The session key is a simple incrementing ID (`session-0`, `session-1`) since mobile has no window labels.

Reference the desktop implementation at `crates/conch_tauri/src/remote/mod.rs:230-400` for the exact pattern. Adapt:
- No `window_label` — use session ID directly
- No `tab_id` — use session ID
- `emit` globally (not `emit_to`) since there's only one webview
- `pty-output` payload includes `session_id` instead of `window_label`+`tab_id`

```rust
//! Tauri commands for SSH session management.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::Emitter;
use tokio::sync::mpsc;

use conch_remote::callbacks::RemoteCallbacks;
use conch_remote::config::ServerEntry;
use conch_remote::ssh::ChannelInput;

use crate::callbacks::MobileRemoteCallbacks;
use crate::state::{MobileState, SshSession};

// ---------------------------------------------------------------------------
// Event types emitted to the frontend
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
struct PtyOutputEvent {
    session_id: String,
    data: String,
}

#[derive(Clone, Serialize)]
struct PtyExitEvent {
    session_id: String,
}

#[derive(Clone, Serialize)]
struct ActiveSessionInfo {
    session_id: String,
    host: String,
    user: String,
    port: u16,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Quick-connect by parsing a connection string.
#[tauri::command]
pub async fn ssh_quick_connect(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Mutex<MobileState>>>,
    spec: String,
    cols: u16,
    rows: u16,
    password: Option<String>,
) -> Result<String, String> {
    let (user, host, port) = parse_quick_connect(&spec);

    let auth_method = if password.is_some() {
        "password".to_string()
    } else {
        "key".to_string()
    };

    let entry = ServerEntry {
        id: uuid::Uuid::new_v4().to_string(),
        label: format!("{user}@{host}:{port}"),
        host,
        port,
        user,
        auth_method,
        key_path: None,
        proxy_command: None,
        proxy_jump: None,
    };

    let (session_id, pending_prompts, paths) = {
        let mut s = state.lock();
        let sid = s.alloc_session_id();
        (sid, Arc::clone(&s.pending_prompts), s.paths.clone())
    };

    let callbacks: Arc<dyn RemoteCallbacks> = Arc::new(MobileRemoteCallbacks {
        app: app.clone(),
        pending_prompts: Arc::clone(&pending_prompts),
    });

    let (ssh_handle, channel) =
        conch_remote::ssh::connect_and_open_shell(&entry, password, callbacks, &paths).await?;

    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Request initial resize.
    let _ = input_tx.send(ChannelInput::Resize { cols, rows });

    // Store the session.
    {
        let mut s = state.lock();
        s.sessions.insert(
            session_id.clone(),
            SshSession {
                input_tx,
                ssh_handle: Arc::new(ssh_handle),
                host: entry.host.clone(),
                user: entry.user.clone(),
                port: entry.port,
            },
        );
    }

    // Spawn channel loop.
    let state_for_loop = Arc::clone(&*state);
    let sid_for_loop = session_id.clone();
    let app_for_loop = app.clone();
    tokio::spawn(async move {
        let exited_naturally =
            conch_remote::ssh::channel_loop(channel, input_rx, output_tx).await;

        state_for_loop.lock().sessions.remove(&sid_for_loop);

        if exited_naturally {
            let _ = app_for_loop.emit("pty-exit", PtyExitEvent {
                session_id: sid_for_loop,
            });
        }
    });

    // Spawn output forwarder.
    spawn_output_forwarder(&app, &session_id, output_rx);

    Ok(session_id)
}

/// Write data to an SSH session.
#[tauri::command]
pub async fn ssh_write(
    state: tauri::State<'_, Arc<Mutex<MobileState>>>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let s = state.lock();
    let session = s.sessions.get(&session_id)
        .ok_or_else(|| format!("Session '{session_id}' not found"))?;
    session.input_tx.send(ChannelInput::Write(data))
        .map_err(|_| "Session channel closed".to_string())
}

/// Resize an SSH session's PTY.
#[tauri::command]
pub async fn ssh_resize(
    state: tauri::State<'_, Arc<Mutex<MobileState>>>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let s = state.lock();
    let session = s.sessions.get(&session_id)
        .ok_or_else(|| format!("Session '{session_id}' not found"))?;
    session.input_tx.send(ChannelInput::Resize { cols, rows })
        .map_err(|_| "Session channel closed".to_string())
}

/// Disconnect an SSH session.
#[tauri::command]
pub async fn ssh_disconnect(
    state: tauri::State<'_, Arc<Mutex<MobileState>>>,
    session_id: String,
) -> Result<(), String> {
    let mut s = state.lock();
    if let Some(session) = s.sessions.remove(&session_id) {
        let _ = session.input_tx.send(ChannelInput::Shutdown);
    }
    Ok(())
}

/// Respond to a host key verification prompt.
#[tauri::command]
pub async fn auth_respond_host_key(
    state: tauri::State<'_, Arc<Mutex<MobileState>>>,
    prompt_id: String,
    accepted: bool,
) {
    let s = state.lock();
    if let Some(tx) = s.pending_prompts.lock().host_key.remove(&prompt_id) {
        let _ = tx.send(accepted);
    }
}

/// Respond to a password prompt.
#[tauri::command]
pub async fn auth_respond_password(
    state: tauri::State<'_, Arc<Mutex<MobileState>>>,
    prompt_id: String,
    password: Option<String>,
) {
    let s = state.lock();
    if let Some(tx) = s.pending_prompts.lock().password.remove(&prompt_id) {
        let _ = tx.send(password);
    }
}

/// Get list of active sessions.
#[tauri::command]
pub async fn get_sessions(
    state: tauri::State<'_, Arc<Mutex<MobileState>>>,
) -> Vec<ActiveSessionInfo> {
    let s = state.lock();
    s.sessions.iter().map(|(id, session)| ActiveSessionInfo {
        session_id: id.clone(),
        host: session.host.clone(),
        user: session.user.clone(),
        port: session.port,
    }).collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Spawn a task that drains output_rx and emits pty-output events.
/// Handles partial UTF-8 sequences that may be split across packets.
fn spawn_output_forwarder(
    app: &tauri::AppHandle,
    session_id: &str,
    mut output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
) {
    let app = app.clone();
    let sid = session_id.to_owned();
    tokio::spawn(async move {
        let mut pending = Vec::new();
        while let Some(data) = output_rx.recv().await {
            pending.extend_from_slice(&data);

            // Find the longest valid UTF-8 prefix, keep the rest for next time.
            let valid_len = match std::str::from_utf8(&pending) {
                Ok(_) => pending.len(),
                Err(e) => e.valid_up_to(),
            };

            if valid_len == 0 {
                continue;
            }

            let text = String::from_utf8_lossy(&pending[..valid_len]).to_string();
            pending.drain(..valid_len);

            let _ = app.emit("pty-output", PtyOutputEvent {
                session_id: sid.clone(),
                data: text,
            });
        }
    });
}

/// Parse a quick connect string into (user, host, port).
/// Supports: `host`, `user@host`, `user@host:port`, `host:port`
fn parse_quick_connect(spec: &str) -> (String, String, u16) {
    let (user, rest) = if let Some((u, r)) = spec.split_once('@') {
        (u.to_string(), r)
    } else {
        ("root".to_string(), spec)
    };

    let (host, port) = if let Some((h, p)) = rest.rsplit_once(':') {
        (h.to_string(), p.parse().unwrap_or(22))
    } else {
        (rest.to_string(), 22)
    };

    (user, host, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quick_connect_host_only() {
        let (user, host, port) = parse_quick_connect("example.com");
        assert_eq!(user, "root");
        assert_eq!(host, "example.com");
        assert_eq!(port, 22);
    }

    #[test]
    fn parse_quick_connect_user_at_host() {
        let (user, host, port) = parse_quick_connect("deploy@10.0.0.1");
        assert_eq!(user, "deploy");
        assert_eq!(host, "10.0.0.1");
        assert_eq!(port, 22);
    }

    #[test]
    fn parse_quick_connect_user_at_host_port() {
        let (user, host, port) = parse_quick_connect("admin@server.io:2222");
        assert_eq!(user, "admin");
        assert_eq!(host, "server.io");
        assert_eq!(port, 2222);
    }

    #[test]
    fn parse_quick_connect_host_port() {
        let (user, host, port) = parse_quick_connect("myhost:8022");
        assert_eq!(user, "root");
        assert_eq!(host, "myhost");
        assert_eq!(port, 8022);
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

Add `mod commands;` to `lib.rs`.

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p conch_mobile`
Expected: 9 tests pass (5 existing + 4 new)

- [ ] **Step 4: Commit**

```bash
git add crates/conch_mobile/src/commands.rs crates/conch_mobile/src/lib.rs
git commit -m "Add SSH Tauri commands: connect, write, resize, disconnect, auth"
```

---

## Task 5: Wire commands into Tauri builder

**Files:**
- Modify: `crates/conch_mobile/src/lib.rs`

Register the commands and manage state in the Tauri builder.

- [ ] **Step 1: Update lib.rs**

```rust
//! Conch Mobile — iOS SSH client.

#[cfg(target_os = "ios")]
mod ios_native;

mod callbacks;
mod commands;
mod state;

use std::sync::Arc;
use parking_lot::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mobile_state = Arc::new(Mutex::new(state::MobileState::new()));

    tauri::Builder::default()
        .manage(mobile_state)
        .invoke_handler(tauri::generate_handler![
            commands::ssh_quick_connect,
            commands::ssh_write,
            commands::ssh_resize,
            commands::ssh_disconnect,
            commands::auth_respond_host_key,
            commands::auth_respond_password,
            commands::get_sessions,
        ])
        .setup(|app| {
            #[cfg(target_os = "ios")]
            {
                use tauri::Manager;
                if let Some(webview) = app.get_webview_window("main") {
                    ios_native::setup_native_tab_bar(&webview);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Conch Mobile");
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_module_loads() {
        assert!(true);
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p conch_mobile`
Expected: compiles

- [ ] **Step 3: Run tests**

Run: `cargo test -p conch_mobile`
Expected: 9 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/conch_mobile/src/lib.rs
git commit -m "Register SSH commands and state in Tauri builder"
```

---

## Task 6: Create terminal CSS

**Files:**
- Create: `crates/conch_mobile/frontend-mobile/styles/terminal.css`

- [ ] **Step 1: Write terminal.css**

Styles for the full-screen terminal overlay, session header bar, accessory bar, and auth prompt dialogs.

```css
/* Terminal view — full-screen overlay */

#terminal-view {
  display: none;
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 1000;
  background: var(--bg);
  flex-direction: column;
}

#terminal-view.active {
  display: flex;
}

/* Session header */
.terminal-header {
  display: flex;
  align-items: center;
  padding: 0 12px;
  padding-top: env(safe-area-inset-top, 0px);
  background: var(--bg-dark);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
  min-height: calc(44px + env(safe-area-inset-top, 0px));
}

.terminal-back-btn {
  background: none;
  border: none;
  color: var(--purple);
  font-size: 16px;
  padding: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
}

.terminal-title {
  flex: 1;
  text-align: center;
  font-size: 15px;
  font-weight: 600;
  color: var(--fg);
}

.terminal-disconnect-btn {
  background: none;
  border: none;
  color: var(--red);
  font-size: 13px;
  font-weight: 600;
  padding: 8px;
  cursor: pointer;
}

/* xterm.js container */
#terminal-container {
  flex: 1;
  overflow: hidden;
  background: var(--bg);
}

#terminal-container .xterm {
  height: 100%;
}

/* Accessory bar */
.accessory-bar {
  display: flex;
  gap: 4px;
  padding: 6px 8px;
  background: var(--bg-dark);
  border-top: 1px solid var(--border);
  overflow-x: auto;
  flex-shrink: 0;
  -webkit-overflow-scrolling: touch;
}

.accessory-bar::-webkit-scrollbar {
  display: none;
}

.accessory-key {
  padding: 7px 12px;
  background: var(--card-bg-alt, #44475a);
  border-radius: 6px;
  color: var(--fg);
  font-size: 13px;
  font-family: 'SF Mono', Menlo, monospace;
  font-weight: 500;
  white-space: nowrap;
  border: none;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  flex-shrink: 0;
}

.accessory-key:active {
  background: var(--purple);
  color: var(--bg);
}

.accessory-key.sticky {
  background: var(--purple);
  color: var(--bg);
}

/* Auth prompt overlay */
.auth-overlay {
  position: fixed;
  top: 0; left: 0; right: 0; bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.auth-dialog {
  background: var(--bg-dark);
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 24px;
  width: 85%;
  max-width: 340px;
}

.auth-dialog h3 {
  color: var(--fg);
  font-size: 17px;
  margin-bottom: 8px;
}

.auth-dialog p {
  color: var(--fg-dim);
  font-size: 13px;
  margin-bottom: 16px;
  word-break: break-all;
}

.auth-dialog input {
  width: 100%;
  padding: 10px 12px;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 8px;
  color: var(--fg);
  font-size: 15px;
  margin-bottom: 16px;
  outline: none;
}

.auth-dialog-buttons {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.auth-dialog-buttons button {
  padding: 8px 20px;
  border-radius: 8px;
  font-size: 15px;
  font-weight: 600;
  border: none;
  cursor: pointer;
}

.auth-btn-cancel {
  background: var(--bg);
  color: var(--fg-dim);
}

.auth-btn-confirm {
  background: var(--purple);
  color: var(--bg);
}
```

- [ ] **Step 2: Add to index.html**

Add `<link rel="stylesheet" href="styles/terminal.css">` after the main.css link.

- [ ] **Step 3: Commit**

```bash
git add crates/conch_mobile/frontend-mobile/styles/terminal.css crates/conch_mobile/frontend-mobile/index.html
git commit -m "Add terminal view CSS: overlay, header, accessory bar, auth dialogs"
```

---

## Task 7: Create terminal.js

**Files:**
- Create: `crates/conch_mobile/frontend-mobile/terminal.js`
- Modify: `crates/conch_mobile/frontend-mobile/index.html`

This is the core frontend module — sets up xterm.js, handles SSH output, accessory bar, auth prompts, and session lifecycle.

- [ ] **Step 1: Add xterm.js to index.html**

Add before the closing `</head>`:
```html
<!-- xterm.js (canvas renderer for iOS WKWebView) -->
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/css/xterm.css">
<script src="https://cdn.jsdelivr.net/npm/@xterm/xterm@5.5.0/lib/xterm.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.10.0/lib/addon-fit.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@xterm/addon-canvas@0.7.0/lib/addon-canvas.js"></script>
```

Add the terminal view container in the `#app` div, before `#tab-content`:
```html
<!-- Terminal overlay (hidden by default, shown on SSH connect) -->
<div id="terminal-view">
  <div class="terminal-header">
    <button class="terminal-back-btn" id="terminal-back">
      <svg width="20" height="20" viewBox="0 0 20 20" fill="none"><path d="M12 4L6 10l6 6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>
      Back
    </button>
    <span class="terminal-title" id="terminal-title">Connecting...</span>
    <button class="terminal-disconnect-btn" id="terminal-disconnect">Disconnect</button>
  </div>
  <div id="terminal-container"></div>
  <div class="accessory-bar" id="accessory-bar"></div>
</div>
```

Add the terminal.js script after profile.js:
```html
<script src="terminal.js"></script>
```

- [ ] **Step 2: Write terminal.js**

```js
// Terminal view — xterm.js + SSH session management + accessory bar.

(function (exports) {
  'use strict';

  let terminal = null;
  let fitAddon = null;
  let currentSessionId = null;
  let ctrlSticky = false;
  let altSticky = false;

  const ACCESSORY_KEYS = [
    { label: 'Esc',  send: '\x1b' },
    { label: 'Tab',  send: '\t' },
    { label: 'Ctrl', ctrl: true },
    { label: 'Alt',  alt: true },
    { label: '↑',    send: '\x1b[A' },
    { label: '↓',    send: '\x1b[B' },
    { label: '→',    send: '\x1b[C' },
    { label: '←',    send: '\x1b[D' },
    { label: '|',    send: '|' },
    { label: '/',    send: '/' },
    { label: '~',    send: '~' },
    { label: '-',    send: '-' },
  ];

  /** Open a terminal session. */
  async function connect(spec, password) {
    const view = document.getElementById('terminal-view');
    const titleEl = document.getElementById('terminal-title');
    view.classList.add('active');
    titleEl.textContent = 'Connecting to ' + spec + '...';

    // Create xterm.js instance
    if (!terminal) {
      createTerminal();
    } else {
      terminal.clear();
    }

    try {
      const sessionId = await window.__TAURI__.core.invoke('ssh_quick_connect', {
        spec,
        cols: terminal.cols,
        rows: terminal.rows,
        password: password || null,
      });

      currentSessionId = sessionId;
      titleEl.textContent = spec;

      // Focus terminal
      terminal.focus();
    } catch (err) {
      titleEl.textContent = 'Connection failed';
      window.toast.error('SSH Error', err);
      setTimeout(() => close(), 2000);
    }
  }

  /** Create the xterm.js terminal instance. */
  function createTerminal() {
    const container = document.getElementById('terminal-container');
    container.innerHTML = '';

    terminal = new window.Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'SF Mono', Menlo, 'Fira Code', monospace",
      theme: {
        background: '#282a36',
        foreground: '#f8f8f2',
        cursor: '#f8f8f2',
        selectionBackground: '#44475a',
        black: '#21222c',
        red: '#ff5555',
        green: '#50fa7b',
        yellow: '#f1fa8c',
        blue: '#bd93f9',
        magenta: '#ff79c6',
        cyan: '#8be9fd',
        white: '#f8f8f2',
        brightBlack: '#6272a4',
        brightRed: '#ff6e6e',
        brightGreen: '#69ff94',
        brightYellow: '#ffffa5',
        brightBlue: '#d6acff',
        brightMagenta: '#ff92df',
        brightCyan: '#a4ffff',
        brightWhite: '#ffffff',
      },
      scrollback: 1000,
      allowProposedApi: true,
    });

    fitAddon = new window.FitAddon.FitAddon();
    terminal.loadAddon(fitAddon);

    // Use canvas renderer for iOS WKWebView performance (WebGL is unreliable)
    if (window.CanvasAddon) {
      terminal.loadAddon(new window.CanvasAddon.CanvasAddon());
    }

    terminal.open(container);

    // Fit after a frame so the container has dimensions
    requestAnimationFrame(() => {
      fitAddon.fit();
    });

    // Send user input to SSH
    terminal.onData((data) => {
      if (!currentSessionId) return;

      // Handle Ctrl sticky mode
      if (ctrlSticky) {
        ctrlSticky = false;
        updateModifierButtons();
        // Convert to Ctrl character (ASCII 1-26)
        const ch = data.toUpperCase().charCodeAt(0);
        if (ch >= 65 && ch <= 90) {
          data = String.fromCharCode(ch - 64);
        }
      }

      // Handle Alt sticky mode (sends ESC prefix)
      if (altSticky) {
        altSticky = false;
        updateModifierButtons();
        data = '\x1b' + data;
      }

      const bytes = new TextEncoder().encode(data);
      window.__TAURI__.core.invoke('ssh_write', {
        sessionId: currentSessionId,
        data: Array.from(bytes),
      }).catch(() => {});
    });

    // Handle resize
    terminal.onResize(({ cols, rows }) => {
      if (!currentSessionId) return;
      window.__TAURI__.core.invoke('ssh_resize', {
        sessionId: currentSessionId,
        cols, rows,
      }).catch(() => {});
    });

    // Refit on window resize / orientation change / iOS keyboard show/hide
    window.addEventListener('resize', () => {
      if (fitAddon && terminal) fitAddon.fit();
    });
    // visualViewport is more reliable for iOS keyboard events
    window.visualViewport?.addEventListener('resize', () => {
      if (fitAddon && terminal) fitAddon.fit();
    });

    // Build accessory bar
    buildAccessoryBar();
  }

  /** Build the accessory key bar. */
  function buildAccessoryBar() {
    const bar = document.getElementById('accessory-bar');
    bar.innerHTML = '';

    ACCESSORY_KEYS.forEach(key => {
      const btn = document.createElement('button');
      btn.className = 'accessory-key';
      btn.textContent = key.label;
      if (key.ctrl) btn.id = 'ctrl-key';
      if (key.alt) btn.id = 'alt-key';

      btn.addEventListener('click', () => {
        if (key.ctrl) {
          ctrlSticky = !ctrlSticky;
          updateCtrlButton();
          terminal.focus();
          return;
        }
        if (key.alt) {
          altSticky = !altSticky;
          updateModifierButtons();
          terminal.focus();
          return;
        }
        if (key.send && currentSessionId) {
          const bytes = new TextEncoder().encode(key.send);
          window.__TAURI__.core.invoke('ssh_write', {
            sessionId: currentSessionId,
            data: Array.from(bytes),
          }).catch(() => {});
        }
        terminal.focus();
      });

      bar.appendChild(btn);
    });
  }

  function updateModifierButtons() {
    const ctrlBtn = document.getElementById('ctrl-key');
    const altBtn = document.getElementById('alt-key');
    if (ctrlBtn) ctrlBtn.classList.toggle('sticky', ctrlSticky);
    if (altBtn) altBtn.classList.toggle('sticky', altSticky);
  }

  /** Close the terminal view and disconnect. */
  function close() {
    const view = document.getElementById('terminal-view');
    view.classList.remove('active');

    if (currentSessionId) {
      window.__TAURI__.core.invoke('ssh_disconnect', {
        sessionId: currentSessionId,
      }).catch(() => {});
      currentSessionId = null;
    }
  }

  /** Initialize event listeners. */
  function init() {
    // Back button
    document.getElementById('terminal-back')
      .addEventListener('click', close);

    // Disconnect button
    document.getElementById('terminal-disconnect')
      .addEventListener('click', close);

    // Listen for SSH output
    if (window.__TAURI__) {
      window.__TAURI__.event.listen('pty-output', (event) => {
        const { session_id, data } = event.payload;
        if (session_id === currentSessionId && terminal) {
          terminal.write(data);
        }
      });

      // Listen for SSH session exit
      window.__TAURI__.event.listen('pty-exit', (event) => {
        const { session_id } = event.payload;
        if (session_id === currentSessionId) {
          window.toast.info('Disconnected', 'Session closed by server.');
          setTimeout(() => close(), 1000);
        }
      });

      // Listen for host key prompts
      window.__TAURI__.event.listen('ssh-host-key-prompt', (event) => {
        showHostKeyPrompt(event.payload);
      });

      // Listen for password prompts
      window.__TAURI__.event.listen('ssh-password-prompt', (event) => {
        showPasswordPrompt(event.payload);
      });
    }
  }

  // ---------------------------------------------------------------------------
  // Auth prompt dialogs
  // ---------------------------------------------------------------------------

  function showHostKeyPrompt({ prompt_id, message, detail }) {
    const overlay = document.createElement('div');
    overlay.className = 'auth-overlay';
    overlay.innerHTML = `
      <div class="auth-dialog">
        <h3>Host Key Verification</h3>
        <p>${window.utils.esc(message)}</p>
        <p style="font-family:monospace;font-size:11px;">${window.utils.esc(detail)}</p>
        <div class="auth-dialog-buttons">
          <button class="auth-btn-cancel" id="hk-reject">Reject</button>
          <button class="auth-btn-confirm" id="hk-accept">Accept</button>
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    overlay.querySelector('#hk-accept').addEventListener('click', () => {
      window.__TAURI__.core.invoke('auth_respond_host_key', {
        promptId: prompt_id, accepted: true,
      });
      overlay.remove();
    });
    overlay.querySelector('#hk-reject').addEventListener('click', () => {
      window.__TAURI__.core.invoke('auth_respond_host_key', {
        promptId: prompt_id, accepted: false,
      });
      overlay.remove();
    });
  }

  function showPasswordPrompt({ prompt_id, message }) {
    const overlay = document.createElement('div');
    overlay.className = 'auth-overlay';
    overlay.innerHTML = `
      <div class="auth-dialog">
        <h3>Password Required</h3>
        <p>${window.utils.esc(message)}</p>
        <input type="password" id="pw-input" placeholder="Password"
               autocomplete="off" autocorrect="off" autocapitalize="none">
        <div class="auth-dialog-buttons">
          <button class="auth-btn-cancel" id="pw-cancel">Cancel</button>
          <button class="auth-btn-confirm" id="pw-submit">Connect</button>
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    const input = overlay.querySelector('#pw-input');
    input.focus();

    overlay.querySelector('#pw-submit').addEventListener('click', () => {
      window.__TAURI__.core.invoke('auth_respond_password', {
        promptId: prompt_id, password: input.value || null,
      });
      overlay.remove();
    });
    overlay.querySelector('#pw-cancel').addEventListener('click', () => {
      window.__TAURI__.core.invoke('auth_respond_password', {
        promptId: prompt_id, password: null,
      });
      overlay.remove();
    });
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        overlay.querySelector('#pw-submit').click();
      }
    });
  }

  exports.terminalView = { init, connect, close };
})(window);
```

- [ ] **Step 3: Initialize terminal in DOMContentLoaded**

In `index.html`'s init script, add after `window.tabBar.init()`:
```js
window.terminalView.init();
```

- [ ] **Step 4: Commit**

```bash
git add crates/conch_mobile/frontend-mobile/terminal.js crates/conch_mobile/frontend-mobile/index.html
git commit -m "Add terminal.js with xterm.js, accessory bar, and auth prompts"
```

---

## Task 8: Wire quick connect to terminal

**Files:**
- Modify: `crates/conch_mobile/frontend-mobile/connections.js`

Connect the quick connect button to actually open a terminal session.

- [ ] **Step 1: Update handleQuickConnect**

Replace the existing `handleQuickConnect` function:

```js
function handleQuickConnect(target) {
  if (!target) {
    window.toast.warn('Quick Connect', 'Enter a host or user@host to connect.');
    return;
  }
  // Open terminal and connect
  window.terminalView.connect(target);
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p conch_mobile`
Expected: compiles

- [ ] **Step 3: Run tests**

Run: `cargo test -p conch_mobile`
Expected: 9 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/conch_mobile/frontend-mobile/connections.js
git commit -m "Wire quick connect to open SSH terminal session"
```

---

## Task 9: Final verification

- [ ] **Step 1: Run workspace tests**

Run: `cargo test --workspace`
Expected: 235+ tests pass

- [ ] **Step 2: Build for iOS**

Run: `cd crates/conch_mobile && cargo tauri ios build --debug`
Expected: builds successfully

- [ ] **Step 3: Test on device/simulator**

Launch on device or simulator and verify:
1. Quick connect bar accepts `user@host` input
2. Tapping Connect opens the terminal overlay
3. Host key prompt appears for new hosts
4. Password prompt appears if needed
5. Terminal receives and displays SSH output
6. Accessory bar keys work (Esc, Tab, Ctrl+C, arrows)
7. Back button closes terminal and returns to tabs
8. Disconnect button terminates session

- [ ] **Step 4: Push**

```bash
git push -u origin feat/conch-mobile-ssh
```
