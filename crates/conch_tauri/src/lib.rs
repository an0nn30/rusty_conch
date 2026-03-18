//! Tauri-based UI for Conch (experimental).
//!
//! Uses xterm.js in a webview for terminal rendering, with a raw PTY backend
//! via `portable-pty`. This bypasses alacritty_terminal entirely — xterm.js
//! handles all terminal emulation.

mod pty_backend;

use std::sync::Arc;

use conch_core::config::UserConfig;
use parking_lot::Mutex;
use pty_backend::PtyBackend;
use tauri::Emitter;

struct TauriState {
    pty: Arc<Mutex<Option<PtyBackend>>>,
    config: UserConfig,
}

#[tauri::command]
fn spawn_shell(state: tauri::State<'_, TauriState>, cols: u16, rows: u16) -> Result<(), String> {
    let shell = state.config.terminal.shell.program.clone();
    let shell = if shell.is_empty() { None } else { Some(shell) };

    let backend = PtyBackend::new(cols, rows, shell.as_deref())
        .map_err(|e| format!("Failed to spawn PTY: {e}"))?;

    *state.pty.lock() = Some(backend);
    Ok(())
}

#[tauri::command]
fn write_to_pty(state: tauri::State<'_, TauriState>, data: String) -> Result<(), String> {
    let guard = state.pty.lock();
    let pty = guard.as_ref().ok_or("PTY not spawned")?;
    pty.write(data.as_bytes()).map_err(|e| format!("{e}"))
}

#[tauri::command]
fn resize_pty(state: tauri::State<'_, TauriState>, cols: u16, rows: u16) -> Result<(), String> {
    let guard = state.pty.lock();
    let pty = guard.as_ref().ok_or("PTY not spawned")?;
    pty.resize(cols, rows).map_err(|e| format!("{e}"))
}

/// Launch the Tauri-based UI.
pub fn run(config: UserConfig) -> anyhow::Result<()> {
    let pty_state = Arc::new(Mutex::new(None::<PtyBackend>));
    let pty_for_reader = Arc::clone(&pty_state);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(TauriState {
            pty: Arc::clone(&pty_state),
            config,
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let pty_ref = Arc::clone(&pty_for_reader);

            // Spawn a thread that reads PTY output and emits it to the frontend.
            std::thread::Builder::new()
                .name("pty-reader".into())
                .spawn(move || {
                    pty_reader_loop(&handle, &pty_ref);
                })
                .expect("Failed to spawn PTY reader thread");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            spawn_shell,
            write_to_pty,
            resize_pty,
        ])
        .run(tauri::generate_context!())
        .map_err(|e| anyhow::anyhow!("Tauri error: {e}"))?;

    Ok(())
}

/// Continuously reads PTY output and emits "pty-output" events to the frontend.
fn pty_reader_loop(
    handle: &tauri::AppHandle,
    pty_state: &Arc<Mutex<Option<PtyBackend>>>,
) {
    let mut buf = [0u8; 8192];

    loop {
        // Wait for a PTY to be available.
        let reader = {
            let guard = pty_state.lock();
            guard.as_ref().and_then(|p| p.try_clone_reader())
        };

        let Some(mut reader) = reader else {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        };

        // Read loop for this PTY session.
        loop {
            use std::io::Read;
            match reader.read(&mut buf) {
                Ok(0) => {
                    // EOF — shell exited.
                    let _ = handle.emit("pty-exit", ());
                    break;
                }
                Ok(n) => {
                    // Send raw bytes as a string (xterm.js expects UTF-8 or latin1).
                    // Use lossy conversion for binary data.
                    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let _ = handle.emit("pty-output", text);
                }
                Err(e) => {
                    log::error!("PTY read error: {e}");
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_state_default_has_no_pty() {
        let state = TauriState {
            pty: Arc::new(Mutex::new(None)),
            config: UserConfig::default(),
        };
        assert!(state.pty.lock().is_none());
    }
}
