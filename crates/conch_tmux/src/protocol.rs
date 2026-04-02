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
