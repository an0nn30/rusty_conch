//! Raw PTY backend using `portable-pty`.
//!
//! Unlike `conch_pty` (which wraps alacritty_terminal for grid-level access),
//! this module provides raw byte-level PTY I/O — xterm.js handles all terminal
//! emulation on the frontend side.

use std::io::Write;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

pub(crate) struct PtyBackend {
    master: Box<dyn MasterPty + Send>,
    writer: Mutex<Box<dyn Write + Send>>,
}

impl PtyBackend {
    /// Spawn a new PTY with the given dimensions and optional shell override.
    pub fn new(cols: u16, rows: u16, shell: Option<&str>) -> Result<Self> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .context("Failed to open PTY pair")?;

        let actual_shell = match shell {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                #[cfg(unix)]
                {
                    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
                }
                #[cfg(not(unix))]
                {
                    "cmd.exe".to_string()
                }
            }
        };

        let mut cmd = CommandBuilder::new(&actual_shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        pair.slave
            .spawn_command(cmd)
            .context("Failed to spawn shell in PTY")?;

        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY writer")?;

        Ok(Self {
            master: pair.master,
            writer: Mutex::new(writer),
        })
    }

    /// Write raw bytes to the PTY (user keyboard input).
    pub fn write(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock();
        writer.write_all(data).context("PTY write failed")?;
        writer.flush().context("PTY flush failed")?;
        Ok(())
    }

    /// Resize the PTY.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("PTY resize failed")
    }

    /// Clone the reader for use in a separate thread.
    pub fn try_clone_reader(&self) -> Option<Box<dyn std::io::Read + Send>> {
        self.master.try_clone_reader().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shell_env_var_used() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        assert!(!shell.is_empty());
    }

    #[test]
    fn pty_size_struct_fields() {
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }
}
