//! Application state: sessions, tabs, and UI state.

use std::collections::HashMap;
use std::sync::Arc;

use alacritty_terminal::event::Event as TermEvent;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use conch_core::color_scheme;
use conch_core::config::{PersistentState, UserConfig};
use conch_pty::EventProxy;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::terminal::color::ResolvedColors;
use crate::ui_theme::UiTheme;

/// A single terminal session backed by a local PTY.
pub struct Session {
    pub id: Uuid,
    pub title: String,
    /// User-set custom title (overrides `title` for display when `Some`).
    pub custom_title: Option<String>,
    pub pty: conch_pty::LocalSession,
    pub event_rx: mpsc::UnboundedReceiver<TermEvent>,
}

impl Session {
    /// Get the terminal state.
    pub fn term(&self) -> &Arc<FairMutex<Term<EventProxy>>> {
        &self.pty.term
    }

    /// Send raw bytes to the PTY.
    pub fn write(&self, data: &[u8]) {
        self.pty.write(data);
    }

    /// Resize the PTY.
    pub fn resize(&self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
        self.pty.resize(cols, rows, cell_width, cell_height);
    }

    /// Display title (custom overrides auto-detected).
    pub fn display_title(&self) -> &str {
        self.custom_title.as_deref().unwrap_or(&self.title)
    }
}

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
