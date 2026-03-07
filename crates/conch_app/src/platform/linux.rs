//! Linux environment initialisation.
//!
//! When launched from a `.desktop` entry the environment is usually complete,
//! but this module provides a hook for any future fixups.

/// Entry point — called from `platform::init()`.
pub fn init() {
    // Linux desktop sessions typically inherit a full environment from the
    // display manager / session manager.  Nothing to patch up for now.
}
