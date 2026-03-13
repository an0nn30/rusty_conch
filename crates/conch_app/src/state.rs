//! Application state: sessions, tabs, and UI state.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;

use alacritty_terminal::event::Event as TermEvent;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use conch_core::color_scheme;
use conch_core::config::{PersistentState, UserConfig};
use conch_plugin_sdk::{SessionBackendVtable, SessionStatus};
use conch_pty::EventProxy;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::host::session_bridge::PluginSessionBridge;
use crate::terminal::color::ResolvedColors;
use crate::ui_theme::UiTheme;

/// An inline prompt displayed in the session's connecting screen.
pub struct SessionPrompt {
    /// 0 = confirm (Accept/Reject), 1 = password input.
    pub prompt_type: u8,
    /// Main message text.
    pub message: String,
    /// Secondary detail text (e.g., fingerprint).
    pub detail: String,
    /// Password input buffer (for prompt_type == 1).
    pub password_buf: String,
    /// Whether to auto-focus the password field on next frame.
    pub focus_password: bool,
    /// Whether to reveal the password text.
    pub show_password: bool,
    /// Channel to send the user's response back to the plugin thread.
    pub reply: Option<tokio::sync::oneshot::Sender<Option<String>>>,
}

/// The underlying backend for a terminal session.
pub enum SessionBackend {
    /// A local PTY process (shell).
    Local(conch_pty::LocalSession),
    /// A plugin-provided session (e.g. SSH).
    Plugin {
        bridge: PluginSessionBridge,
        vtable: SessionBackendVtable,
        backend_handle: *mut c_void,
    },
}

// SAFETY: The backend_handle raw pointer is only used through the vtable
// callbacks which are thread-safe by contract with the plugin.
unsafe impl Send for SessionBackend {}

/// A single terminal session.
pub struct Session {
    pub id: Uuid,
    pub title: String,
    /// User-set custom title (overrides `title` for display when `Some`).
    pub custom_title: Option<String>,
    pub backend: SessionBackend,
    pub event_rx: mpsc::UnboundedReceiver<TermEvent>,
    /// Connection status for plugin sessions. Local sessions are always Connected.
    pub status: SessionStatus,
    /// Detail/error message for Connecting/Error states.
    pub status_detail: Option<String>,
    /// When the session started connecting (for progress animation).
    pub connect_started: Option<std::time::Instant>,
    /// Inline prompt (fingerprint accept, password) shown in the connecting screen.
    pub prompt: Option<SessionPrompt>,
}

impl Session {
    /// Get the terminal state.
    pub fn term(&self) -> &Arc<FairMutex<Term<EventProxy>>> {
        match &self.backend {
            SessionBackend::Local(local) => &local.term,
            SessionBackend::Plugin { bridge, .. } => &bridge.term,
        }
    }

    /// Send raw bytes to the session.
    pub fn write(&self, data: &[u8]) {
        match &self.backend {
            SessionBackend::Local(local) => local.write(data),
            SessionBackend::Plugin { vtable, backend_handle, .. } => {
                (vtable.write)(*backend_handle, data.as_ptr(), data.len());
            }
        }
    }

    /// Resize the session.
    pub fn resize(&self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
        match &self.backend {
            SessionBackend::Local(local) => local.resize(cols, rows, cell_width, cell_height),
            SessionBackend::Plugin { bridge, vtable, backend_handle } => {
                // Resize the terminal emulator grid so rendered output matches.
                if let Some(mut term) = bridge.term.try_lock_unfair() {
                    term.resize(crate::host::session_bridge::TermSize::new(cols, rows));
                }
                // Resize the remote PTY (e.g. SSH channel window-change).
                (vtable.resize)(*backend_handle, cols, rows);
            }
        }
    }

    /// Shut down the session backend.
    pub fn shutdown(&self) {
        match &self.backend {
            SessionBackend::Local(local) => local.shutdown(),
            SessionBackend::Plugin { vtable, backend_handle, .. } => {
                (vtable.shutdown)(*backend_handle);
            }
        }
    }

    /// Get the child PID if this is a local session.
    pub fn child_pid(&self) -> Option<u32> {
        match &self.backend {
            SessionBackend::Local(local) => Some(local.child_pid()),
            SessionBackend::Plugin { .. } => None,
        }
    }

    /// Display title (custom overrides auto-detected).
    pub fn display_title(&self) -> String {
        let base = self.custom_title.as_deref().unwrap_or(&self.title);
        match self.status {
            SessionStatus::Connecting => format!("{base}\u{2026}"),
            SessionStatus::Error => format!("{base} (failed)"),
            SessionStatus::Connected => base.to_string(),
        }
    }
}

// NOTE: No Drop impl — the plugin owns the backend state (via its own
// HashMap<SessionHandle, Box<SshBackendState>>). The host only borrows
// the backend_handle pointer. Calling vtable.drop here would double-free.

/// The full application state.
pub struct AppState {
    pub user_config: UserConfig,
    pub persistent: PersistentState,
    pub colors: ResolvedColors,
    pub theme: UiTheme,
    pub theme_dirty: bool,
    pub sessions: HashMap<Uuid, Session>,
    pub active_tab: Option<Uuid>,
    pub tab_order: Vec<Uuid>,
}

impl AppState {
    pub fn new(user_config: UserConfig, persistent: PersistentState) -> Self {
        let scheme = color_scheme::resolve_theme(&user_config.colors.theme);
        let colors = ResolvedColors::from_scheme(&scheme);
        let theme = UiTheme::from_colors(&colors, user_config.colors.appearance_mode);

        Self {
            user_config,
            persistent,
            colors,
            theme,
            theme_dirty: true,
            sessions: HashMap::new(),
            active_tab: None,
            tab_order: Vec::new(),
        }
    }

    /// Get the currently active session, if any.
    pub fn active_session(&self) -> Option<&Session> {
        self.active_tab.and_then(|id| self.sessions.get(&id))
    }
}
