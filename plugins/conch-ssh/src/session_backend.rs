//! SSH session backend — bridges a russh SSH channel to the host's terminal
//! emulator via the SessionBackendVtable and OutputCallback.

use std::ffi::c_void;

use conch_plugin_sdk::{OutputCallback, SessionBackendVtable, SessionHandle};
use russh::ChannelMsg;
use tokio::runtime::Handle as TokioHandle;
use tokio::sync::mpsc;

/// Function pointer type for `host_close_session`.
type CloseSessionFn = extern "C" fn(SessionHandle);

/// Send-safe wrapper for the output callback + context pointer pair.
/// SAFETY: The output callback and context are thread-safe by contract with
/// the host — they are designed to be called from any thread.
struct OutputSink {
    cb: OutputCallback,
    ctx: usize, // stored as usize to avoid *mut c_void Send issues
}

unsafe impl Send for OutputSink {}

impl OutputSink {
    fn new(cb: OutputCallback, ctx: *mut c_void) -> Self {
        Self { cb, ctx: ctx as usize }
    }

    fn push(&self, data: &[u8]) {
        if !data.is_empty() {
            (self.cb)(self.ctx as *mut c_void, data.as_ptr(), data.len());
        }
    }
}

/// Per-connection state shared between the vtable callbacks and the async loop.
pub struct SshBackendState {
    /// Set after `activate()` — sends input to the channel loop.
    input_tx: Option<mpsc::UnboundedSender<BackendMsg>>,
    /// SSH connection handle for opening additional channels (exec, SFTP).
    ssh_handle: Option<russh::client::Handle<super::SshHandler>>,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub connected: bool,
}

enum BackendMsg {
    Write(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Shutdown,
}

impl SshBackendState {
    /// Pre-allocate a backend state (before we have the output callback).
    /// Call `activate()` after getting the callback from `open_session`.
    pub fn new_preallocated(host: String, user: String, port: u16) -> Box<Self> {
        Box::new(Self {
            input_tx: None,
            ssh_handle: None,
            host,
            user,
            port,
            connected: false,
        })
    }

    /// Convert to a raw handle for passing to the host vtable.
    pub fn as_handle_ptr(state: &mut Box<Self>) -> *mut c_void {
        &mut **state as *mut Self as *mut c_void
    }

    /// Wire up the SSH channel and output callback. Must be called after
    /// `open_session` returns the output callback.
    pub fn activate(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        ssh_handle: russh::client::Handle<super::SshHandler>,
        output_cb: OutputCallback,
        output_ctx: *mut c_void,
        rt: &TokioHandle,
        session_handle: SessionHandle,
        close_session: CloseSessionFn,
    ) {
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        self.input_tx = Some(input_tx);
        self.ssh_handle = Some(ssh_handle);
        self.connected = true;

        let sink = OutputSink::new(output_cb, output_ctx);
        rt.spawn(channel_loop(channel, sink, input_rx, session_handle, close_session));
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the SSH connection handle (for opening exec/SFTP channels).
    pub fn ssh_handle(&self) -> Option<&russh::client::Handle<super::SshHandler>> {
        self.ssh_handle.as_ref()
    }

    /// Execute a command on a separate SSH channel, returning (stdout, stderr, exit_code).
    pub async fn exec(&self, command: &str) -> Result<(String, String, u32), String> {
        let handle = self.ssh_handle.as_ref()
            .ok_or_else(|| "session not connected".to_string())?;

        let mut channel = handle.channel_open_session()
            .await
            .map_err(|e| format!("failed to open exec channel: {e}"))?;

        channel.exec(true, command)
            .await
            .map_err(|e| format!("exec failed: {e}"))?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = 0u32;

        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) => {
                    stdout.extend_from_slice(&data[..]);
                }
                Some(ChannelMsg::ExtendedData { data, ext }) => {
                    if ext == 1 {
                        // stderr
                        stderr.extend_from_slice(&data[..]);
                    } else {
                        stdout.extend_from_slice(&data[..]);
                    }
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    exit_code = exit_status;
                }
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                    break;
                }
                _ => {}
            }
        }

        Ok((
            String::from_utf8_lossy(&stdout).to_string(),
            String::from_utf8_lossy(&stderr).to_string(),
            exit_code,
        ))
    }
}

/// Build the vtable the host uses to send input/resize/shutdown.
pub fn ssh_vtable() -> SessionBackendVtable {
    SessionBackendVtable {
        write: ssh_backend_write,
        resize: ssh_backend_resize,
        shutdown: ssh_backend_shutdown,
        drop: ssh_backend_drop,
    }
}

// ---------------------------------------------------------------------------
// Combined channel loop (owns the Channel, handles reads and writes)
// ---------------------------------------------------------------------------

async fn channel_loop(
    mut channel: russh::Channel<russh::client::Msg>,
    sink: OutputSink,
    mut input_rx: mpsc::UnboundedReceiver<BackendMsg>,
    session_handle: SessionHandle,
    close_session: CloseSessionFn,
) {
    let mut initiated_by_host = false;
    loop {
        tokio::select! {
            // Read from SSH channel → push to host terminal.
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        sink.push(&data[..]);
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        sink.push(&data[..]);
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        log::info!("SSH channel exited with status {exit_status}");
                        break;
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                        log::debug!("SSH channel closed/EOF");
                        break;
                    }
                    _ => {}
                }
            }

            // Write from host → SSH channel.
            input = input_rx.recv() => {
                match input {
                    Some(BackendMsg::Write(data)) => {
                        if let Err(e) = channel.data(&data[..]).await {
                            log::warn!("SSH write error: {e}");
                            break;
                        }
                    }
                    Some(BackendMsg::Resize { cols, rows }) => {
                        if let Err(e) = channel.window_change(cols as u32, rows as u32, 0, 0).await {
                            log::warn!("SSH resize error: {e}");
                        }
                    }
                    Some(BackendMsg::Shutdown) | None => {
                        initiated_by_host = true;
                        let _ = channel.eof().await;
                        let _ = channel.close().await;
                        break;
                    }
                }
            }
        }
    }

    // If the channel closed on its own (e.g. user typed "exit"), tell the host
    // to remove the tab. Skip if the host initiated the shutdown.
    if !initiated_by_host {
        log::info!("SSH channel ended, requesting host close session {:?}", session_handle);
        close_session(session_handle);
    }
}

// ---------------------------------------------------------------------------
// Vtable implementations
// ---------------------------------------------------------------------------

extern "C" fn ssh_backend_write(handle: *mut c_void, buf: *const u8, len: usize) {
    if handle.is_null() || buf.is_null() || len == 0 {
        return;
    }
    let state = unsafe { &*(handle as *const SshBackendState) };
    if let Some(tx) = &state.input_tx {
        let data = unsafe { std::slice::from_raw_parts(buf, len) }.to_vec();
        let _ = tx.send(BackendMsg::Write(data));
    }
}

extern "C" fn ssh_backend_resize(handle: *mut c_void, cols: u16, rows: u16) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &*(handle as *const SshBackendState) };
    if let Some(tx) = &state.input_tx {
        let _ = tx.send(BackendMsg::Resize { cols, rows });
    }
}

extern "C" fn ssh_backend_shutdown(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    let state = unsafe { &*(handle as *const SshBackendState) };
    if let Some(tx) = &state.input_tx {
        let _ = tx.send(BackendMsg::Shutdown);
    }
}

extern "C" fn ssh_backend_drop(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle as *mut SshBackendState));
    }
}
