# Tmux Workspace Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tmux as a first-class terminal backend using control mode (`tmux -CC`), with a new `conch_tmux` crate for the protocol layer, config extensions, a Tmux Sessions tool window, and frontend routing so that tab/pane actions go to tmux instead of the local PTY when in tmux mode.

**Architecture:** New `conch_tmux` workspace crate owns the control mode parser, command builder, and session model — pure Rust, no Tauri dependency. A thin integration layer in `conch_tauri/src/tmux/` bridges it to Tauri events and commands. The frontend gets a `backendRouter` that switches action routing between local PTY and tmux based on the window's backend mode. Per-window `tmux -CC` connections mean each Conch window owns its control mode process.

**Tech Stack:** Rust (std, serde, log), Tauri v2 commands/events, JavaScript (IIFE modules, xterm.js)

**Spec:** `docs/superpowers/specs/2026-04-02-tmux-workspace-design.md`

---

## Prerequisite

The "Sessions" → "Hosts" rename is a separate `chore/rename-sessions-to-hosts` branch and must merge before this work starts. It is NOT part of this plan.

---

## File Structure

### New Crate: `crates/conch_tmux/`

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Crate manifest with optional `serde` feature |
| `src/lib.rs` | Public re-exports |
| `src/protocol.rs` | `Notification` enum — all control mode message types |
| `src/parser.rs` | `ControlModeParser` — bytes in, `Notification`s out |
| `src/command.rs` | `CommandBuilder` — typed methods → tmux command strings |
| `src/session.rs` | `TmuxSession`, `SessionList` — in-memory session model |
| `src/connection.rs` | `ControlModeConnection` — child process lifecycle |

### Modified: `crates/conch_core/`

| File | Change |
|------|--------|
| `src/config/terminal.rs` | Add `TerminalBackend` enum, `TmuxConfig` struct, update `TerminalConfig` |
| `src/config/persistent.rs` | Add `last_tmux_session: Option<String>` to `PersistentState` |

### New: `crates/conch_tauri/src/tmux/`

| File | Responsibility |
|------|---------------|
| `mod.rs` | `TmuxState`, Tauri commands, connection registry |
| `bridge.rs` | Reader thread — drives parser, emits Tauri events |
| `events.rs` | Serializable event payload structs |

### Modified: `crates/conch_tauri/`

| File | Change |
|------|--------|
| `Cargo.toml` | Add `conch_tmux` dependency |
| `src/lib.rs` | Register `TmuxState`, add tmux commands to invoke handler, emit `init-backend` |
| `src/menu.rs` | Add `MENU_FOCUS_TMUX_SESSIONS_ID` constant and conditional menu item |

### New Frontend Files: `crates/conch_tauri/frontend/app/`

| File | Responsibility |
|------|---------------|
| `backend-router.js` | Routes tab/pane actions by backend mode (`local` vs `tmux`) |
| `tmux-id-map.js` | Maps tmux IDs ↔ frontend pane/tab IDs |
| `panels/tmux-panel.js` | Tmux Sessions tool window |

### Modified Frontend Files

| File | Change |
|------|--------|
| `tool-window-runtime.js` | Conditionally register `tmux-sessions` tool window |
| `event-wiring-runtime.js` | Register tmux event listeners when mode is `tmux` |
| `command-palette-runtime.js` | Conditionally add tmux command palette entries |
| `ui/titlebar.js` | Show tmux session badge, add menu entry |
| `tab-manager.js` | Route through `backendRouter` instead of direct PTY calls |
| `pane-manager.js` | Route through `backendRouter` instead of direct PTY calls |

---

## Task 1: Create `conch_tmux` Crate Scaffolding

**Files:**
- Create: `crates/conch_tmux/Cargo.toml`
- Create: `crates/conch_tmux/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create crate directory**

```bash
mkdir -p crates/conch_tmux/src
```

- [ ] **Step 2: Write `Cargo.toml`**

Create `crates/conch_tmux/Cargo.toml`:

```toml
[package]
name = "conch_tmux"
version.workspace = true
edition.workspace = true
description = "Tmux control mode protocol library for Conch"

[dependencies]
log = { workspace = true }

[dependencies.serde]
workspace = true
optional = true

[features]
default = []
serde = ["dep:serde"]

[dev-dependencies]
```

- [ ] **Step 3: Write `src/lib.rs`**

Create `crates/conch_tmux/src/lib.rs`:

```rust
//! Tmux control mode protocol library.
//!
//! Provides a parser for `tmux -CC` output, a command builder for sending
//! tmux commands, and an in-memory session model.

pub mod command;
pub mod connection;
pub mod parser;
pub mod protocol;
pub mod session;

pub use command::CommandBuilder;
pub use connection::ControlModeConnection;
pub use parser::ControlModeParser;
pub use protocol::Notification;
pub use session::{SessionList, TmuxSession};
```

- [ ] **Step 4: Create stub modules so the crate compiles**

Create `crates/conch_tmux/src/protocol.rs`:

```rust
//! Control mode notification types.

/// A notification emitted by tmux in control mode.
#[derive(Debug, Clone, PartialEq)]
pub enum Notification {}
```

Create `crates/conch_tmux/src/parser.rs`:

```rust
//! Control mode output parser.

use crate::protocol::Notification;

/// Parses raw bytes from a `tmux -CC` process into typed notifications.
pub struct ControlModeParser {
    buffer: Vec<u8>,
}

impl ControlModeParser {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// Feed raw bytes and return any complete notifications parsed.
    pub fn feed(&mut self, _data: &[u8]) -> Vec<Notification> {
        Vec::new()
    }
}

impl Default for ControlModeParser {
    fn default() -> Self {
        Self::new()
    }
}
```

Create `crates/conch_tmux/src/command.rs`:

```rust
//! Tmux command builder for control mode.

/// Builds tmux command strings to send over a control mode connection.
pub struct CommandBuilder;
```

Create `crates/conch_tmux/src/session.rs`:

```rust
//! In-memory tmux session model.

/// A tmux session.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TmuxSession {
    pub id: u64,
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
    pub created: Option<u64>,
}

/// Tracks the set of known tmux sessions.
#[derive(Debug, Clone, Default)]
pub struct SessionList {
    sessions: Vec<TmuxSession>,
}

impl SessionList {
    pub fn sessions(&self) -> &[TmuxSession] {
        &self.sessions
    }
}
```

Create `crates/conch_tmux/src/connection.rs`:

```rust
//! Control mode connection manager.

use std::io::{self, BufWriter, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::parser::ControlModeParser;
use crate::protocol::Notification;

/// Manages a `tmux -CC` child process.
pub struct ControlModeConnection {
    child: Child,
    writer: BufWriter<ChildStdin>,
    parser: ControlModeParser,
    next_command_number: u64,
}
```

- [ ] **Step 5: Add workspace member**

In root `Cargo.toml`, add `"crates/conch_tmux"` to the `[workspace] members` array and add the workspace dependency:

```toml
# In [workspace] members array, add:
"crates/conch_tmux",

# In [workspace.dependencies], add:
conch_tmux = { path = "crates/conch_tmux" }
```

- [ ] **Step 6: Verify crate compiles**

Run: `cargo check -p conch_tmux`
Expected: compiles with no errors (warnings about unused fields are OK)

- [ ] **Step 7: Commit**

```bash
git add crates/conch_tmux/ Cargo.toml
git commit -m "Add conch_tmux crate scaffolding"
```

---

## Task 2: Protocol Types

**Files:**
- Modify: `crates/conch_tmux/src/protocol.rs`

- [ ] **Step 1: Write tests for notification types**

Add to `crates/conch_tmux/src/protocol.rs`:

```rust
//! Control mode notification types.

/// A notification emitted by tmux in control mode.
///
/// Each variant corresponds to a `%`-prefixed line in the control mode stream.
/// See `tmux(1)` CONTROL MODE section for the full protocol reference.
#[derive(Debug, Clone, PartialEq)]
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

    // Output: raw terminal bytes for a pane
    Output { pane_id: u64, data: Vec<u8> },

    // Layout
    LayoutChange { window_id: u64, layout: String },

    // Command response framing
    Begin { command_number: u64, flags: u32 },
    End { command_number: u64, flags: u32 },
    Error { command_number: u64, message: String },

    // Connection lifecycle
    Exit { reason: Option<String> },

    // Forward compatibility for unknown notification types
    Unknown { name: String, args: String },
}

/// A complete command response collected between %begin and %end.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResponse {
    pub command_number: u64,
    pub lines: Vec<String>,
    pub success: bool,
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_variants_are_distinct() {
        let a = Notification::SessionsChanged;
        let b = Notification::Exit { reason: None };
        assert_ne!(a, b);
    }

    #[test]
    fn notification_clone() {
        let n = Notification::Output {
            pane_id: 1,
            data: vec![65, 66, 67],
        };
        assert_eq!(n, n.clone());
    }

    #[test]
    fn command_response_with_lines() {
        let r = CommandResponse {
            command_number: 42,
            lines: vec!["line1".into(), "line2".into()],
            success: true,
            error_message: None,
        };
        assert_eq!(r.lines.len(), 2);
        assert!(r.success);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p conch_tmux`
Expected: 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tmux/src/protocol.rs
git commit -m "Add tmux control mode notification types"
```

---

## Task 3: Control Mode Parser

**Files:**
- Modify: `crates/conch_tmux/src/parser.rs`

This is the largest and most critical task. The parser must handle all notification types, partial line buffering, `%begin`/`%end` command response blocks, and `%output` octal escape decoding.

- [ ] **Step 1: Write failing tests for basic notifications**

Replace `crates/conch_tmux/src/parser.rs` with:

```rust
//! Control mode output parser.
//!
//! Tmux control mode emits lines prefixed with `%`. This parser accumulates
//! bytes, splits on newlines, and converts each `%`-line into a typed
//! [`Notification`].

use std::collections::HashMap;

use crate::protocol::{CommandResponse, Notification};

/// Parses raw bytes from a `tmux -CC` process into typed notifications.
pub struct ControlModeParser {
    /// Accumulates bytes until a complete line (ending in `\n`) is available.
    buffer: Vec<u8>,
    /// Tracks in-progress command responses between %begin and %end.
    pending_responses: HashMap<u64, Vec<String>>,
}

impl ControlModeParser {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            pending_responses: HashMap::new(),
        }
    }

    /// Feed raw bytes from the control mode stream. Returns any complete
    /// notifications that were parsed from complete lines in the input.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Notification> {
        self.buffer.extend_from_slice(data);
        let mut notifications = Vec::new();

        loop {
            let newline_pos = match self.buffer.iter().position(|&b| b == b'\n') {
                Some(pos) => pos,
                None => break,
            };

            let line_bytes: Vec<u8> = self.buffer.drain(..=newline_pos).collect();
            let line = String::from_utf8_lossy(&line_bytes).trim_end().to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(notif) = self.parse_line(&line) {
                notifications.push(notif);
            }
        }

        notifications
    }

    fn parse_line(&mut self, line: &str) -> Option<Notification> {
        // Lines between %begin and %end are command response body lines
        if !line.starts_with('%') {
            // Could be a command response body line — append to all pending responses
            for lines in self.pending_responses.values_mut() {
                lines.push(line.to_string());
            }
            return None;
        }

        let (name, rest) = match line[1..].split_once(' ') {
            Some((n, r)) => (n, r),
            None => (&line[1..], ""),
        };

        match name {
            "sessions-changed" => Some(Notification::SessionsChanged),

            "session-changed" => {
                let (id, name) = parse_id_and_name(rest, '$')?;
                Some(Notification::SessionChanged {
                    session_id: id,
                    name,
                })
            }

            "session-renamed" => {
                let (id, name) = parse_id_and_name(rest, '$')?;
                Some(Notification::SessionRenamed {
                    session_id: id,
                    name,
                })
            }

            "session-window-changed" => {
                let (session_id, window_part) = parse_id_and_name(rest, '$')?;
                let window_id = parse_prefixed_id(&window_part, '@')?;
                Some(Notification::SessionWindowChanged {
                    session_id,
                    window_id,
                })
            }

            "window-add" => {
                let id = parse_prefixed_id(rest.trim(), '@')?;
                Some(Notification::WindowAdd { window_id: id })
            }

            "window-close" => {
                let id = parse_prefixed_id(rest.trim(), '@')?;
                Some(Notification::WindowClose { window_id: id })
            }

            "window-renamed" => {
                let (id, name) = parse_id_and_name(rest, '@')?;
                Some(Notification::WindowRenamed {
                    window_id: id,
                    name,
                })
            }

            "window-pane-changed" => {
                let (window_id, pane_part) = parse_id_and_name(rest, '@')?;
                let pane_id = parse_prefixed_id(&pane_part, '%')?;
                Some(Notification::WindowPaneChanged { window_id, pane_id })
            }

            "pane-mode-changed" => {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                let pane_id = parse_prefixed_id(parts.first()?, '%')?;
                let mode = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                Some(Notification::PaneModeChanged { pane_id, mode })
            }

            "output" => {
                let (pane_id, encoded) = parse_id_and_name(rest, '%')?;
                let data = decode_octal_escapes(&encoded);
                Some(Notification::Output { pane_id, data })
            }

            "layout-change" => {
                let (id, layout) = parse_id_and_name(rest, '@')?;
                Some(Notification::LayoutChange {
                    window_id: id,
                    layout,
                })
            }

            "begin" => {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                let ts_and_cmd = parts.first()?;
                let command_number: u64 = ts_and_cmd.parse().ok()?;
                let flags: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                self.pending_responses.entry(command_number).or_default();
                Some(Notification::Begin {
                    command_number,
                    flags,
                })
            }

            "end" => {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                let command_number: u64 = parts.first()?.parse().ok()?;
                let flags: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                self.pending_responses.remove(&command_number);
                Some(Notification::End {
                    command_number,
                    flags,
                })
            }

            "error" => {
                let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                let command_number: u64 = parts.first()?.parse().ok()?;
                let message = parts.get(1).unwrap_or(&"").to_string();
                self.pending_responses.remove(&command_number);
                Some(Notification::Error {
                    command_number,
                    message,
                })
            }

            "exit" => {
                let reason = if rest.is_empty() {
                    None
                } else {
                    Some(rest.to_string())
                };
                Some(Notification::Exit { reason })
            }

            _ => Some(Notification::Unknown {
                name: name.to_string(),
                args: rest.to_string(),
            }),
        }
    }

    /// Collect a completed command response. Call after receiving an `End`
    /// or `Error` notification to retrieve the accumulated output lines.
    pub fn take_response(&mut self, command_number: u64) -> Option<Vec<String>> {
        self.pending_responses.remove(&command_number)
    }
}

impl Default for ControlModeParser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a tmux ID like `$1`, `@2`, or `%3` — returns the numeric part.
fn parse_prefixed_id(s: &str, prefix: char) -> Option<u64> {
    s.strip_prefix(prefix)?.parse().ok()
}

/// Parse `"$1 session-name"` or `"@2 window-name"` into `(id, name)`.
fn parse_id_and_name(s: &str, prefix: char) -> Option<(u64, String)> {
    let (id_part, name_part) = s.split_once(' ')?;
    let id = parse_prefixed_id(id_part, prefix)?;
    Some((id, name_part.to_string()))
}

/// Decode tmux octal escapes in `%output` data.
///
/// Tmux escapes non-printable bytes as `\OOO` (three-digit octal).
fn decode_octal_escapes(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            let d1 = bytes[i + 1];
            let d2 = bytes[i + 2];
            let d3 = bytes[i + 3];
            if d1.is_ascii_digit() && d2.is_ascii_digit() && d3.is_ascii_digit() {
                let val = (d1 - b'0') as u16 * 64 + (d2 - b'0') as u16 * 8 + (d3 - b'0') as u16;
                if val <= 255 {
                    out.push(val as u8);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Basic notification parsing ---

    #[test]
    fn parse_sessions_changed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%sessions-changed\n");
        assert_eq!(notifs, vec![Notification::SessionsChanged]);
    }

    #[test]
    fn parse_session_changed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%session-changed $1 my-session\n");
        assert_eq!(
            notifs,
            vec![Notification::SessionChanged {
                session_id: 1,
                name: "my-session".into(),
            }]
        );
    }

    #[test]
    fn parse_session_renamed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%session-renamed $3 new-name\n");
        assert_eq!(
            notifs,
            vec![Notification::SessionRenamed {
                session_id: 3,
                name: "new-name".into(),
            }]
        );
    }

    #[test]
    fn parse_session_window_changed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%session-window-changed $1 @2\n");
        assert_eq!(
            notifs,
            vec![Notification::SessionWindowChanged {
                session_id: 1,
                window_id: 2,
            }]
        );
    }

    #[test]
    fn parse_window_add() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%window-add @5\n");
        assert_eq!(notifs, vec![Notification::WindowAdd { window_id: 5 }]);
    }

    #[test]
    fn parse_window_close() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%window-close @3\n");
        assert_eq!(notifs, vec![Notification::WindowClose { window_id: 3 }]);
    }

    #[test]
    fn parse_window_renamed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%window-renamed @1 my-window\n");
        assert_eq!(
            notifs,
            vec![Notification::WindowRenamed {
                window_id: 1,
                name: "my-window".into(),
            }]
        );
    }

    #[test]
    fn parse_window_pane_changed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%window-pane-changed @1 %3\n");
        assert_eq!(
            notifs,
            vec![Notification::WindowPaneChanged {
                window_id: 1,
                pane_id: 3,
            }]
        );
    }

    #[test]
    fn parse_pane_mode_changed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%pane-mode-changed %2 1\n");
        assert_eq!(
            notifs,
            vec![Notification::PaneModeChanged {
                pane_id: 2,
                mode: 1,
            }]
        );
    }

    #[test]
    fn parse_layout_change() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%layout-change @1 abc123,80x24,0,0\n");
        assert_eq!(
            notifs,
            vec![Notification::LayoutChange {
                window_id: 1,
                layout: "abc123,80x24,0,0".into(),
            }]
        );
    }

    #[test]
    fn parse_exit_no_reason() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%exit\n");
        assert_eq!(notifs, vec![Notification::Exit { reason: None }]);
    }

    #[test]
    fn parse_exit_with_reason() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%exit server exited\n");
        assert_eq!(
            notifs,
            vec![Notification::Exit {
                reason: Some("server exited".into()),
            }]
        );
    }

    #[test]
    fn parse_unknown_notification() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%future-thing arg1 arg2\n");
        assert_eq!(
            notifs,
            vec![Notification::Unknown {
                name: "future-thing".into(),
                args: "arg1 arg2".into(),
            }]
        );
    }

    // --- Output parsing ---

    #[test]
    fn parse_output_plain_text() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%output %0 hello world\n");
        assert_eq!(
            notifs,
            vec![Notification::Output {
                pane_id: 0,
                data: b"hello world".to_vec(),
            }]
        );
    }

    #[test]
    fn parse_output_octal_escapes() {
        let mut p = ControlModeParser::new();
        // \033 = ESC (0x1b), \012 = LF (0x0a)
        let notifs = p.feed(b"%output %1 \\033[31mred\\012\n");
        assert_eq!(
            notifs,
            vec![Notification::Output {
                pane_id: 1,
                data: vec![0x1b, b'[', b'3', b'1', b'm', b'r', b'e', b'd', 0x0a],
            }]
        );
    }

    #[test]
    fn parse_output_empty_data() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%output %5 \n");
        assert_eq!(
            notifs,
            vec![Notification::Output {
                pane_id: 5,
                data: Vec::new(),
            }]
        );
    }

    // --- Begin/End blocks ---

    #[test]
    fn parse_begin_end() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%begin 1234 0\n%end 1234 0\n");
        assert_eq!(
            notifs,
            vec![
                Notification::Begin {
                    command_number: 1234,
                    flags: 0,
                },
                Notification::End {
                    command_number: 1234,
                    flags: 0,
                },
            ]
        );
    }

    #[test]
    fn parse_error_response() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%error 42 session not found\n");
        assert_eq!(
            notifs,
            vec![Notification::Error {
                command_number: 42,
                message: "session not found".into(),
            }]
        );
    }

    // --- Edge cases ---

    #[test]
    fn partial_line_across_feeds() {
        let mut p = ControlModeParser::new();
        let n1 = p.feed(b"%sessions-");
        assert!(n1.is_empty(), "incomplete line should yield nothing");
        let n2 = p.feed(b"changed\n");
        assert_eq!(n2, vec![Notification::SessionsChanged]);
    }

    #[test]
    fn multiple_notifications_one_feed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"%sessions-changed\n%window-add @1\n%window-add @2\n");
        assert_eq!(
            notifs,
            vec![
                Notification::SessionsChanged,
                Notification::WindowAdd { window_id: 1 },
                Notification::WindowAdd { window_id: 2 },
            ]
        );
    }

    #[test]
    fn empty_feed() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"");
        assert!(notifs.is_empty());
    }

    #[test]
    fn blank_lines_ignored() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(b"\n\n%sessions-changed\n\n");
        assert_eq!(notifs, vec![Notification::SessionsChanged]);
    }

    #[test]
    fn interleaved_output_and_notifications() {
        let mut p = ControlModeParser::new();
        let notifs = p.feed(
            b"%output %0 data\n%window-add @1\n%output %0 more\n"
        );
        assert_eq!(notifs.len(), 3);
        assert!(matches!(notifs[0], Notification::Output { pane_id: 0, .. }));
        assert_eq!(notifs[1], Notification::WindowAdd { window_id: 1 });
        assert!(matches!(notifs[2], Notification::Output { pane_id: 0, .. }));
    }

    // --- Octal decode helper ---

    #[test]
    fn decode_octal_simple() {
        assert_eq!(decode_octal_escapes("abc"), b"abc");
    }

    #[test]
    fn decode_octal_escape_sequence() {
        // \033 = 27 = 0x1b
        assert_eq!(decode_octal_escapes("\\033"), vec![0x1b]);
    }

    #[test]
    fn decode_octal_mixed() {
        assert_eq!(
            decode_octal_escapes("A\\101B"),
            vec![b'A', 0x41, b'B']
        );
    }

    #[test]
    fn decode_octal_backslash_not_followed_by_digits() {
        assert_eq!(decode_octal_escapes("a\\bc"), b"a\\bc");
    }

    #[test]
    fn decode_octal_empty() {
        assert_eq!(decode_octal_escapes(""), b"");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p conch_tmux`
Expected: all tests pass (the implementation is included above since TDD for a parser is clearest when presented together)

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tmux/src/parser.rs
git commit -m "Add tmux control mode parser with full test coverage"
```

---

## Task 4: Command Builder

**Files:**
- Modify: `crates/conch_tmux/src/command.rs`

- [ ] **Step 1: Write command builder with tests**

Replace `crates/conch_tmux/src/command.rs`:

```rust
//! Tmux command builder for control mode.
//!
//! Each method returns a newline-terminated `String` ready to write to
//! the control mode connection's stdin.

/// Builds tmux command strings to send over a control mode connection.
pub struct CommandBuilder;

impl CommandBuilder {
    pub fn list_sessions() -> String {
        "list-sessions\n".into()
    }

    pub fn new_session(name: Option<&str>) -> String {
        match name {
            Some(n) => format!("new-session -d -s {}\n", quote(n)),
            None => "new-session -d\n".into(),
        }
    }

    pub fn kill_session(target: &str) -> String {
        format!("kill-session -t {}\n", quote(target))
    }

    pub fn rename_session(target: &str, new_name: &str) -> String {
        format!("rename-session -t {} {}\n", quote(target), quote(new_name))
    }

    pub fn switch_client(target: &str) -> String {
        format!("switch-client -t {}\n", quote(target))
    }

    pub fn detach_client() -> String {
        "detach-client\n".into()
    }

    pub fn new_window(target_session: &str) -> String {
        format!("new-window -t {}\n", quote(target_session))
    }

    pub fn kill_window(target: &str) -> String {
        format!("kill-window -t {}\n", quote(target))
    }

    pub fn rename_window(target: &str, new_name: &str) -> String {
        format!("rename-window -t {} {}\n", quote(target), quote(new_name))
    }

    pub fn split_window(target: &str, horizontal: bool) -> String {
        let flag = if horizontal { "-h" } else { "-v" };
        format!("split-window {} -t {}\n", flag, quote(target))
    }

    pub fn select_pane(target: &str) -> String {
        format!("select-pane -t {}\n", quote(target))
    }

    pub fn kill_pane(target: &str) -> String {
        format!("kill-pane -t {}\n", quote(target))
    }

    pub fn resize_pane(target: &str, cols: u16, rows: u16) -> String {
        format!("resize-pane -t {} -x {} -y {}\n", quote(target), cols, rows)
    }

    /// Send keys to a pane (used for writing terminal input).
    pub fn send_keys(target: &str, keys: &str) -> String {
        format!("send-keys -t {} -l -- {}\n", quote(target), quote(keys))
    }

    /// List windows in a session with a parseable format.
    pub fn list_windows(target_session: &str) -> String {
        format!(
            "list-windows -t {} -F '#{{window_id}} #{{window_name}} #{{window_active}}'\n",
            quote(target_session)
        )
    }

    /// List panes in a window with a parseable format.
    pub fn list_panes(target_window: &str) -> String {
        format!(
            "list-panes -t {} -F '#{{pane_id}} #{{pane_active}} #{{pane_width}} #{{pane_height}}'\n",
            quote(target_window)
        )
    }
}

/// Quote a tmux target/argument if it contains whitespace or special chars.
fn quote(s: &str) -> String {
    if s.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '\\') {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_sessions_cmd() {
        assert_eq!(CommandBuilder::list_sessions(), "list-sessions\n");
    }

    #[test]
    fn new_session_named() {
        assert_eq!(
            CommandBuilder::new_session(Some("work")),
            "new-session -d -s work\n"
        );
    }

    #[test]
    fn new_session_unnamed() {
        assert_eq!(CommandBuilder::new_session(None), "new-session -d\n");
    }

    #[test]
    fn kill_session_cmd() {
        assert_eq!(
            CommandBuilder::kill_session("old"),
            "kill-session -t old\n"
        );
    }

    #[test]
    fn rename_session_cmd() {
        assert_eq!(
            CommandBuilder::rename_session("old", "new"),
            "rename-session -t old new\n"
        );
    }

    #[test]
    fn switch_client_cmd() {
        assert_eq!(
            CommandBuilder::switch_client("work"),
            "switch-client -t work\n"
        );
    }

    #[test]
    fn detach_client_cmd() {
        assert_eq!(CommandBuilder::detach_client(), "detach-client\n");
    }

    #[test]
    fn new_window_cmd() {
        assert_eq!(
            CommandBuilder::new_window("work"),
            "new-window -t work\n"
        );
    }

    #[test]
    fn kill_window_cmd() {
        assert_eq!(
            CommandBuilder::kill_window("@1"),
            "kill-window -t @1\n"
        );
    }

    #[test]
    fn rename_window_cmd() {
        assert_eq!(
            CommandBuilder::rename_window("@1", "editor"),
            "rename-window -t @1 editor\n"
        );
    }

    #[test]
    fn split_window_horizontal() {
        assert_eq!(
            CommandBuilder::split_window("%1", true),
            "split-window -h -t %1\n"
        );
    }

    #[test]
    fn split_window_vertical() {
        assert_eq!(
            CommandBuilder::split_window("%1", false),
            "split-window -v -t %1\n"
        );
    }

    #[test]
    fn select_pane_cmd() {
        assert_eq!(
            CommandBuilder::select_pane("%3"),
            "select-pane -t %3\n"
        );
    }

    #[test]
    fn kill_pane_cmd() {
        assert_eq!(
            CommandBuilder::kill_pane("%2"),
            "kill-pane -t %2\n"
        );
    }

    #[test]
    fn resize_pane_cmd() {
        assert_eq!(
            CommandBuilder::resize_pane("%1", 120, 40),
            "resize-pane -t %1 -x 120 -y 40\n"
        );
    }

    #[test]
    fn session_name_with_spaces_is_quoted() {
        assert_eq!(
            CommandBuilder::new_session(Some("my project")),
            "new-session -d -s \"my project\"\n"
        );
    }

    #[test]
    fn session_name_with_quotes_is_escaped() {
        assert_eq!(
            CommandBuilder::kill_session("test\"name"),
            "kill-session -t \"test\\\"name\"\n"
        );
    }

    #[test]
    fn quote_plain_string() {
        assert_eq!(quote("simple"), "simple");
    }

    #[test]
    fn quote_string_with_space() {
        assert_eq!(quote("has space"), "\"has space\"");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p conch_tmux`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tmux/src/command.rs
git commit -m "Add tmux command builder with quoting and tests"
```

---

## Task 5: Session Model

**Files:**
- Modify: `crates/conch_tmux/src/session.rs`

- [ ] **Step 1: Write session model with tests**

Replace `crates/conch_tmux/src/session.rs`:

```rust
//! In-memory tmux session model.

use crate::protocol::Notification;

/// A tmux session.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TmuxSession {
    pub id: u64,
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
    pub created: Option<u64>,
}

/// Tracks the set of known tmux sessions.
#[derive(Debug, Clone, Default)]
pub struct SessionList {
    sessions: Vec<TmuxSession>,
}

impl SessionList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sessions(&self) -> &[TmuxSession] {
        &self.sessions
    }

    /// Replace the full session list from `list-sessions` command output.
    ///
    /// Expected format per line: `$ID name: N windows (created <time>) (attached)`
    /// The exact format depends on the tmux format string we use.
    /// We use a simpler custom format:
    ///   `list-sessions -F '#{session_id} #{session_name} #{session_windows} #{session_attached} #{session_created}'`
    pub fn update_from_list_output(&mut self, raw: &str) {
        let mut new_sessions = Vec::new();
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(session) = parse_session_line(line) {
                new_sessions.push(session);
            }
        }
        self.sessions = new_sessions;
    }

    /// Apply a single notification to update the model.
    pub fn apply_notification(&mut self, notif: &Notification) {
        match notif {
            Notification::SessionsChanged => {
                // External change — caller should follow up with list-sessions
                // to get the full picture. We can't update incrementally from
                // this notification alone.
            }
            Notification::SessionRenamed { session_id, name } => {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == *session_id) {
                    s.name = name.clone();
                }
            }
            Notification::SessionChanged { session_id, name } => {
                // Mark the new session as attached, unmark others
                for s in &mut self.sessions {
                    s.attached = s.id == *session_id;
                }
                // Update name in case it changed
                if let Some(s) = self.sessions.iter_mut().find(|s| s.id == *session_id) {
                    s.name = name.clone();
                }
            }
            Notification::WindowAdd { .. } => {
                // Increment window count on the attached session
                if let Some(s) = self.sessions.iter_mut().find(|s| s.attached) {
                    s.window_count += 1;
                }
            }
            Notification::WindowClose { .. } => {
                if let Some(s) = self.sessions.iter_mut().find(|s| s.attached) {
                    s.window_count = s.window_count.saturating_sub(1);
                }
            }
            _ => {}
        }
    }
}

/// Parse a line from our custom `list-sessions -F` output.
///
/// Format: `$ID name windows attached created`
/// Example: `$1 my-session 3 1 1711990800`
fn parse_session_line(line: &str) -> Option<TmuxSession> {
    let parts: Vec<&str> = line.splitn(5, ' ').collect();
    if parts.len() < 4 {
        return None;
    }
    let id = parts[0].strip_prefix('$')?.parse().ok()?;
    let name = parts[1].to_string();
    let window_count = parts[2].parse().ok()?;
    let attached = parts[3] == "1";
    let created = parts.get(4).and_then(|s| s.parse().ok());

    Some(TmuxSession {
        id,
        name,
        window_count,
        attached,
        created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_session_list() {
        let list = SessionList::new();
        assert!(list.sessions().is_empty());
    }

    #[test]
    fn update_from_list_output() {
        let mut list = SessionList::new();
        list.update_from_list_output(
            "$1 work 3 1 1711990800\n$2 scratch 1 0 1711990900\n",
        );
        assert_eq!(list.sessions().len(), 2);
        assert_eq!(list.sessions()[0].name, "work");
        assert_eq!(list.sessions()[0].window_count, 3);
        assert!(list.sessions()[0].attached);
        assert_eq!(list.sessions()[1].name, "scratch");
        assert!(!list.sessions()[1].attached);
    }

    #[test]
    fn update_replaces_previous() {
        let mut list = SessionList::new();
        list.update_from_list_output("$1 old 1 0 0\n");
        assert_eq!(list.sessions().len(), 1);
        list.update_from_list_output("$2 new 2 1 0\n");
        assert_eq!(list.sessions().len(), 1);
        assert_eq!(list.sessions()[0].name, "new");
    }

    #[test]
    fn apply_session_renamed() {
        let mut list = SessionList::new();
        list.update_from_list_output("$1 old-name 2 1 0\n");
        list.apply_notification(&Notification::SessionRenamed {
            session_id: 1,
            name: "new-name".into(),
        });
        assert_eq!(list.sessions()[0].name, "new-name");
    }

    #[test]
    fn apply_session_renamed_unknown_id_is_noop() {
        let mut list = SessionList::new();
        list.update_from_list_output("$1 work 2 1 0\n");
        list.apply_notification(&Notification::SessionRenamed {
            session_id: 99,
            name: "new-name".into(),
        });
        assert_eq!(list.sessions()[0].name, "work");
    }

    #[test]
    fn apply_session_changed_updates_attached() {
        let mut list = SessionList::new();
        list.update_from_list_output("$1 work 2 1 0\n$2 play 1 0 0\n");
        list.apply_notification(&Notification::SessionChanged {
            session_id: 2,
            name: "play".into(),
        });
        assert!(!list.sessions()[0].attached);
        assert!(list.sessions()[1].attached);
    }

    #[test]
    fn apply_window_add_increments_count() {
        let mut list = SessionList::new();
        list.update_from_list_output("$1 work 2 1 0\n");
        list.apply_notification(&Notification::WindowAdd { window_id: 5 });
        assert_eq!(list.sessions()[0].window_count, 3);
    }

    #[test]
    fn apply_window_close_decrements_count() {
        let mut list = SessionList::new();
        list.update_from_list_output("$1 work 2 1 0\n");
        list.apply_notification(&Notification::WindowClose { window_id: 1 });
        assert_eq!(list.sessions()[0].window_count, 1);
    }

    #[test]
    fn apply_window_close_does_not_underflow() {
        let mut list = SessionList::new();
        list.update_from_list_output("$1 work 0 1 0\n");
        list.apply_notification(&Notification::WindowClose { window_id: 1 });
        assert_eq!(list.sessions()[0].window_count, 0);
    }

    #[test]
    fn parse_session_line_valid() {
        let s = parse_session_line("$1 my-session 3 1 1711990800").unwrap();
        assert_eq!(s.id, 1);
        assert_eq!(s.name, "my-session");
        assert_eq!(s.window_count, 3);
        assert!(s.attached);
        assert_eq!(s.created, Some(1711990800));
    }

    #[test]
    fn parse_session_line_not_attached() {
        let s = parse_session_line("$2 dev 1 0 0").unwrap();
        assert!(!s.attached);
    }

    #[test]
    fn parse_session_line_invalid_returns_none() {
        assert!(parse_session_line("garbage").is_none());
        assert!(parse_session_line("").is_none());
    }

    #[test]
    fn update_from_empty_string() {
        let mut list = SessionList::new();
        list.update_from_list_output("");
        assert!(list.sessions().is_empty());
    }

    #[test]
    fn update_skips_blank_lines() {
        let mut list = SessionList::new();
        list.update_from_list_output("\n\n$1 work 1 0 0\n\n");
        assert_eq!(list.sessions().len(), 1);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p conch_tmux`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tmux/src/session.rs
git commit -m "Add tmux session model with notification-driven updates"
```

---

## Task 6: Control Mode Connection

**Files:**
- Modify: `crates/conch_tmux/src/connection.rs`

This module wraps child process management. It cannot be fully unit tested without tmux installed, but we structure it for clarity and test what we can.

- [ ] **Step 1: Write connection module**

Replace `crates/conch_tmux/src/connection.rs`:

```rust
//! Control mode connection manager.
//!
//! Manages a `tmux -CC` child process. The caller is responsible for
//! reading from [`reader()`] in a loop and feeding bytes to
//! [`parse_bytes()`].

use std::io::{self, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::parser::ControlModeParser;
use crate::protocol::Notification;

/// Manages a `tmux -CC` child process.
pub struct ControlModeConnection {
    child: Child,
    writer: BufWriter<ChildStdin>,
    parser: ControlModeParser,
    next_command_number: u64,
}

impl ControlModeConnection {
    /// Spawn a tmux control mode process.
    ///
    /// `binary` is the path to the tmux executable (e.g., `"tmux"` or
    /// `"/opt/homebrew/bin/tmux"`).
    ///
    /// `args` should include `-CC` and the tmux subcommand, e.g.:
    /// - `["-CC", "new-session", "-A", "-s", "myname"]` — attach or create
    /// - `["-CC", "attach-session", "-t", "myname"]` — attach existing
    pub fn new(binary: &str, args: &[&str]) -> io::Result<Self> {
        let mut child = Command::new(binary)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "failed to open tmux stdin"))?;

        Ok(Self {
            child,
            writer: BufWriter::new(stdin),
            parser: ControlModeParser::new(),
            next_command_number: 0,
        })
    }

    /// Send a command string to tmux. The command should already be
    /// newline-terminated (as returned by [`CommandBuilder`] methods).
    ///
    /// Returns the command number that will appear in the `%begin`/`%end`
    /// response framing.
    pub fn send_command(&mut self, cmd: &str) -> io::Result<u64> {
        let num = self.next_command_number;
        self.next_command_number += 1;
        self.writer.write_all(cmd.as_bytes())?;
        self.writer.flush()?;
        Ok(num)
    }

    /// Returns a mutable reference to the child's stdout for reading.
    ///
    /// The caller should read from this in a loop and pass the bytes to
    /// [`parse_bytes()`].
    pub fn reader(&mut self) -> &mut ChildStdout {
        self.child
            .stdout
            .as_mut()
            .expect("stdout was captured at spawn")
    }

    /// Feed raw bytes from stdout through the parser and return any
    /// complete notifications.
    pub fn parse_bytes(&mut self, data: &[u8]) -> Vec<Notification> {
        self.parser.feed(data)
    }

    /// Get the PID of the tmux child process.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Kill the tmux child process. This causes tmux to detach the client.
    pub fn kill(mut self) -> io::Result<()> {
        self.child.kill()?;
        self.child.wait()?;
        Ok(())
    }
}

impl Drop for ControlModeConnection {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
```

- [ ] **Step 2: Verify crate compiles**

Run: `cargo check -p conch_tmux`
Expected: compiles cleanly

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tmux/src/connection.rs
git commit -m "Add tmux control mode connection manager"
```

---

## Task 7: Configuration Types in `conch_core`

**Files:**
- Modify: `crates/conch_core/src/config/terminal.rs`
- Modify: `crates/conch_core/src/config/persistent.rs`

- [ ] **Step 1: Add `TerminalBackend` and `TmuxConfig` to terminal.rs**

In `crates/conch_core/src/config/terminal.rs`, add after the existing imports (line 5):

```rust
/// Terminal backend mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalBackend {
    #[default]
    Local,
    Tmux,
}

/// Tmux-specific configuration. Only used when `backend = "tmux"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TmuxConfig {
    /// Path to tmux binary. Empty string means search `$PATH`.
    pub binary: String,
    /// What to do when a window opens in tmux mode.
    pub startup_behavior: TmuxStartupBehavior,
    /// What "New Tab" does in tmux mode.
    pub new_tab_behavior: TmuxNewTabBehavior,
    /// What "New Window" does in tmux mode.
    pub new_window_behavior: TmuxNewWindowBehavior,
}

impl Default for TmuxConfig {
    fn default() -> Self {
        Self {
            binary: String::new(),
            startup_behavior: TmuxStartupBehavior::default(),
            new_tab_behavior: TmuxNewTabBehavior::default(),
            new_window_behavior: TmuxNewWindowBehavior::default(),
        }
    }
}

impl TmuxConfig {
    /// Returns the tmux binary path, defaulting to `"tmux"` if empty.
    pub fn resolved_binary(&self) -> &str {
        let b = self.binary.trim();
        if b.is_empty() { "tmux" } else { b }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TmuxStartupBehavior {
    #[default]
    AttachLastSession,
    ShowSessionPicker,
    CreateNewSession,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TmuxNewTabBehavior {
    #[default]
    NewTmuxWindow,
    SessionPicker,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TmuxNewWindowBehavior {
    #[default]
    AttachSameSession,
    ShowSessionPicker,
}
```

Then update `TerminalConfig` to include the new fields:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    pub backend: TerminalBackend,
    pub tmux: TmuxConfig,
    pub shell: TerminalShell,
    pub env: HashMap<String, String>,
    pub cursor: CursorConfig,
    pub scroll_sensitivity: f32,
    pub font: FontConfig,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            backend: TerminalBackend::default(),
            tmux: TmuxConfig::default(),
            shell: TerminalShell::default(),
            env: HashMap::new(),
            cursor: CursorConfig::default(),
            scroll_sensitivity: 0.15,
            font: FontConfig::default(),
        }
    }
}
```

- [ ] **Step 2: Add `last_tmux_session` to persistent.rs**

In `crates/conch_core/src/config/persistent.rs`, add the field to `PersistentState`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistentState {
    pub layout: LayoutConfig,
    /// Names of plugins that were loaded when the app last exited.
    pub loaded_plugins: Vec<String>,
    /// Last tmux session name for `attach_last_session` startup behavior.
    pub last_tmux_session: Option<String>,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            layout: LayoutConfig::default(),
            loaded_plugins: Vec::new(),
            last_tmux_session: None,
        }
    }
}
```

- [ ] **Step 3: Add tests for new config types**

Add to the `#[cfg(test)] mod tests` in `terminal.rs`:

```rust
    #[test]
    fn default_backend_is_local() {
        let cfg = TerminalConfig::default();
        assert_eq!(cfg.backend, TerminalBackend::Local);
    }

    #[test]
    fn parse_tmux_backend() {
        let cfg: TerminalConfig = toml::from_str(r#"backend = "tmux""#).unwrap();
        assert_eq!(cfg.backend, TerminalBackend::Tmux);
    }

    #[test]
    fn tmux_config_defaults() {
        let cfg: TerminalConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.tmux.binary, "");
        assert_eq!(cfg.tmux.startup_behavior, TmuxStartupBehavior::AttachLastSession);
        assert_eq!(cfg.tmux.new_tab_behavior, TmuxNewTabBehavior::NewTmuxWindow);
        assert_eq!(cfg.tmux.new_window_behavior, TmuxNewWindowBehavior::AttachSameSession);
    }

    #[test]
    fn tmux_config_serde_roundtrip() {
        let cfg = TmuxConfig {
            binary: "/opt/homebrew/bin/tmux".into(),
            startup_behavior: TmuxStartupBehavior::ShowSessionPicker,
            new_tab_behavior: TmuxNewTabBehavior::SessionPicker,
            new_window_behavior: TmuxNewWindowBehavior::ShowSessionPicker,
        };
        let s = toml::to_string(&cfg).unwrap();
        let parsed: TmuxConfig = toml::from_str(&s).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn backward_compat_no_tmux_section() {
        let toml_str = r#"
            [shell]
            program = "/bin/zsh"
            args = ["-l"]
            scroll_sensitivity = 0.2
        "#;
        let cfg: TerminalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.backend, TerminalBackend::Local);
        assert_eq!(cfg.shell.program, "/bin/zsh");
        assert_eq!(cfg.tmux, TmuxConfig::default());
    }

    #[test]
    fn resolved_binary_empty() {
        let cfg = TmuxConfig::default();
        assert_eq!(cfg.resolved_binary(), "tmux");
    }

    #[test]
    fn resolved_binary_explicit() {
        let cfg = TmuxConfig {
            binary: "/opt/homebrew/bin/tmux".into(),
            ..Default::default()
        };
        assert_eq!(cfg.resolved_binary(), "/opt/homebrew/bin/tmux");
    }

    #[test]
    fn resolved_binary_whitespace_only() {
        let cfg = TmuxConfig {
            binary: "   ".into(),
            ..Default::default()
        };
        assert_eq!(cfg.resolved_binary(), "tmux");
    }
```

Add to the `#[cfg(test)] mod tests` in `persistent.rs`:

```rust
    #[test]
    fn last_tmux_session_default_is_none() {
        let ps = PersistentState::default();
        assert!(ps.last_tmux_session.is_none());
    }

    #[test]
    fn last_tmux_session_roundtrip() {
        let state = PersistentState {
            last_tmux_session: Some("my-session".into()),
            ..Default::default()
        };
        let s = toml::to_string(&state).unwrap();
        let parsed: PersistentState = toml::from_str(&s).unwrap();
        assert_eq!(parsed.last_tmux_session, Some("my-session".into()));
    }

    #[test]
    fn backward_compat_no_last_tmux_session() {
        let toml_str = r#"
            loaded_plugins = ["my-plugin"]
            [layout]
            zoom_factor = 1.5
        "#;
        let ps: PersistentState = toml::from_str(toml_str).unwrap();
        assert!(ps.last_tmux_session.is_none());
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p conch_core`
Expected: all tests pass (existing + new)

- [ ] **Step 5: Verify full workspace compiles**

Run: `cargo check`
Expected: compiles. If `conch_tauri` references `TerminalConfig` fields, the new `backend` and `tmux` fields with `#[serde(default)]` should be transparent.

- [ ] **Step 6: Commit**

```bash
git add crates/conch_core/src/config/terminal.rs crates/conch_core/src/config/persistent.rs
git commit -m "Add tmux backend config types and state persistence"
```

---

## Task 8: Tauri Integration — Event Types

**Files:**
- Modify: `crates/conch_tauri/Cargo.toml`
- Create: `crates/conch_tauri/src/tmux/mod.rs`
- Create: `crates/conch_tauri/src/tmux/events.rs`

- [ ] **Step 1: Add `conch_tmux` dependency to `conch_tauri`**

In `crates/conch_tauri/Cargo.toml`, add under `[dependencies]`:

```toml
conch_tmux = { workspace = true, features = ["serde"] }
```

- [ ] **Step 2: Create events module**

Create `crates/conch_tauri/src/tmux/events.rs`:

```rust
//! Serializable event payloads for tmux → frontend communication.

use serde::Serialize;

#[derive(Clone, Serialize)]
pub(crate) struct TmuxSessionInfo {
    pub id: u64,
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
    pub created: Option<u64>,
}

impl From<&conch_tmux::TmuxSession> for TmuxSessionInfo {
    fn from(s: &conch_tmux::TmuxSession) -> Self {
        Self {
            id: s.id,
            name: s.name.clone(),
            window_count: s.window_count,
            attached: s.attached,
            created: s.created,
        }
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct TmuxConnectedEvent {
    pub session: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct TmuxDisconnectedEvent {
    pub reason: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct TmuxOutputEvent {
    pub pane_id: u64,
    pub data: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct TmuxWindowEvent {
    pub window_id: u64,
    pub name: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct TmuxPaneEvent {
    pub window_id: Option<u64>,
    pub pane_id: u64,
}

#[derive(Clone, Serialize)]
pub(crate) struct TmuxLayoutEvent {
    pub window_id: u64,
    pub layout: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct TmuxSessionsChangedEvent {
    pub sessions: Vec<TmuxSessionInfo>,
}
```

- [ ] **Step 3: Create tmux mod.rs with TmuxState**

Create `crates/conch_tauri/src/tmux/mod.rs`:

```rust
//! Tmux backend integration for Tauri.
//!
//! Bridges `conch_tmux` protocol layer to Tauri events and commands.

pub(crate) mod bridge;
pub(crate) mod events;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

use conch_tmux::{ControlModeConnection, SessionList};

/// Per-window tmux connection state.
pub(crate) struct TmuxWindowConnection {
    pub connection: ControlModeConnection,
    pub reader_handle: Option<JoinHandle<()>>,
    pub attached_session: Option<String>,
}

/// App-level tmux state, registered as Tauri managed state.
pub(crate) struct TmuxState {
    /// Per-window connections, keyed by window label.
    pub connections: Mutex<HashMap<String, TmuxWindowConnection>>,
    /// Shared session list, updated by any connection's notifications.
    pub sessions: Arc<RwLock<SessionList>>,
    /// Resolved tmux binary path.
    pub binary: String,
}

impl TmuxState {
    pub(crate) fn new(binary: String) -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            sessions: Arc::new(RwLock::new(SessionList::new())),
            binary,
        }
    }
}
```

- [ ] **Step 4: Create bridge stub**

Create `crates/conch_tauri/src/tmux/bridge.rs`:

```rust
//! Reader thread that drives the control mode parser and emits Tauri events.

use std::io::Read;
use std::sync::{Arc, RwLock};

use conch_tmux::{ControlModeConnection, Notification, SessionList};
use tauri::{AppHandle, Emitter};

use super::events::*;

/// Spawn a reader loop for a control mode connection.
///
/// Reads from the connection's stdout, parses notifications, updates the
/// shared session list, and emits Tauri events to the specified window.
/// Returns the join handle for the reader thread.
pub(crate) fn spawn_reader_thread(
    app: AppHandle,
    window_label: String,
    mut connection: ControlModeConnection,
    sessions: Arc<RwLock<SessionList>>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(format!("tmux-reader-{window_label}"))
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match connection.reader().read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        for notif in connection.parse_bytes(&buf[..n]) {
                            // Update shared model
                            if let Ok(mut list) = sessions.write() {
                                list.apply_notification(&notif);
                            }
                            // Emit to frontend
                            emit_notification(&app, &window_label, &notif, &sessions);
                        }
                    }
                    Err(e) => {
                        log::error!("tmux reader error for {window_label}: {e}");
                        break;
                    }
                }
            }
            let _ = app.emit_to(
                &window_label,
                "tmux-disconnected",
                TmuxDisconnectedEvent { reason: None },
            );
        })
        .expect("failed to spawn tmux reader thread")
}

fn emit_notification(
    app: &AppHandle,
    window_label: &str,
    notif: &Notification,
    sessions: &Arc<RwLock<SessionList>>,
) {
    match notif {
        Notification::Output { pane_id, data } => {
            let _ = app.emit_to(
                window_label,
                "tmux-output",
                TmuxOutputEvent {
                    pane_id: *pane_id,
                    data: String::from_utf8_lossy(data).into_owned(),
                },
            );
        }
        Notification::WindowAdd { window_id } => {
            let _ = app.emit_to(
                window_label,
                "tmux-window-add",
                TmuxWindowEvent {
                    window_id: *window_id,
                    name: None,
                },
            );
        }
        Notification::WindowClose { window_id } => {
            let _ = app.emit_to(
                window_label,
                "tmux-window-close",
                TmuxWindowEvent {
                    window_id: *window_id,
                    name: None,
                },
            );
        }
        Notification::WindowRenamed { window_id, name } => {
            let _ = app.emit_to(
                window_label,
                "tmux-window-renamed",
                TmuxWindowEvent {
                    window_id: *window_id,
                    name: Some(name.clone()),
                },
            );
        }
        Notification::LayoutChange { window_id, layout } => {
            let _ = app.emit_to(
                window_label,
                "tmux-layout-change",
                TmuxLayoutEvent {
                    window_id: *window_id,
                    layout: layout.clone(),
                },
            );
        }
        Notification::WindowPaneChanged { window_id, pane_id } => {
            let _ = app.emit_to(
                window_label,
                "tmux-pane-changed",
                TmuxPaneEvent {
                    window_id: Some(*window_id),
                    pane_id: *pane_id,
                },
            );
        }
        Notification::PaneModeChanged { pane_id, .. } => {
            // Mode changes (copy mode, etc.) — logged but not acted on in phase 1
            log::debug!("tmux pane mode changed: %{pane_id}");
        }
        Notification::SessionsChanged
        | Notification::SessionChanged { .. }
        | Notification::SessionRenamed { .. } => {
            if let Ok(list) = sessions.read() {
                let infos: Vec<TmuxSessionInfo> =
                    list.sessions().iter().map(TmuxSessionInfo::from).collect();
                let _ = app.emit_to(
                    window_label,
                    "tmux-sessions-changed",
                    TmuxSessionsChangedEvent { sessions: infos },
                );
            }
        }
        Notification::Exit { reason } => {
            let _ = app.emit_to(
                window_label,
                "tmux-disconnected",
                TmuxDisconnectedEvent {
                    reason: reason.clone(),
                },
            );
        }
        _ => {}
    }
}
```

- [ ] **Step 5: Register tmux module in lib.rs**

In `crates/conch_tauri/src/lib.rs`, add:

```rust
mod tmux;
```

near the other `mod` declarations at the top of the file.

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p conch_tauri`
Expected: compiles (warnings about unused code are OK at this stage — the commands and state registration come in the next task)

- [ ] **Step 7: Commit**

```bash
git add crates/conch_tauri/Cargo.toml crates/conch_tauri/src/tmux/
git commit -m "Add tmux Tauri integration layer: state, events, reader bridge"
```

---

## Task 9: Tauri Commands

**Files:**
- Modify: `crates/conch_tauri/src/tmux/mod.rs`
- Modify: `crates/conch_tauri/src/lib.rs`

- [ ] **Step 1: Add Tauri commands to tmux/mod.rs**

Append to `crates/conch_tauri/src/tmux/mod.rs`:

```rust
use tauri::{AppHandle, Emitter, WebviewWindow};

use events::TmuxSessionInfo;

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn tmux_connect(
    window: WebviewWindow,
    app: AppHandle,
    state: tauri::State<'_, TmuxState>,
    session_name: String,
) -> Result<(), String> {
    let window_label = window.label().to_string();
    let args = vec![
        "-CC",
        "new-session",
        "-A",
        "-s",
        &session_name,
    ];

    let connection = ControlModeConnection::new(&state.binary, &args)
        .map_err(|e| format!("Failed to start tmux: {e}"))?;

    let sessions = Arc::clone(&state.sessions);
    let reader_handle = bridge::spawn_reader_thread(
        app.clone(),
        window_label.clone(),
        // We need to hand off the connection to the reader thread,
        // but also keep a way to send commands. This requires splitting.
        // For now, we store and use send_command through the mutex.
        // Actually, the connection must be owned by the reader thread for reading,
        // but we need write access for commands. This needs a channel-based approach.
        // See Step 2 for the revised design.
    );

    Ok(())
}
```

Actually, there's an ownership issue: `ControlModeConnection` owns both the reader (stdout) and writer (stdin). The reader thread needs to read continuously, but we also need to send commands from Tauri command handlers. This requires splitting the connection.

- [ ] **Step 2: Revise connection to support split ownership**

Go back to `crates/conch_tmux/src/connection.rs` and refactor to split the connection into a reader half and a writer half:

```rust
//! Control mode connection manager.

use std::io::{self, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::parser::ControlModeParser;
use crate::protocol::Notification;

/// The write half of a control mode connection.
///
/// Send commands to tmux by writing to this.
pub struct ConnectionWriter {
    writer: BufWriter<ChildStdin>,
}

impl ConnectionWriter {
    /// Send a command string to tmux. The command should be
    /// newline-terminated (as returned by [`CommandBuilder`] methods).
    pub fn send_command(&mut self, cmd: &str) -> io::Result<()> {
        self.writer.write_all(cmd.as_bytes())?;
        self.writer.flush()
    }
}

/// The read half of a control mode connection.
///
/// Read bytes from this and feed them to the parser.
pub struct ConnectionReader {
    stdout: ChildStdout,
    parser: ControlModeParser,
}

impl ConnectionReader {
    /// Get the raw stdout for reading bytes.
    pub fn stdout(&mut self) -> &mut ChildStdout {
        &mut self.stdout
    }

    /// Feed raw bytes through the parser and return notifications.
    pub fn parse_bytes(&mut self, data: &[u8]) -> Vec<Notification> {
        self.parser.feed(data)
    }
}

/// A handle to the tmux child process. Drop this to kill tmux.
pub struct ConnectionHandle {
    child: Child,
}

impl ConnectionHandle {
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn kill(mut self) -> io::Result<()> {
        self.child.kill()?;
        self.child.wait()?;
        Ok(())
    }
}

impl Drop for ConnectionHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawn a tmux control mode process and split into reader, writer, and handle.
///
/// `binary` is the path to tmux. `args` should include `-CC` and the subcommand.
pub fn spawn(binary: &str, args: &[&str]) -> io::Result<(ConnectionReader, ConnectionWriter, ConnectionHandle)> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "failed to open tmux stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "failed to open tmux stdout"))?;

    Ok((
        ConnectionReader {
            stdout,
            parser: ControlModeParser::new(),
        },
        ConnectionWriter {
            writer: BufWriter::new(stdin),
        },
        ConnectionHandle { child },
    ))
}
```

Also update `crates/conch_tmux/src/lib.rs` to export the new types:

```rust
pub use connection::{spawn, ConnectionHandle, ConnectionReader, ConnectionWriter};
```

And remove the old `ControlModeConnection` export.

- [ ] **Step 3: Update TmuxWindowConnection to use split types**

Revise `crates/conch_tauri/src/tmux/mod.rs`:

```rust
//! Tmux backend integration for Tauri.

pub(crate) mod bridge;
pub(crate) mod events;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

use conch_tmux::{CommandBuilder, ConnectionHandle, ConnectionWriter, SessionList};
use tauri::{AppHandle, Emitter, WebviewWindow};

use events::{TmuxConnectedEvent, TmuxSessionInfo};

/// Per-window tmux connection state.
pub(crate) struct TmuxWindowConnection {
    pub writer: ConnectionWriter,
    pub handle: ConnectionHandle,
    pub reader_join: Option<JoinHandle<()>>,
    pub attached_session: Option<String>,
}

/// App-level tmux state.
pub(crate) struct TmuxState {
    pub connections: Mutex<HashMap<String, TmuxWindowConnection>>,
    pub sessions: Arc<RwLock<SessionList>>,
    pub binary: String,
}

impl TmuxState {
    pub(crate) fn new(binary: String) -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            sessions: Arc::new(RwLock::new(SessionList::new())),
            binary,
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn tmux_connect(
    window: WebviewWindow,
    app: AppHandle,
    state: tauri::State<'_, TmuxState>,
    session_name: String,
) -> Result<(), String> {
    let window_label = window.label().to_string();
    let binary = state.binary.clone();
    let session = session_name.clone();

    let (reader, writer, handle) = conch_tmux::spawn(
        &binary,
        &["-CC", "new-session", "-A", "-s", &session],
    )
    .map_err(|e| format!("Failed to start tmux: {e}"))?;

    let sessions = Arc::clone(&state.sessions);
    let reader_join = bridge::spawn_reader_thread(app.clone(), window_label.clone(), reader, sessions);

    let conn = TmuxWindowConnection {
        writer,
        handle,
        reader_join: Some(reader_join),
        attached_session: Some(session_name.clone()),
    };

    state
        .connections
        .lock()
        .map_err(|e| e.to_string())?
        .insert(window_label.clone(), conn);

    let _ = app.emit_to(
        &window_label,
        "tmux-connected",
        TmuxConnectedEvent {
            session: session_name,
        },
    );

    Ok(())
}

#[tauri::command]
pub(crate) fn tmux_disconnect(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    if let Some(conn) = conns.remove(&label) {
        // Dropping handle kills the child, which causes the reader to EOF
        drop(conn);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn tmux_list_sessions(
    state: tauri::State<'_, TmuxState>,
) -> Result<Vec<TmuxSessionInfo>, String> {
    let list = state.sessions.read().map_err(|e| e.to_string())?;
    Ok(list.sessions().iter().map(TmuxSessionInfo::from).collect())
}

#[tauri::command]
pub(crate) fn tmux_create_session(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    name: Option<String>,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let cmd = CommandBuilder::new_session(name.as_deref());
    conn.writer.send_command(&cmd).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_kill_session(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    name: String,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    conn.writer
        .send_command(&CommandBuilder::kill_session(&name))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_rename_session(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    conn.writer
        .send_command(&CommandBuilder::rename_session(&old_name, &new_name))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_new_window(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let session = conn
        .attached_session
        .as_deref()
        .ok_or("Not attached to a session")?;
    conn.writer
        .send_command(&CommandBuilder::new_window(session))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_close_window(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    window_id: u64,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let target = format!("@{window_id}");
    conn.writer
        .send_command(&CommandBuilder::kill_window(&target))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_rename_window(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    window_id: u64,
    name: String,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let target = format!("@{window_id}");
    conn.writer
        .send_command(&CommandBuilder::rename_window(&target, &name))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_split_pane(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    pane_id: u64,
    horizontal: bool,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let target = format!("%{pane_id}");
    conn.writer
        .send_command(&CommandBuilder::split_window(&target, horizontal))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_close_pane(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    pane_id: u64,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let target = format!("%{pane_id}");
    conn.writer
        .send_command(&CommandBuilder::kill_pane(&target))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_select_pane(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    pane_id: u64,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let target = format!("%{pane_id}");
    conn.writer
        .send_command(&CommandBuilder::select_pane(&target))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_write_to_pane(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    pane_id: u64,
    data: String,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let target = format!("%{pane_id}");
    conn.writer
        .send_command(&CommandBuilder::send_keys(&target, &data))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn tmux_resize_pane(
    window: WebviewWindow,
    state: tauri::State<'_, TmuxState>,
    pane_id: u64,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let label = window.label().to_string();
    let mut conns = state.connections.lock().map_err(|e| e.to_string())?;
    let conn = conns
        .get_mut(&label)
        .ok_or("No tmux connection for this window")?;
    let target = format!("%{pane_id}");
    conn.writer
        .send_command(&CommandBuilder::resize_pane(&target, cols, rows))
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Update bridge.rs to use ConnectionReader**

Revise `crates/conch_tauri/src/tmux/bridge.rs` — change the function signature to accept `ConnectionReader` instead of `ControlModeConnection`:

```rust
use conch_tmux::ConnectionReader;

pub(crate) fn spawn_reader_thread(
    app: AppHandle,
    window_label: String,
    mut reader: ConnectionReader,
    sessions: Arc<RwLock<SessionList>>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(format!("tmux-reader-{window_label}"))
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.stdout().read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        for notif in reader.parse_bytes(&buf[..n]) {
                            if let Ok(mut list) = sessions.write() {
                                list.apply_notification(&notif);
                            }
                            emit_notification(&app, &window_label, &notif, &sessions);
                        }
                    }
                    Err(e) => {
                        log::error!("tmux reader error for {window_label}: {e}");
                        break;
                    }
                }
            }
            let _ = app.emit_to(
                &window_label,
                "tmux-disconnected",
                TmuxDisconnectedEvent { reason: None },
            );
        })
        .expect("failed to spawn tmux reader thread")
}
```

- [ ] **Step 5: Register TmuxState and commands in lib.rs**

In `crates/conch_tauri/src/lib.rs`:

1. Add `mod tmux;` near the top with other module declarations.

2. In the Tauri builder `.manage()` chain, add:

```rust
.manage(tmux::TmuxState::new(
    config.terminal.tmux.resolved_binary().to_string(),
))
```

3. In the `.invoke_handler(tauri::generate_handler![...])` macro, add all the tmux commands:

```rust
tmux::tmux_connect,
tmux::tmux_disconnect,
tmux::tmux_list_sessions,
tmux::tmux_create_session,
tmux::tmux_kill_session,
tmux::tmux_rename_session,
tmux::tmux_new_window,
tmux::tmux_close_window,
tmux::tmux_rename_window,
tmux::tmux_split_pane,
tmux::tmux_close_pane,
tmux::tmux_select_pane,
tmux::tmux_write_to_pane,
tmux::tmux_resize_pane,
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p conch_tauri`
Expected: compiles (fix any type mismatches the compiler finds)

- [ ] **Step 7: Commit**

```bash
git add crates/conch_tmux/src/connection.rs crates/conch_tmux/src/lib.rs crates/conch_tauri/src/tmux/ crates/conch_tauri/src/lib.rs
git commit -m "Add tmux Tauri commands and split connection ownership"
```

---

## Task 10: Frontend — Backend Router

**Files:**
- Create: `crates/conch_tauri/frontend/app/backend-router.js`

- [ ] **Step 1: Write backend router module**

Create `crates/conch_tauri/frontend/app/backend-router.js`:

```javascript
/**
 * Backend Router — routes tab/pane actions by backend mode.
 *
 * In 'local' mode, actions go through the existing PTY commands.
 * In 'tmux' mode, actions go through the tmux Tauri commands.
 */
(function initConchBackendRouter(global) {
  'use strict';

  function create(deps) {
    const invoke = deps.invoke;

    let mode = 'local';

    function setMode(m) {
      mode = m;
      log.info('[backend-router] mode set to:', mode);
    }

    function getMode() {
      return mode;
    }

    function isTmux() {
      return mode === 'tmux';
    }

    // --- Tab/Window actions ---

    function newTab(opts) {
      if (isTmux()) {
        return invoke('tmux_new_window');
      }
      // Local mode: caller handles via existing spawn_shell flow
      return null;
    }

    function closeTab(tmuxWindowId) {
      if (isTmux()) {
        return invoke('tmux_close_window', { windowId: tmuxWindowId });
      }
      return null;
    }

    function renameTab(tmuxWindowId, name) {
      if (isTmux()) {
        return invoke('tmux_rename_window', { windowId: tmuxWindowId, name });
      }
      return null;
    }

    // --- Pane actions ---

    function writeToPane(paneId, data) {
      if (isTmux()) {
        return invoke('tmux_write_to_pane', { paneId, data });
      }
      return invoke('write_to_pty', { paneId, data });
    }

    function resizePane(paneId, cols, rows) {
      if (isTmux()) {
        return invoke('tmux_resize_pane', { paneId, cols, rows });
      }
      return invoke('resize_pty', { paneId, cols, rows });
    }

    function splitVertical(paneId) {
      if (isTmux()) {
        return invoke('tmux_split_pane', { paneId, horizontal: false });
      }
      return null;
    }

    function splitHorizontal(paneId) {
      if (isTmux()) {
        return invoke('tmux_split_pane', { paneId, horizontal: true });
      }
      return null;
    }

    function closePane(paneId) {
      if (isTmux()) {
        return invoke('tmux_close_pane', { paneId });
      }
      return invoke('close_pty', { paneId });
    }

    function selectPane(paneId) {
      if (isTmux()) {
        return invoke('tmux_select_pane', { paneId });
      }
      return null;
    }

    // --- Session actions ---

    function connect(sessionName) {
      return invoke('tmux_connect', { sessionName });
    }

    function disconnect() {
      return invoke('tmux_disconnect');
    }

    return {
      setMode,
      getMode,
      isTmux,
      newTab,
      closeTab,
      renameTab,
      writeToPane,
      resizePane,
      splitVertical,
      splitHorizontal,
      closePane,
      selectPane,
      connect,
      disconnect,
    };
  }

  global.conchBackendRouter = { create };
})(typeof window !== 'undefined' ? window : globalThis);
```

- [ ] **Step 2: Add script tag to index.html**

In `crates/conch_tauri/frontend/index.html`, add a `<script>` tag for the new module, placed before `tab-manager.js` and `pane-manager.js` (since those will eventually call backendRouter):

```html
<script src="app/backend-router.js"></script>
```

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tauri/frontend/app/backend-router.js crates/conch_tauri/frontend/index.html
git commit -m "Add frontend backend router for local/tmux action dispatch"
```

---

## Task 11: Frontend — Tmux ID Map

**Files:**
- Create: `crates/conch_tauri/frontend/app/tmux-id-map.js`

- [ ] **Step 1: Write ID map module**

Create `crates/conch_tauri/frontend/app/tmux-id-map.js`:

```javascript
/**
 * Tmux ID Map — bidirectional mapping between tmux IDs and frontend IDs.
 *
 * In tmux mode, tab and pane creation is driven by tmux notifications.
 * This module tracks which tmux window/pane IDs correspond to which
 * frontend tab/pane IDs.
 */
(function initConchTmuxIdMap(global) {
  'use strict';

  function create() {
    // tmux window_id (number) ↔ frontend tabId (string)
    const windowToTab = new Map();
    const tabToWindow = new Map();

    // tmux pane_id (number) ↔ frontend paneId (number)
    const tmuxToPane = new Map();
    const paneToTmux = new Map();

    function addWindow(tmuxWindowId, frontendTabId) {
      windowToTab.set(tmuxWindowId, frontendTabId);
      tabToWindow.set(frontendTabId, tmuxWindowId);
    }

    function removeWindow(tmuxWindowId) {
      const tabId = windowToTab.get(tmuxWindowId);
      if (tabId !== undefined) {
        tabToWindow.delete(tabId);
      }
      windowToTab.delete(tmuxWindowId);
    }

    function removeWindowByTab(frontendTabId) {
      const windowId = tabToWindow.get(frontendTabId);
      if (windowId !== undefined) {
        windowToTab.delete(windowId);
      }
      tabToWindow.delete(frontendTabId);
    }

    function getTabForWindow(tmuxWindowId) {
      return windowToTab.get(tmuxWindowId);
    }

    function getWindowForTab(frontendTabId) {
      return tabToWindow.get(frontendTabId);
    }

    function addPane(tmuxPaneId, frontendPaneId) {
      tmuxToPane.set(tmuxPaneId, frontendPaneId);
      paneToTmux.set(frontendPaneId, tmuxPaneId);
    }

    function removePane(tmuxPaneId) {
      const paneId = tmuxToPane.get(tmuxPaneId);
      if (paneId !== undefined) {
        paneToTmux.delete(paneId);
      }
      tmuxToPane.delete(tmuxPaneId);
    }

    function removePaneByFrontend(frontendPaneId) {
      const tmuxId = paneToTmux.get(frontendPaneId);
      if (tmuxId !== undefined) {
        tmuxToPane.delete(tmuxId);
      }
      paneToTmux.delete(frontendPaneId);
    }

    function getPaneForTmux(tmuxPaneId) {
      return tmuxToPane.get(tmuxPaneId);
    }

    function getTmuxForPane(frontendPaneId) {
      return paneToTmux.get(frontendPaneId);
    }

    function clear() {
      windowToTab.clear();
      tabToWindow.clear();
      tmuxToPane.clear();
      paneToTmux.clear();
    }

    return {
      addWindow,
      removeWindow,
      removeWindowByTab,
      getTabForWindow,
      getWindowForTab,
      addPane,
      removePane,
      removePaneByFrontend,
      getPaneForTmux,
      getTmuxForPane,
      clear,
    };
  }

  global.conchTmuxIdMap = { create };
})(typeof window !== 'undefined' ? window : globalThis);
```

- [ ] **Step 2: Add script tag to index.html**

```html
<script src="app/tmux-id-map.js"></script>
```

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tauri/frontend/app/tmux-id-map.js crates/conch_tauri/frontend/index.html
git commit -m "Add frontend tmux ID map for window/pane identity mapping"
```

---

## Task 12: Frontend — Tmux Sessions Tool Window

**Files:**
- Create: `crates/conch_tauri/frontend/app/panels/tmux-panel.js`

- [ ] **Step 1: Write tmux panel module**

Create `crates/conch_tauri/frontend/app/panels/tmux-panel.js`:

```javascript
/**
 * Tmux Sessions tool window.
 *
 * Displays tmux sessions with attach/create/rename/kill actions.
 * Live-updated via tmux-sessions-changed events from the backend.
 */
(function (exports) {
  'use strict';

  let invoke = null;
  let listen = null;
  let panelEl = null;
  let sessions = [];
  let selectedSessionName = null;
  let unlistenSessionsChanged = null;

  function init(opts) {
    invoke = opts.invoke;
    listen = opts.listen;
    panelEl = opts.panelEl;

    render();
    bindEvents();
    refreshSessions();
  }

  function render() {
    if (!panelEl) return;
    panelEl.innerHTML = '';

    // Toolbar
    const toolbar = document.createElement('div');
    toolbar.className = 'tmux-panel-toolbar';
    toolbar.innerHTML = [
      '<button class="tmux-btn" data-action="new" title="New Session">+ New</button>',
      '<button class="tmux-btn" data-action="attach" title="Attach">Attach</button>',
      '<button class="tmux-btn" data-action="refresh" title="Refresh">\u21BB</button>',
    ].join('');
    panelEl.appendChild(toolbar);

    // Session list
    const listEl = document.createElement('div');
    listEl.className = 'tmux-session-list';
    listEl.id = 'tmux-session-list';
    panelEl.appendChild(listEl);

    renderSessionList();
  }

  function renderSessionList() {
    const listEl = document.getElementById('tmux-session-list');
    if (!listEl) return;

    if (sessions.length === 0) {
      listEl.innerHTML = [
        '<div class="tmux-empty-state">',
        '  <p>No tmux sessions found.</p>',
        '  <p>Create one to get started.</p>',
        '  <button class="tmux-btn tmux-create-btn" data-action="new">Create Session</button>',
        '</div>',
      ].join('');
      return;
    }

    listEl.innerHTML = sessions
      .map((s) => {
        const indicator = s.attached ? '\u25CF' : '\u25CB';
        const selected = s.name === selectedSessionName ? ' tmux-session-selected' : '';
        const attached = s.attached ? ' tmux-session-attached' : '';
        const winLabel = s.window_count === 1 ? '1 win' : s.window_count + ' wins';
        return [
          '<div class="tmux-session-row' + selected + attached + '" data-session="' + window.utils.attr(s.name) + '">',
          '  <span class="tmux-session-indicator">' + indicator + '</span>',
          '  <span class="tmux-session-name">' + window.utils.esc(s.name) + '</span>',
          '  <span class="tmux-session-wins">' + winLabel + '</span>',
          '</div>',
        ].join('');
      })
      .join('');
  }

  function bindEvents() {
    // Toolbar clicks
    panelEl.addEventListener('click', (e) => {
      const btn = e.target.closest('[data-action]');
      if (!btn) return;
      const action = btn.dataset.action;
      if (action === 'new') createSession();
      else if (action === 'attach') attachSelected();
      else if (action === 'refresh') refreshSessions();
    });

    // Session row clicks
    panelEl.addEventListener('click', (e) => {
      const row = e.target.closest('.tmux-session-row');
      if (!row) return;
      selectedSessionName = row.dataset.session;
      renderSessionList();
    });

    // Double click to attach
    panelEl.addEventListener('dblclick', (e) => {
      const row = e.target.closest('.tmux-session-row');
      if (!row) return;
      const name = row.dataset.session;
      attachSession(name);
    });

    // Context menu
    panelEl.addEventListener('contextmenu', (e) => {
      const row = e.target.closest('.tmux-session-row');
      if (!row) return;
      e.preventDefault();
      selectedSessionName = row.dataset.session;
      renderSessionList();
      showContextMenu(e.clientX, e.clientY, row.dataset.session);
    });

    // Live updates from backend
    listen('tmux-sessions-changed', (event) => {
      const payload = event.payload || {};
      if (payload.sessions) {
        sessions = payload.sessions;
        renderSessionList();
      }
    });
  }

  async function refreshSessions() {
    try {
      const result = await invoke('tmux_list_sessions');
      if (Array.isArray(result)) {
        sessions = result;
        renderSessionList();
      }
    } catch (err) {
      console.error('[tmux-panel] refresh error:', err);
    }
  }

  async function createSession() {
    const name = prompt('Session name (leave empty for default):');
    if (name === null) return; // cancelled
    try {
      await invoke('tmux_create_session', { name: name || null });
      refreshSessions();
    } catch (err) {
      window.toast && window.toast.error('Failed to create session: ' + err);
    }
  }

  function attachSelected() {
    if (selectedSessionName) {
      attachSession(selectedSessionName);
    }
  }

  async function attachSession(name) {
    try {
      await invoke('tmux_connect', { sessionName: name });
    } catch (err) {
      window.toast && window.toast.error('Failed to attach: ' + err);
    }
  }

  async function renameSession(name) {
    const newName = prompt('New session name:', name);
    if (!newName || newName === name) return;
    try {
      await invoke('tmux_rename_session', { oldName: name, newName });
      refreshSessions();
    } catch (err) {
      window.toast && window.toast.error('Failed to rename: ' + err);
    }
  }

  async function killSession(name) {
    if (!confirm('Kill session "' + name + '"? This will close all its windows.')) return;
    try {
      await invoke('tmux_kill_session', { name });
      if (selectedSessionName === name) selectedSessionName = null;
      refreshSessions();
    } catch (err) {
      window.toast && window.toast.error('Failed to kill session: ' + err);
    }
  }

  function showContextMenu(x, y, sessionName) {
    // Remove any existing context menu
    const existing = document.getElementById('tmux-context-menu');
    if (existing) existing.remove();

    const menu = document.createElement('div');
    menu.id = 'tmux-context-menu';
    menu.className = 'tmux-context-menu';
    menu.style.left = x + 'px';
    menu.style.top = y + 'px';
    menu.innerHTML = [
      '<div class="tmux-ctx-item" data-ctx="attach">Attach</div>',
      '<div class="tmux-ctx-item" data-ctx="new-window">Open In New Window</div>',
      '<div class="tmux-ctx-item" data-ctx="rename">Rename</div>',
      '<div class="tmux-ctx-item tmux-ctx-danger" data-ctx="kill">Kill</div>',
    ].join('');

    menu.addEventListener('click', (e) => {
      const item = e.target.closest('[data-ctx]');
      if (!item) return;
      menu.remove();
      const action = item.dataset.ctx;
      if (action === 'attach') attachSession(sessionName);
      else if (action === 'rename') renameSession(sessionName);
      else if (action === 'kill') killSession(sessionName);
      // 'new-window' is a future feature
    });

    document.body.appendChild(menu);

    // Close on click elsewhere
    const closeMenu = () => {
      menu.remove();
      document.removeEventListener('click', closeMenu);
    };
    setTimeout(() => document.addEventListener('click', closeMenu), 0);
  }

  exports.tmuxPanel = {
    init,
    refreshSessions,
    createSession,
    renameCurrentSession: () => {
      if (selectedSessionName) renameSession(selectedSessionName);
    },
    killSessionPrompt: () => {
      if (selectedSessionName) killSession(selectedSessionName);
    },
  };
})(window);
```

- [ ] **Step 2: Add script tag to index.html**

```html
<script src="app/panels/tmux-panel.js"></script>
```

- [ ] **Step 3: Commit**

```bash
git add crates/conch_tauri/frontend/app/panels/tmux-panel.js crates/conch_tauri/frontend/index.html
git commit -m "Add Tmux Sessions tool window panel"
```

---

## Task 13: Frontend — Tool Window Registration and Event Wiring

**Files:**
- Modify: `crates/conch_tauri/frontend/app/tool-window-runtime.js`
- Modify: `crates/conch_tauri/frontend/app/event-wiring-runtime.js`
- Modify: `crates/conch_tauri/frontend/app/command-palette-runtime.js`

- [ ] **Step 1: Register tmux-sessions tool window conditionally**

In `tool-window-runtime.js`, after the existing `ssh-sessions` registration block, add:

```javascript
// Register tmux sessions tool window when in tmux mode
if (global.backendRouter && global.backendRouter.isTmux()) {
  global.toolWindowManager.register('tmux-sessions', {
    title: 'Tmux Sessions',
    type: 'built-in',
    defaultZone: 'right-bottom',
    renderFn: (container) => {
      const panelEl = document.createElement('div');
      panelEl.id = 'tmux-sessions-panel';
      container.appendChild(panelEl);
      if (global.tmuxPanel) {
        global.tmuxPanel.init({
          invoke,
          listen: listenOnCurrentWindow,
          panelEl,
        });
      }
    },
  });
}
```

Note: The exact insertion point depends on when `backendRouter` mode is set during startup. The implementer should wire this registration to fire after the `init-backend` event is received, which may require deferring registration. The pattern:

```javascript
listenOnCurrentWindow('init-backend', (event) => {
  const backend = event.payload;
  if (global.backendRouter) global.backendRouter.setMode(backend);
  if (backend === 'tmux') {
    // register tmux-sessions tool window here
  }
});
```

- [ ] **Step 2: Add tmux event listeners to event-wiring-runtime.js**

In `event-wiring-runtime.js`, add a block that registers tmux-specific listeners when in tmux mode:

```javascript
// Tmux event wiring — registered after init-backend confirms tmux mode
function wireTmuxEvents() {
  listenOnCurrentWindow('tmux-output', (event) => {
    const { pane_id, data } = event.payload;
    if (!global.tmuxIdMap) return;
    const frontendPaneId = global.tmuxIdMap.getPaneForTmux(pane_id);
    if (frontendPaneId != null) {
      const pane = getPanes().get(frontendPaneId);
      if (pane && pane.term) pane.term.write(data);
    }
  });

  listenOnCurrentWindow('tmux-window-add', (event) => {
    const { window_id, name } = event.payload;
    // Create a new tab for this tmux window
    if (typeof handleMenuAction === 'function') {
      // The tab creation will be handled by tab-manager's tmux-aware path
    }
  });

  listenOnCurrentWindow('tmux-window-close', (event) => {
    const { window_id } = event.payload;
    if (!global.tmuxIdMap) return;
    const tabId = global.tmuxIdMap.getTabForWindow(window_id);
    if (tabId != null) {
      // Close the tab without sending a command back to tmux
    }
  });

  listenOnCurrentWindow('tmux-window-renamed', (event) => {
    const { window_id, name } = event.payload;
    if (!global.tmuxIdMap) return;
    const tabId = global.tmuxIdMap.getTabForWindow(window_id);
    if (tabId != null && name) {
      // Update tab label
    }
  });

  listenOnCurrentWindow('tmux-disconnected', (event) => {
    const reason = event.payload && event.payload.reason;
    if (window.toast) {
      window.toast.warn(reason ? 'Tmux disconnected: ' + reason : 'Tmux session ended');
    }
    // Open session picker or show reconnect option
  });

  listenOnCurrentWindow('tmux-sessions-changed', (event) => {
    // tmux-panel handles this itself, but other UI can also react
  });
}
```

Note: The stub comments (`// Close the tab`, `// Update tab label`) are intentionally not implemented here — they require integration with tab-manager internals that the implementer should wire in Task 14. The event listeners are registered; the handlers will call through to tab-manager APIs.

- [ ] **Step 3: Add tmux commands to command palette**

In `command-palette-runtime.js`, add conditional tmux commands:

```javascript
// Add after existing command registrations, guarded by backend mode
if (global.backendRouter && global.backendRouter.isTmux()) {
  add('tmux:show-sessions', 'Tmux: Show Sessions', 'Tmux', 'tmux session list browse', () => {
    if (global.toolWindowManager) global.toolWindowManager.activate('tmux-sessions');
  });
  add('tmux:create-session', 'Tmux: Create Session', 'Tmux', 'tmux session new create', () => {
    if (global.tmuxPanel) global.tmuxPanel.createSession();
  });
  add('tmux:rename-session', 'Tmux: Rename Session', 'Tmux', 'tmux session rename', () => {
    if (global.tmuxPanel) global.tmuxPanel.renameCurrentSession();
  });
  add('tmux:kill-session', 'Tmux: Kill Session', 'Tmux', 'tmux session kill destroy', () => {
    if (global.tmuxPanel) global.tmuxPanel.killSessionPrompt();
  });
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/conch_tauri/frontend/app/tool-window-runtime.js crates/conch_tauri/frontend/app/event-wiring-runtime.js crates/conch_tauri/frontend/app/command-palette-runtime.js
git commit -m "Wire tmux tool window registration, events, and command palette"
```

---

## Task 14: Frontend — Tab/Pane Manager Integration

**Files:**
- Modify: `crates/conch_tauri/frontend/app/tab-manager.js`
- Modify: `crates/conch_tauri/frontend/app/pane-manager.js`

This task modifies the existing tab and pane managers to route through `backendRouter` when in tmux mode. The changes are surgical — existing local-mode code paths stay untouched, and tmux mode adds a branch.

- [ ] **Step 1: Modify tab-manager.js**

The tab manager needs to:
1. Accept `backendRouter` as a dependency
2. On "new tab" action: check `backendRouter.isTmux()` — if true, call `backendRouter.newTab()` and return (tmux will notify when the window is created)
3. On "close tab": if tmux, call `backendRouter.closeTab(tmuxWindowId)` via the ID map
4. On "rename tab": if tmux, call `backendRouter.renameTab(tmuxWindowId, name)`

The implementer should find each action callsite and add the tmux branch. Example pattern for the "new tab" action handler:

```javascript
// In the new-tab handler:
if (backendRouter && backendRouter.isTmux()) {
  backendRouter.newTab();
  return; // Tab creation happens when tmux-window-add event arrives
}
// ... existing local-mode tab creation code
```

- [ ] **Step 2: Modify pane-manager.js**

The pane manager needs to:
1. Accept `backendRouter` as a dependency
2. Route `writeToPane` through `backendRouter.writeToPane(tmuxPaneId, data)` in tmux mode
3. Route resize through `backendRouter.resizePane(tmuxPaneId, cols, rows)` in tmux mode
4. Route splits through `backendRouter.splitVertical/splitHorizontal(tmuxPaneId)` in tmux mode
5. Route close through `backendRouter.closePane(tmuxPaneId)` in tmux mode

Each routing uses `tmuxIdMap.getTmuxForPane(frontendPaneId)` to get the tmux pane ID.

Example for the xterm.js onData handler:

```javascript
// In the terminal onData callback for a pane:
if (backendRouter && backendRouter.isTmux()) {
  const tmuxPaneId = tmuxIdMap.getTmuxForPane(paneId);
  if (tmuxPaneId != null) {
    backendRouter.writeToPane(tmuxPaneId, data);
  }
  return;
}
// ... existing write_to_pty call
```

- [ ] **Step 3: Verify no regressions in local mode**

Run the app in local mode (default config, no `backend = "tmux"`). All existing tab/pane behavior should work exactly as before. The tmux branches are never entered when `backendRouter.isTmux()` returns false.

- [ ] **Step 4: Commit**

```bash
git add crates/conch_tauri/frontend/app/tab-manager.js crates/conch_tauri/frontend/app/pane-manager.js
git commit -m "Route tab/pane actions through backend router for tmux support"
```

---

## Task 15: Titlebar and Menu Updates

**Files:**
- Modify: `crates/conch_tauri/frontend/app/ui/titlebar.js`
- Modify: `crates/conch_tauri/src/menu.rs`

- [ ] **Step 1: Add tmux session badge to titlebar**

In `titlebar.js`, add a tmux badge element to the tab strip area. The badge is hidden by default and shown when in tmux mode:

```javascript
// After tab strip creation, add tmux badge
const tmuxBadge = document.createElement('span');
tmuxBadge.id = 'tmux-session-badge';
tmuxBadge.className = 'tmux-session-badge hidden';
tmuxBadge.title = 'Click to show Tmux Sessions';
tmuxBadge.addEventListener('click', () => {
  if (window.toolWindowManager) {
    window.toolWindowManager.activate('tmux-sessions');
  }
});
// Insert before the tab container
tabStripEl.insertBefore(tmuxBadge, tabStripEl.firstChild);
```

Add a function to update the badge:

```javascript
function setTmuxSessionName(name) {
  const badge = document.getElementById('tmux-session-badge');
  if (!badge) return;
  if (name) {
    badge.textContent = 'tmux: ' + name;
    badge.classList.remove('hidden');
  } else {
    badge.classList.add('hidden');
  }
}
```

Wire it to the `tmux-connected` event listener.

- [ ] **Step 2: Add CSS for tmux badge**

In the appropriate CSS file (e.g., `styles/layout.css`), add:

```css
.tmux-session-badge {
  font-size: 11px;
  color: var(--text-secondary);
  padding: 2px 8px;
  white-space: nowrap;
  cursor: pointer;
  flex-shrink: 0;
}
.tmux-session-badge:hover {
  color: var(--fg);
}
```

- [ ] **Step 3: Add tmux panel CSS**

Add styles for the tmux sessions panel:

```css
.tmux-panel-toolbar {
  display: flex;
  gap: 4px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--border);
}
.tmux-btn {
  background: var(--button-bg, var(--bg));
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 3px;
  padding: 2px 8px;
  cursor: pointer;
  font-size: 12px;
}
.tmux-btn:hover {
  background: var(--selection-bg);
}
.tmux-session-list {
  overflow-y: auto;
  flex: 1;
}
.tmux-session-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  cursor: pointer;
  font-size: 13px;
}
.tmux-session-row:hover {
  background: var(--selection-bg);
}
.tmux-session-selected {
  background: var(--selection-bg);
}
.tmux-session-attached .tmux-session-name {
  font-weight: bold;
}
.tmux-session-indicator {
  font-size: 10px;
  flex-shrink: 0;
}
.tmux-session-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tmux-session-wins {
  color: var(--text-secondary);
  font-size: 11px;
  flex-shrink: 0;
}
.tmux-empty-state {
  padding: 16px;
  text-align: center;
  color: var(--text-secondary);
}
.tmux-empty-state p {
  margin: 4px 0;
}
.tmux-create-btn {
  margin-top: 8px;
}
.tmux-context-menu {
  position: fixed;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 4px 0;
  z-index: 9999;
  min-width: 160px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.3);
}
.tmux-ctx-item {
  padding: 4px 12px;
  cursor: pointer;
  font-size: 13px;
}
.tmux-ctx-item:hover {
  background: var(--selection-bg);
}
.tmux-ctx-danger {
  color: var(--red, #e06c75);
}
```

- [ ] **Step 4: Add menu constant in menu.rs**

In `crates/conch_tauri/src/menu.rs`, add constants:

```rust
pub(crate) const MENU_FOCUS_TMUX_SESSIONS_ID: &str = "view.focus_tmux_sessions";
pub(crate) const MENU_ACTION_FOCUS_TMUX_SESSIONS: &str = "focus-tmux-sessions";
```

In the `build_app_menu` function, conditionally add the menu item to the View menu. Since the menu is built once at startup, the item is always present but only functional when tmux mode is active. The frontend ignores the action if not in tmux mode.

- [ ] **Step 5: Handle menu event in lib.rs**

In the `.on_menu_event()` handler in `lib.rs`, add:

```rust
menu::MENU_FOCUS_TMUX_SESSIONS_ID => {
    menu::emit_menu_action_to_focused_window(app, menu::MENU_ACTION_FOCUS_TMUX_SESSIONS)
}
```

In the frontend menu action handler (`event-wiring-runtime.js` `handleMenuAction`):

```javascript
case 'focus-tmux-sessions':
  if (global.toolWindowManager) global.toolWindowManager.activate('tmux-sessions');
  break;
```

- [ ] **Step 6: Commit**

```bash
git add crates/conch_tauri/frontend/app/ui/titlebar.js crates/conch_tauri/frontend/styles/ crates/conch_tauri/src/menu.rs crates/conch_tauri/src/lib.rs crates/conch_tauri/frontend/app/event-wiring-runtime.js
git commit -m "Add tmux session badge, panel CSS, and menu wiring"
```

---

## Task 16: Startup Flow

**Files:**
- Modify: `crates/conch_tauri/src/lib.rs`
- Modify: `crates/conch_tauri/frontend/app/startup-runtime.js` (or equivalent bootstrap file)

- [ ] **Step 1: Add tmux version validation helper**

In `crates/conch_tauri/src/tmux/mod.rs`, add a helper to validate tmux is installed and meets the minimum version:

```rust
/// Check that tmux is installed and >= 1.8 (control mode support).
/// Returns the version string on success or an error message.
pub(crate) fn validate_tmux_binary(binary: &str) -> Result<String, String> {
    let output = std::process::Command::new(binary)
        .arg("-V")
        .output()
        .map_err(|e| format!("tmux not found at '{}': {}", binary, e))?;
    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Parse "tmux X.Y" or "tmux X.Ya" format
    let version_part = version_str
        .strip_prefix("tmux ")
        .unwrap_or(&version_str);
    let major_minor: f64 = version_part
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .parse()
        .unwrap_or(0.0);
    if major_minor < 1.8 {
        return Err(format!(
            "tmux {} is too old — control mode requires tmux >= 1.8",
            version_str,
        ));
    }
    Ok(version_str)
}
```

- [ ] **Step 2: Emit init-backend event from Rust with validation**

In `lib.rs`, during window setup (inside the `.setup()` callback or window creation logic), after the window is created:

```rust
// Determine backend mode, validating tmux if configured
let backend_str = match config.terminal.backend {
    conch_core::config::TerminalBackend::Tmux => {
        match tmux::validate_tmux_binary(&config.terminal.tmux.resolved_binary()) {
            Ok(version) => {
                log::info!("tmux backend enabled: {version}");
                "tmux"
            }
            Err(e) => {
                log::error!("Falling back to local backend: {e}");
                // Frontend will show a toast about the fallback
                let _ = window.emit("tmux-validation-error", &e);
                "local"
            }
        }
    }
    conch_core::config::TerminalBackend::Local => "local",
};
let _ = window.emit("init-backend", backend_str);
```

- [ ] **Step 2: Handle init-backend in frontend startup**

In the frontend startup code, listen for `init-backend` and initialize accordingly:

```javascript
listenOnCurrentWindow('init-backend', async (event) => {
  const backend = event.payload;
  if (backendRouter) backendRouter.setMode(backend);

  if (backend === 'tmux') {
    // Register tmux tool window
    // Apply startup behavior
    const config = await invoke('get_terminal_config');
    const startup = config && config.tmux && config.tmux.startup_behavior;

    if (startup === 'show_session_picker') {
      // Open tmux sessions panel, don't auto-connect
      if (window.toolWindowManager) window.toolWindowManager.activate('tmux-sessions');
    } else if (startup === 'create_new_session') {
      // Create and connect
      await invoke('tmux_connect', { sessionName: '' });
    } else {
      // Default: attach_last_session
      // Attempt to connect to last session, fall back to picker
      try {
        const lastSession = await invoke('get_last_tmux_session');
        if (lastSession) {
          await invoke('tmux_connect', { sessionName: lastSession });
        } else {
          if (window.toolWindowManager) window.toolWindowManager.activate('tmux-sessions');
        }
      } catch {
        if (window.toolWindowManager) window.toolWindowManager.activate('tmux-sessions');
      }
    }
  }
});
```

Note: This requires two new small Tauri commands: `get_terminal_config` (return tmux config to frontend) and `get_last_tmux_session` (read from persistent state). These are thin wrappers:

```rust
#[tauri::command]
pub(crate) fn get_terminal_backend(
    state: tauri::State<'_, TauriState>,
) -> String {
    let config = state.config.read().unwrap();
    match config.terminal.backend {
        conch_core::config::TerminalBackend::Local => "local".into(),
        conch_core::config::TerminalBackend::Tmux => "tmux".into(),
    }
}

#[tauri::command]
pub(crate) fn get_last_tmux_session() -> Option<String> {
    conch_core::config::load_persistent_state()
        .ok()
        .and_then(|s| s.last_tmux_session)
}
```

- [ ] **Step 3: Save last session on connect**

In `tmux_connect` command (tmux/mod.rs), after successful connection, persist the session name:

```rust
// Save last session for attach_last_session startup behavior
if let Ok(mut state) = conch_core::config::load_persistent_state() {
    state.last_tmux_session = Some(session_name.clone());
    let _ = conch_core::config::save_persistent_state(&state);
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: full workspace compiles

- [ ] **Step 5: Commit**

```bash
git add crates/conch_tauri/src/lib.rs crates/conch_tauri/src/tmux/mod.rs crates/conch_tauri/frontend/app/
git commit -m "Wire tmux startup flow with backend detection and session persistence"
```

---

## Task 17: Update config.example.toml

**Files:**
- Modify: `crates/conch_tauri/../../config.example.toml` (root-level example config)

- [ ] **Step 1: Add tmux configuration examples**

Add a new section to `config.example.toml`:

```toml
# Terminal backend: "local" (default) or "tmux"
# In tmux mode, Conch acts as a tmux client — tabs map to tmux windows,
# panes map to tmux panes.
# backend = "tmux"

# Tmux-specific settings (only used when backend = "tmux")
# [terminal.tmux]
# Path to tmux binary. Empty = search $PATH.
# binary = ""
#
# What to do when a window opens in tmux mode:
#   "attach_last_session" — resume the last session (default)
#   "show_session_picker" — show the session list, let the user choose
#   "create_new_session"  — always create a fresh session
# startup_behavior = "attach_last_session"
#
# What "New Tab" does in tmux mode:
#   "new_tmux_window" — create a tmux window in the current session (default)
#   "session_picker"  — show the session picker
# new_tab_behavior = "new_tmux_window"
#
# What "New Window" does in tmux mode:
#   "attach_same_session"  — attach to the same session (default)
#   "show_session_picker"  — show the session picker
# new_window_behavior = "attach_same_session"
```

- [ ] **Step 2: Commit**

```bash
git add config.example.toml
git commit -m "Document tmux configuration options in example config"
```

---

## Task 18: Final Integration Test

This is a manual verification task — no automated test, since it requires a running tmux server and Tauri app.

- [ ] **Step 1: Verify local mode is unaffected**

1. Ensure `config.toml` has no `backend` setting (or `backend = "local"`)
2. Run the app: `cargo tauri dev`
3. Verify: tabs, panes, splits, close, rename all work as before
4. Verify: no tmux-related UI appears (no badge, no Tmux Sessions panel)

- [ ] **Step 2: Verify tmux mode startup**

1. Set `backend = "tmux"` in `config.toml`
2. Ensure tmux is installed (`tmux -V`)
3. Run the app: `cargo tauri dev`
4. Verify: `init-backend` event fires with `"tmux"`
5. Verify: Tmux Sessions tool window appears
6. Verify: session badge appears in tab strip when attached

- [ ] **Step 3: Verify session CRUD**

1. Create a session via the tool window
2. Rename the session
3. Kill the session
4. Verify the list updates in real time

- [ ] **Step 4: Verify tab/pane mapping**

1. Attach to a tmux session with multiple windows
2. Verify each tmux window appears as a Conch tab
3. Create a new tmux window — verify new tab appears
4. Close a tmux window — verify tab disappears
5. Type in a pane — verify output renders in xterm.js

- [ ] **Step 5: Run full test suite**

Run: `cargo test --workspace`
Expected: all tests pass (existing + new conch_tmux + new conch_core tests)

- [ ] **Step 6: Final commit if any fixups needed**

```bash
git add -A
git commit -m "Fix integration issues found during manual testing"
```

---

## Summary

| Task | Component | New Tests |
|------|-----------|-----------|
| 1 | `conch_tmux` crate scaffolding | — |
| 2 | Protocol types | 3 |
| 3 | Control mode parser | 20+ |
| 4 | Command builder | 18+ |
| 5 | Session model | 12+ |
| 6 | Connection (split ownership) | — |
| 7 | Config types in `conch_core` | 10+ |
| 8 | Tauri integration events | — |
| 9 | Tauri commands | — |
| 10 | Frontend backend router | — |
| 11 | Frontend tmux ID map | — |
| 12 | Frontend tmux sessions panel | — |
| 13 | Tool window + event wiring | — |
| 14 | Tab/pane manager integration | — |
| 15 | Titlebar + menu + CSS | — |
| 16 | Startup flow | — |
| 17 | Example config docs | — |
| 18 | Manual integration test | — |

**Total estimated new tests:** 63+
