use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_terminal::event::WindowSize;
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{self, Term};
use alacritty_terminal::tty;
use anyhow::{Context, Result};
use tokio::sync::mpsc;

use crate::connector::EventProxy;

/// Get the current working directory of a process by PID.
///
/// On macOS, uses `proc_pidinfo` with `PROC_PIDVNODEPATHINFO`.
/// On Linux, reads `/proc/{pid}/cwd` symlink.
/// Returns `None` on failure or unsupported platforms.
#[cfg(target_os = "macos")]
pub fn get_cwd_of_pid(pid: u32) -> Option<PathBuf> {
    use std::mem::MaybeUninit;

    // PROC_PIDVNODEPATHINFO = 9
    const PROC_PIDVNODEPATHINFO: i32 = 9;

    #[repr(C)]
    struct VnodeInfoPath {
        _vip_vi: [u8; 152], // vnode_info (we don't need its fields)
        vip_path: [u8; 1024], // MAXPATHLEN
    }

    #[repr(C)]
    struct ProcVnodePathInfo {
        pvi_cdir: VnodeInfoPath,
        pvi_rdir: VnodeInfoPath,
    }

    unsafe {
        let mut info = MaybeUninit::<ProcVnodePathInfo>::uninit();
        let size = std::mem::size_of::<ProcVnodePathInfo>() as i32;
        let ret = libc::proc_pidinfo(
            pid as i32,
            PROC_PIDVNODEPATHINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        );
        if ret <= 0 {
            return None;
        }
        let info = info.assume_init();
        let path_bytes = &info.pvi_cdir.vip_path;
        let nul_pos = path_bytes.iter().position(|&b| b == 0).unwrap_or(path_bytes.len());
        let path_str = std::str::from_utf8(&path_bytes[..nul_pos]).ok()?;
        if path_str.is_empty() {
            return None;
        }
        Some(PathBuf::from(path_str))
    }
}

#[cfg(target_os = "linux")]
pub fn get_cwd_of_pid(pid: u32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn get_cwd_of_pid(_pid: u32) -> Option<PathBuf> {
    None
}

/// Simple Dimensions impl for creating a Term.
struct TermSize {
    columns: usize,
    lines: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

/// A local terminal session backed by a PTY.
pub struct LocalSession {
    /// The terminal state (shared with the event loop thread).
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    /// Channel to send input/resize/shutdown to the event loop.
    loop_tx: EventLoopSender,
    /// Async receiver for terminal events (Wakeup, Title, Exit, etc.).
    event_rx: Option<mpsc::UnboundedReceiver<alacritty_terminal::event::Event>>,
    /// Join handle for the event loop thread (Option so Drop can take it).
    join_handle: Option<JoinHandle<(EventLoop<tty::Pty, EventProxy>, alacritty_terminal::event_loop::State)>>,
    /// Child process PID (for CWD polling via macOS proc_pidinfo).
    child_pid: u32,
}

impl LocalSession {
    /// Spawn a new local PTY session.
    ///
    /// If `shell` is `Some`, the given program + args are used instead of `$SHELL`.
    /// `extra_env` is merged on top of the defaults (`TERM`, `COLORTERM`), so
    /// user-specified values override the built-in ones.
    /// `term_config` allows setting cursor style and other terminal options.
    pub fn new(
        cols: u16,
        rows: u16,
        cell_width: u16,
        cell_height: u16,
        shell: Option<tty::Shell>,
        extra_env: &HashMap<String, String>,
        term_config: term::Config,
        working_directory: Option<PathBuf>,
    ) -> Result<Self> {
        let window_size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width,
            cell_height,
        };

        let (event_proxy, event_rx) = EventProxy::new();

        // Terminal state
        let term_size = TermSize {
            columns: cols as usize,
            lines: rows as usize,
        };
        let term = Term::new(term_config, &term_size, event_proxy.clone());
        let term = Arc::new(FairMutex::new(term));

        // PTY options — defaults first, then user overrides.
        let mut env = HashMap::new();
        env.insert("TERM".into(), "xterm-256color".into());
        env.insert("COLORTERM".into(), "truecolor".into());
        for (k, v) in extra_env {
            env.insert(k.clone(), v.clone());
        }
        let cwd = working_directory
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")));
        log::debug!(
            "PTY spawn: shell={:?}, cwd={:?}, env_keys={:?}, size={}x{}",
            shell,
            cwd,
            env.keys().collect::<Vec<_>>(),
            cols,
            rows,
        );
        let options = tty::Options {
            shell,
            working_directory: Some(cwd),
            drain_on_exit: true,
            env,
            #[cfg(target_os = "windows")]
            escape_args: false,
        };

        // Create the PTY
        let pty = tty::new(&options, window_size, 0)
            .context("Failed to create PTY")?;

        // Extract child PID before EventLoop takes ownership of the Pty.
        #[cfg(not(windows))]
        let child_pid = pty.child().id();
        #[cfg(windows)]
        let child_pid = 0u32;

        // Create the event loop
        let event_loop = EventLoop::new(
            Arc::clone(&term),
            event_proxy,
            pty,
            true,  // drain_on_exit
            false, // ref_test
        )?;

        // Get sender BEFORE spawning
        let loop_tx = event_loop.channel();
        let join_handle = event_loop.spawn();

        Ok(Self {
            term,
            loop_tx,
            event_rx: Some(event_rx),
            join_handle: Some(join_handle),
            child_pid,
        })
    }

    /// Take the event receiver (can only be called once).
    pub fn take_event_rx(&mut self) -> mpsc::UnboundedReceiver<alacritty_terminal::event::Event> {
        self.event_rx.take().expect("event_rx already taken")
    }

    /// Get the child process PID (for CWD polling).
    pub fn child_pid(&self) -> u32 {
        self.child_pid
    }

    /// Send raw bytes to the PTY (keyboard input).
    pub fn write(&self, data: &[u8]) {
        let _ = self.loop_tx.send(Msg::Input(Cow::Owned(data.to_vec())));
    }

    /// Resize the terminal.
    pub fn resize(&self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
        if let Some(mut term) = self.term.try_lock_unfair() {
            term.resize(TermSize {
                columns: cols as usize,
                lines: rows as usize,
            });
        }

        let size = WindowSize {
            num_lines: rows,
            num_cols: cols,
            cell_width,
            cell_height,
        };
        let _ = self.loop_tx.send(Msg::Resize(size));
    }

    /// Shut down the event loop and PTY.
    pub fn shutdown(&self) {
        let _ = self.loop_tx.send(Msg::Shutdown);
    }
}

impl Drop for LocalSession {
    fn drop(&mut self) {
        self.shutdown();

        let child_pid = self.child_pid;
        if let Some(join_handle) = self.join_handle.take() {
            std::thread::Builder::new()
                .name("pty-cleanup".into())
                .spawn(move || {
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(child_pid as i32, libc::SIGKILL);
                    }
                    drop(join_handle);
                })
                .ok();
        }
    }
}
