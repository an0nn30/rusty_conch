//! Platform-specific environment initialisation.
//!
//! When launched from a desktop environment (macOS Finder, Linux desktop entry,
//! Windows Start Menu) the process inherits a minimal environment that may lack
//! variables like `LANG`, `SSH_AUTH_SOCK`, or a complete `PATH`.
//!
//! Each platform module detects and repairs these gaps so the rest of the app
//! can assume a sane environment.

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

/// Perform platform-specific environment setup.
///
/// Must be called early in `main()`, before any child processes are spawned or
/// environment variables are read.
pub fn init() {
    #[cfg(target_os = "macos")]
    macos::init();

    #[cfg(target_os = "linux")]
    linux::init();

    #[cfg(target_os = "windows")]
    windows::init();
}
