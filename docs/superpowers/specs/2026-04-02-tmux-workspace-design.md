# Tmux Workspace Mode — Technical Design

## Context

Conch users who rely on tmux currently use a shell-wrapper hack (`bash -c "tmux new-session"`) that creates junk sessions and inverts the ownership model. This design makes tmux a first-class backend where Conch owns rendering/chrome and tmux owns sessions/windows/panes.

See the product spec in this same directory for goals, non-goals, UX rules, and success criteria.

## Key Design Decisions

These were resolved during brainstorming:

1. **Control mode (`tmux -CC`) from day one** — no CLI subprocess fallback, no hybrid PTY layer. Control mode provides structured notifications and per-pane output routing.
2. **Full parser, selective handlers** — parse all control mode notification types upfront. Only wire handlers for what each delivery milestone needs. Avoids rework when later phases need notifications we skipped.
3. **Per-window control mode connections** — each Conch window that attaches to a tmux session spawns its own `tmux -CC` process. Window owns the connection lifecycle. No global multiplexing layer.
4. **No hybrid rendering** — `%output` notifications feed xterm.js directly. The existing PTY backend is not involved when a window is in tmux mode. No throwaway bridge code.
5. **Live-update from notifications** — the Tmux Sessions tool window and tab/pane state update in real time from control mode notifications. No polling.
6. **"Sessions" → "Hosts" rename** — done as a separate prerequisite PR before tmux work begins, to avoid mixing rename noise into the tmux diff.
7. **`terminal.tmux.binary` in phase 1** — one config field with a sensible default (search `$PATH`), prevents support issues for non-standard installs.

## Prerequisite: Rename "Sessions" to "Hosts"

Separate `chore/` branch. Mechanical rename across:

- `ssh-panel.js`: panel title `Sessions` → `Hosts`
- `tool-window-runtime.js`: registration title `Sessions` → `Hosts`
- `menu.rs`: `"Toggle & Focus Sessions"` → `"Toggle & Focus Hosts"`
- `titlebar.js`: menu entry label
- `command-palette-runtime.js`: `"Focus Sessions"` → `"Focus Hosts"`
- `ssh-panel.js`: section header `"SSH Sessions"` → `"SSH Hosts"` (if present)
- Config/shortcut labels referencing "sessions" in the SSH context

This must merge before the tmux feature branch begins.

## Architecture

### New Crate: `conch_tmux`

Pure Rust library for the tmux control mode protocol. No Tauri, no async runtime, no UI concepts.

```
crates/conch_tmux/
  Cargo.toml
  src/
    lib.rs              — Public API re-exports
    protocol.rs         — Notification enum, all control mode message types
    parser.rs           — ControlModeParser: bytes → Notifications
    command.rs          — CommandBuilder: typed methods → tmux command strings
    connection.rs       — ControlModeConnection: process lifecycle, read/write
    session.rs          — TmuxSession model, SessionList with apply_notification
```

Dependencies: `std`, `log`. Optional `serde` feature (enabled by `conch_tauri`) for serializing `TmuxSession`.

No dependency on any other workspace crate.

### Notification Types

```rust
pub enum Notification {
    // Session lifecycle
    SessionChanged { session_id: u64, name: String },
    SessionRenamed { session_id: u64, name: String },
    SessionWindowChanged { session_id: u64, window_id: u64 },
    SessionsChanged,

    // Window lifecycle
    WindowAdd { window_id: u64 },
    WindowClose { window_id: u64 },
    WindowRenamed { window_id: u64, name: String },
    WindowPaneChanged { window_id: u64, pane_id: u64 },

    // Pane lifecycle
    PaneModeChanged { pane_id: u64, mode: u8 },

    // Output
    Output { pane_id: u64, data: Vec<u8> },

    // Layout
    LayoutChange { window_id: u64, layout: String },

    // Command responses
    Begin { command_number: u64, flags: u32 },
    End { command_number: u64, flags: u32 },
    Error { command_number: u64, message: String },

    // Connection
    Exit { reason: Option<String> },

    // Forward compat
    Unknown { name: String, args: String },
}
```

### ControlModeParser

Synchronous, no I/O. Fed bytes via `feed()`, yields `Notification` values. Handles:

- Partial line accumulation across `feed()` calls
- `%begin`/`%end` block tracking for command responses
- UTF-8 boundary safety
- Octal escape decoding for `%output` payloads

```rust
pub struct ControlModeParser {
    buffer: Vec<u8>,
    pending_responses: HashMap<u64, Vec<String>>,
}

impl ControlModeParser {
    pub fn new() -> Self;
    pub fn feed(&mut self, data: &[u8]) -> Vec<Notification>;
}
```

### CommandBuilder

Generates tmux command strings. Each method returns a `String` ready to write to the control mode connection stdin.

```rust
pub struct CommandBuilder;

impl CommandBuilder {
    pub fn list_sessions() -> String;
    pub fn new_session(name: Option<&str>) -> String;
    pub fn kill_session(target: &str) -> String;
    pub fn rename_session(target: &str, new_name: &str) -> String;
    pub fn attach_session(target: &str) -> String;
    pub fn detach_client() -> String;
    pub fn new_window(target_session: &str) -> String;
    pub fn kill_window(target: &str) -> String;
    pub fn split_window(target: &str, horizontal: bool) -> String;
    pub fn select_pane(target: &str) -> String;
    pub fn rename_window(target: &str, new_name: &str) -> String;
    pub fn resize_pane(target: &str, cols: u16, rows: u16) -> String;
}
```

### ControlModeConnection

Manages the `tmux -CC` child process. The caller controls the tmux subcommand via `args` — typical invocations:

- Attach or create: `["-CC", "new-session", "-A", "-s", "myname"]`
- Attach existing: `["-CC", "attach-session", "-t", "myname"]`

```rust
pub struct ControlModeConnection {
    child: std::process::Child,
    writer: std::io::BufWriter<ChildStdin>,
    parser: ControlModeParser,
}

impl ControlModeConnection {
    pub fn new(binary: &str, args: &[&str]) -> Result<Self>;
    pub fn send_command(&mut self, cmd: &str) -> Result<u64>;
    pub fn reader(&mut self) -> &mut ChildStdout;
    pub fn parse_bytes(&mut self, data: &[u8]) -> Vec<Notification>;
    pub fn kill(self) -> Result<()>;
}
```

### Session Model

```rust
pub struct TmuxSession {
    pub id: u64,
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
    pub created: Option<u64>,
}

pub struct SessionList {
    sessions: Vec<TmuxSession>,
}

impl SessionList {
    pub fn update_from_list_output(&mut self, raw: &str);
    pub fn apply_notification(&mut self, notif: &Notification);
    pub fn sessions(&self) -> &[TmuxSession];
}
```

## Integration Layer: `conch_tauri/src/tmux/`

Thin bridge between `conch_tmux` and the Tauri app.

```
crates/conch_tauri/src/tmux/
  mod.rs              — TmuxState, Tauri commands, connection registry
  bridge.rs           — Reader thread: drives parser, emits Tauri events
  events.rs           — Serializable event payload structs
```

### TmuxState

```rust
pub(crate) struct TmuxState {
    connections: Mutex<HashMap<String, TmuxWindowConnection>>,
    sessions: RwLock<SessionList>,
    binary: String,
}

struct TmuxWindowConnection {
    connection: ControlModeConnection,
    reader_handle: JoinHandle<()>,
    attached_session: Option<String>,
}
```

### Reader Thread (bridge.rs)

Each connection spawns a reader thread:

```rust
fn tmux_reader_loop(
    app: AppHandle,
    window_label: String,
    mut connection: ControlModeConnection,
    sessions: Arc<RwLock<SessionList>>,
) {
    let mut buf = [0u8; 8192];
    loop {
        match connection.reader().read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                for notif in connection.parse_bytes(&buf[..n]) {
                    sessions.write().apply_notification(&notif);
                    emit_notification(&app, &window_label, &notif);
                }
            }
            Err(_) => break,
        }
    }
    app.emit_to(&window_label, "tmux-disconnected", ());
}
```

### Tauri Commands

```rust
// Session management
tmux_connect(window, session_name) -> Result<()>
tmux_disconnect(window) -> Result<()>
tmux_list_sessions() -> Result<Vec<TmuxSessionInfo>>
tmux_create_session(name: Option<String>) -> Result<String>
tmux_kill_session(name) -> Result<()>
tmux_rename_session(old, new) -> Result<()>

// Window/pane control
tmux_new_window(window) -> Result<()>
tmux_close_window(window, window_id: u64) -> Result<()>
tmux_rename_window(window, window_id: u64, name: String) -> Result<()>
tmux_split_pane(window, horizontal: bool) -> Result<()>
tmux_close_pane(window, pane_id: u64) -> Result<()>
tmux_select_pane(window, pane_id: u64) -> Result<()>
tmux_write_to_pane(window, pane_id: u64, data: String) -> Result<()>
tmux_resize_pane(window, pane_id: u64, cols: u16, rows: u16) -> Result<()>
```

### Frontend Events

| Event | Payload | Purpose |
|-------|---------|---------|
| `tmux-sessions-changed` | `Vec<TmuxSessionInfo>` | Session list updated |
| `tmux-connected` | `{ session, windows: [...] }` | Initial state on attach |
| `tmux-disconnected` | `{ reason }` | Connection lost |
| `tmux-output` | `{ pane_id, data }` | Terminal bytes → xterm.js |
| `tmux-window-add` | `{ window_id, name }` | Create tab |
| `tmux-window-close` | `{ window_id }` | Remove tab |
| `tmux-window-renamed` | `{ window_id, name }` | Update tab label |
| `tmux-pane-add` | `{ window_id, pane_id }` | Create pane split |
| `tmux-pane-close` | `{ pane_id }` | Remove pane |
| `tmux-layout-change` | `{ window_id, layout }` | Adjust pane geometry |

## Configuration

### New Fields in `conch_core`

```toml
[terminal]
backend = "local"          # "local" or "tmux"

[terminal.tmux]
binary = ""                # empty = search $PATH
startup_behavior = "attach_last_session"
new_tab_behavior = "new_tmux_window"
new_window_behavior = "attach_same_session"
```

### Rust Types

```rust
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalBackend {
    #[default]
    Local,
    Tmux,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TmuxConfig {
    pub binary: String,
    pub startup_behavior: TmuxStartupBehavior,
    pub new_tab_behavior: TmuxNewTabBehavior,
    pub new_window_behavior: TmuxNewWindowBehavior,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TmuxStartupBehavior {
    #[default]
    AttachLastSession,
    ShowSessionPicker,
    CreateNewSession,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TmuxNewTabBehavior {
    #[default]
    NewTmuxWindow,
    SessionPicker,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TmuxNewWindowBehavior {
    #[default]
    AttachSameSession,
    ShowSessionPicker,
}
```

All fields use `#[serde(default)]` — existing configs without `[terminal.tmux]` parse cleanly as `backend = Local`.

### State Persistence

Add to `PersistentState`:

```rust
pub last_tmux_session: Option<String>,
```

Tracks Conch's own last-attached session for `attach_last_session` startup behavior.

## Frontend

### Backend Router

New file: `frontend/app/backend-router.js`

Routes tab/pane actions by backend mode. The tab and pane managers call `backendRouter` instead of directly invoking PTY commands.

```javascript
window.backendRouter = {
    mode: 'local',

    newTab()                    { /* spawn_shell or tmux_new_window */ },
    closeTab(tabId)             { /* close_pty or tmux_close_window */ },
    writeToPane(paneId, data)   { /* write_to_pty or tmux_write_to_pane */ },
    resizePane(paneId, c, r)    { /* resize_pty or tmux_resize_pane */ },
    splitVertical()             { /* local split or tmux_split_pane */ },
    splitHorizontal()           { /* local split or tmux_split_pane */ },
    renameTab(tabId, name)      { /* local rename or tmux_rename_window */ },
    closePane(paneId)           { /* close_pty or tmux_close_pane */ },
};
```

### Tmux ID Map

New file: `frontend/app/tmux-id-map.js`

Maps tmux IDs to frontend DOM elements:

```javascript
window.tmuxIdMap = {
    windowToTab: new Map(),   // tmux window_id → frontend tabId
    tabToWindow: new Map(),   // frontend tabId → tmux window_id
    tmuxToPane: new Map(),    // tmux pane_id → frontend paneId
    paneToTmux: new Map(),    // frontend paneId → tmux pane_id
};
```

### Event-Driven Tab/Pane Lifecycle

In tmux mode, the frontend never creates tabs/panes speculatively. The flow is inverted:

1. User clicks "New Tab" → `backendRouter.newTab()` → `tmux_new_window` command
2. tmux creates the window → emits `%window-add` notification
3. Reader thread emits `tmux-window-add` event
4. Frontend creates the tab in response

Same pattern for splits, closes, renames. The UI waits for tmux confirmation.

### Output Routing

```javascript
listen('tmux-output', (event) => {
    const { pane_id, data } = event.payload;
    const frontendPaneId = tmuxIdMap.tmuxToPane.get(pane_id);
    if (frontendPaneId) {
        const pane = getPanes().get(frontendPaneId);
        if (pane?.term) pane.term.write(data);
    }
});
```

### Tmux Sessions Tool Window

New file: `frontend/app/panels/tmux-panel.js`

Self-contained IIFE exposing `window.tmuxPanel`. Registered conditionally when backend is `tmux`:

```javascript
toolWindowManager.register('tmux-sessions', {
    title: 'Tmux Sessions',
    type: 'built-in',
    defaultZone: 'right-bottom',
    renderFn: (container) => { ... },
});
```

**Layout:**

```
┌─ Tmux Sessions ──────────────┐
│ [+ New] [Attach] [⟳ Refresh] │
├───────────────────────────────┤
│ ● my-project          3 wins │  ← attached
│ ○ scratch              1 win │
│ ○ server-logs          2 wins│
└───────────────────────────────┘
```

**Interactions:**

- Single click: select row
- Double click: attach in current window
- Right-click context menu: Attach, Open In New Window, Rename, Kill
- Toolbar: New Session, Attach, Open In New Window, Rename, Kill, Refresh
- Empty state: message + "Create Session" button

Live-updated via `tmux-sessions-changed` events. No polling.

### Chrome Indicators

Tmux session badge in the tab strip area:

```
[tmux: my-project]  │ Tab 1 │ Tab 2 │ Tab 3 │+
```

- `var(--text-secondary)` styling, not distracting
- Clicking the badge opens the Tmux Sessions tool window
- Hidden in local mode

### Command Palette

Conditionally registered when backend is `tmux`:

- `Tmux: Show Sessions`
- `Tmux: Attach Session`
- `Tmux: Create Session`
- `Tmux: Rename Session`
- `Tmux: Kill Session`
- `Tmux: Open Session In New Window`
- `Tmux: New Window`
- `Tmux: Split Horizontal`
- `Tmux: Split Vertical`

### Menu

Add to View menu (conditional on tmux backend):

- `Toggle & Focus Tmux Sessions` (no default shortcut)

Existing menu items (New Tab, Split, etc.) don't change labels — they route through `backendRouter`.

## Startup Flow

### Local Mode

No change.

### Tmux Mode

```
App launches → config says backend = "tmux"
  → resolve tmux binary
  → validate: spawn `tmux -V`, check >= 1.8
  → if not found: toast error, fall back to local
  → emit init-backend("tmux") to frontend
  → apply startup_behavior
```

| `startup_behavior` | Flow |
|--------------------|------|
| `attach_last_session` | Read `last_tmux_session` from state.toml → if session alive, connect → else fall through to `show_session_picker` |
| `show_session_picker` | Open Tmux Sessions panel, wait for user |
| `create_new_session` | Create fresh session, connect immediately |

### Session Switching

1. Send `detach-client` on current connection
2. Tear down connection (reader thread exits)
3. Frontend clears all tabs/panes
4. Spawn new `tmux -CC attach-session -t <new>`
5. `tmux-connected` → frontend rebuilds tabs/panes

### New Conch Window (Tmux Mode)

| `new_window_behavior` | Flow |
|------------------------|------|
| `attach_same_session` | New window connects to same session as originator |
| `show_session_picker` | New window opens with session picker, no auto-connect |

### Window Close

Always detaches, never kills the session. Killing the `tmux -CC` child detaches the client. Session and processes continue.

### Error States

| Condition | Behavior |
|-----------|----------|
| tmux binary not found | Toast, fall back to local |
| tmux server not running | `new-session` starts one implicitly |
| Attached session killed externally | `tmux-disconnected` → toast → session picker |
| Control mode connection drops | `tmux-disconnected` → toast → offer reconnect |
| tmux version < 1.8 | Warn on startup, fall back to local |

## Modified Existing Files

| File | Change |
|------|--------|
| `tab-manager.js` | Call `backendRouter` instead of direct PTY invocations |
| `pane-manager.js` | Call `backendRouter` instead of direct PTY invocations |
| `tool-window-runtime.js` | Conditionally register `tmux-sessions` tool window |
| `command-palette-runtime.js` | Conditionally add tmux command palette entries |
| `event-wiring-runtime.js` | Register tmux event listeners when mode is tmux |
| `ui/titlebar.js` | Show tmux session badge |
| `lib.rs` | Register `TmuxState`, emit `init-backend` event |
| `menu.rs` | Add conditional "Toggle & Focus Tmux Sessions" item |
| `conch_core/config/terminal.rs` | Add `TerminalBackend`, `TmuxConfig`, enums |
| `conch_core/config/persistent.rs` | Add `last_tmux_session` field |
| `Cargo.toml` (workspace) | Add `conch_tmux` member |
| `conch_tauri/Cargo.toml` | Add `conch_tmux` dependency |
| `config.example.toml` | Add tmux configuration examples |

## Dependency Graph

```
conch_core    ← conch_tauri
conch_tmux    ← conch_tauri
conch_plugin_sdk ← conch_plugin ← conch_tauri
```

`conch_tmux` depends on no other workspace crate.

## Testing Strategy

### `conch_tmux` (heaviest investment)

**Parser tests:** feed raw control mode byte sequences, assert `Notification` variants. Cover every notification type, partial reads, interleaved blocks, UTF-8 boundaries, octal escape decoding, unknown notification types.

**Command builder tests:** assert exact command strings for every method, including special characters in names.

**Session model tests:** feed notification sequences, assert model state transitions. Empty state, add/remove/rename.

### `conch_core`

Config struct serde round-trips, defaults, backward compat with configs that lack `[terminal.tmux]`, `resolved_binary()` behavior.

### `conch_tauri`

Tauri command logic where testable without live app context. The integration layer is intentionally thin to minimize untestable surface.

### Not unit tested

- Actual tmux process spawning (requires tmux installed)
- Frontend JS (no test framework)
- Tauri command handlers requiring live app context
- xterm.js rendering

Covered by manual testing.
