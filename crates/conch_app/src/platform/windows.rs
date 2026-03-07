//! Windows environment initialisation.
//!
//! Windows processes inherit the full system environment by default, and the
//! SSH agent uses a named pipe rather than `SSH_AUTH_SOCK`.  This module
//! provides a hook for any future fixups.

/// Entry point — called from `platform::init()`.
pub fn init() {
    // Nothing to patch up for now.
}
