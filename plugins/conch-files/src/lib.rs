//! Conch Files Plugin — dual-pane local & remote file explorer with transfer.

mod format;
pub(crate) mod local;
pub(crate) mod pane;
mod remote;
pub(crate) mod sftp_direct;

use std::collections::HashMap;
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use conch_plugin_sdk::{
    declare_plugin,
    widgets::{PluginEvent, SplitDirection, TextStyle, Widget, WidgetEvent},
    HostApi, PanelHandle, PanelLocation, PluginInfo, PluginType,
};

use pane::{Pane, PaneMode};

/// Log a message through the HostApi.
fn host_log(api: &HostApi, level: u8, msg: &str) {
    if let Ok(c) = CString::new(msg) {
        (api.log)(level, c.as_ptr());
    }
}

/// Set the global status bar via the HostApi. `progress` < 0 hides the bar.
fn host_set_status(api: &HostApi, msg: &str, level: u8, progress: f32) {
    if let Ok(c) = CString::new(msg) {
        (api.set_status)(c.as_ptr(), level, progress);
    }
}

/// Clear the global status bar.
#[allow(dead_code)]
fn host_clear_status(api: &HostApi) {
    (api.set_status)(std::ptr::null(), 0, -1.0);
}

/// A single file/directory entry.
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<u64>,
}

/// Cached info about an SSH session.
struct SshSessionInfo {
    host: String,
    user: String,
}

/// Thread-safe inner state for transfer progress.
struct TransferInner {
    message: Option<(String, TextStyle)>,
    active: bool,
    needs_refresh_local: bool,
    needs_refresh_remote: bool,
}

/// Shared transfer state accessible from background threads.
#[derive(Clone)]
struct SharedTransferState {
    inner: Arc<Mutex<TransferInner>>,
    cancelled: Arc<AtomicBool>,
}

impl SharedTransferState {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TransferInner {
                message: None,
                active: false,
                needs_refresh_local: false,
                needs_refresh_remote: false,
            })),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    fn is_active(&self) -> bool {
        self.inner.lock().unwrap().active
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    fn start(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.active = true;
        inner.needs_refresh_local = false;
        inner.needs_refresh_remote = false;
        self.cancelled.store(false, Ordering::Relaxed);
    }

    fn finish(&self, msg: String, style: TextStyle, refresh_local: bool, refresh_remote: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.active = false;
        inner.message = Some((msg, style));
        inner.needs_refresh_local = refresh_local;
        inner.needs_refresh_remote = refresh_remote;
    }

    fn set_message(&self, msg: String, style: TextStyle) {
        let mut inner = self.inner.lock().unwrap();
        inner.message = Some((msg, style));
    }

    fn snapshot(&self) -> (Option<(String, TextStyle)>, bool) {
        let inner = self.inner.lock().unwrap();
        (inner.message.clone(), inner.active)
    }
}

/// The dual-pane file explorer plugin state.
struct FilesPlugin {
    api: &'static HostApi,
    _panel: PanelHandle,

    /// Top pane — always local.
    local_pane: Pane,
    /// Bottom pane — remote when SSH active, local otherwise.
    remote_pane: Pane,

    /// Known SSH sessions.
    ssh_sessions: HashMap<u64, SshSessionInfo>,
    active_session_id: Option<u64>,

    /// Shared transfer progress/status.
    transfer: SharedTransferState,
}

impl FilesPlugin {
    fn new(api: &'static HostApi) -> Self {
        host_log(api, 2, "Files plugin initializing");

        let name = CString::new("Files").unwrap();
        let icon = CString::new("tab-files").unwrap();
        let panel = (api.register_panel)(PanelLocation::Left, name.as_ptr(), icon.as_ptr());

        for event in &["ssh.session_ready", "ssh.session_closed", "app.tab_changed"] {
            let ev = CString::new(*event).unwrap();
            (api.subscribe)(ev.as_ptr());
        }

        FilesPlugin {
            api,
            _panel: panel,
            local_pane: Pane::new_local("local"),
            remote_pane: Pane::new_local("remote"),
            ssh_sessions: HashMap::new(),
            active_session_id: None,
            transfer: SharedTransferState::new(),
        }
    }

    // -------------------------------------------------------------------
    // Event handling
    // -------------------------------------------------------------------

    fn handle_event(&mut self, event: PluginEvent) {
        match event {
            PluginEvent::Widget(widget_event) => self.handle_widget_event(widget_event),
            PluginEvent::BusEvent { event_type, data } => {
                self.handle_bus_event(&event_type, data);
            }
            PluginEvent::Shutdown => {
                // Cancel any active transfer on shutdown.
                if self.transfer.is_active() {
                    self.transfer.cancel();
                }
                host_log(self.api, 2, "Files plugin shutting down");
            }
            _ => {}
        }
    }

    fn handle_widget_event(&mut self, event: WidgetEvent) {
        // Drain refresh flags from completed background transfers.
        {
            let mut inner = self.transfer.inner.lock().unwrap();
            if inner.needs_refresh_local {
                inner.needs_refresh_local = false;
                drop(inner);
                self.local_pane.refresh(Some(self.api));
            } else if inner.needs_refresh_remote {
                inner.needs_refresh_remote = false;
                drop(inner);
                self.remote_pane.refresh(Some(self.api));
            }
        }

        // Transfer buttons.
        if let WidgetEvent::ButtonClick { ref id } = event {
            match id.as_str() {
                "transfer_download" => {
                    self.do_download();
                    return;
                }
                "transfer_upload" => {
                    self.do_upload();
                    return;
                }
                "transfer_cancel" => {
                    self.transfer.cancel();
                    self.transfer.set_message("Cancelling...".into(), TextStyle::Warn);
                    host_set_status(self.api, "Cancelling transfer...", 1, -1.0);
                    return;
                }
                _ => {}
            }
        }

        // Route to the correct pane.
        let api = Some(self.api as &HostApi);
        if !self.local_pane.handle_widget_event(&event, api) {
            self.remote_pane.handle_widget_event(&event, api);
        }
    }

    fn handle_bus_event(&mut self, event_type: &str, data: serde_json::Value) {
        match event_type {
            "ssh.session_ready" => {
                let session_id = data["session_id"].as_u64().unwrap_or(0);
                let host = data["host"].as_str().unwrap_or("").to_string();
                let user = data["user"].as_str().unwrap_or("").to_string();

                host_log(
                    self.api,
                    1,
                    &format!("SSH session ready: {session_id} ({user}@{host})"),
                );

                self.ssh_sessions.insert(session_id, SshSessionInfo {
                    host: host.clone(),
                    user: user.clone(),
                });

                // Switch the remote pane to the new SSH session.
                self.active_session_id = Some(session_id);
                self.remote_pane.switch_to_remote(session_id, &host, &user, self.api);
            }

            "ssh.session_closed" => {
                let session_id = data["session_id"].as_u64().unwrap_or(0);
                self.ssh_sessions.remove(&session_id);

                // If the closed session was our active remote, fall back to local.
                if self.active_session_id == Some(session_id) {
                    self.active_session_id = None;
                    self.remote_pane.switch_to_local();
                }
            }

            "app.tab_changed" => {
                let session_id = data["session_id"].as_u64();
                let is_ssh = data["is_ssh"].as_bool().unwrap_or(false);

                if is_ssh {
                    if let Some(sid) = session_id {
                        if let Some(info) = self.ssh_sessions.get(&sid) {
                            let host = info.host.clone();
                            let user = info.user.clone();
                            self.active_session_id = Some(sid);
                            // Only switch if not already on this session.
                            let already = matches!(
                                &self.remote_pane.mode,
                                PaneMode::Remote { session_id: active, .. } if *active == sid
                            );
                            if !already {
                                self.remote_pane.switch_to_remote(sid, &host, &user, self.api);
                            }
                        }
                    }
                } else {
                    self.active_session_id = None;
                    self.remote_pane.switch_to_local();
                }
            }

            _ => {}
        }
    }

    // -------------------------------------------------------------------
    // Transfer operations (async — spawns background threads)
    // -------------------------------------------------------------------

    /// Download: remote pane selection → local pane directory.
    fn do_download(&mut self) {
        if self.transfer.is_active() {
            self.transfer.set_message("Transfer already in progress".into(), TextStyle::Warn);
            return;
        }

        let Some(remote_name) = self.remote_pane.selected_row.clone() else {
            self.transfer.set_message("No file selected in remote pane".into(), TextStyle::Warn);
            return;
        };

        let remote_path = self.remote_pane.selected_path().unwrap();
        let local_dest = join_path(&self.local_pane.current_path, &remote_name);
        let is_dir = self.remote_pane.selected_is_dir();
        let file_size = self.remote_pane.selected_size();

        match &self.remote_pane.mode {
            PaneMode::Remote { session_id, .. } => {
                let session_id = *session_id;
                let api = self.api;
                let state = self.transfer.clone();

                state.start();
                let size_label = if file_size > 0 { format_bytes(file_size) } else { "?".into() };
                let init_label = format!("{remote_name} — 0 B / {size_label}");
                host_set_status(api, &init_label, 0, 0.0);
                state.set_message(init_label, TextStyle::Secondary);

                let name = remote_name.clone();
                std::thread::Builder::new()
                    .name("plugin:File Explorer".into())
                    .spawn(move || {
                        // Try direct SFTP vtable first, fall back to query_plugin IPC.
                        let sftp = sftp_direct::SftpAccess::acquire(api, session_id);
                        if sftp.is_some() {
                            host_log(api, 1, "download: using direct SFTP vtable");
                        } else {
                            host_log(api, 1, "download: falling back to query_plugin IPC");
                        }

                        let result = if is_dir {
                            download_dir(api, session_id, &remote_path, &local_dest, &state, sftp.as_ref())
                        } else if let Some(ref sftp) = sftp {
                            // Direct path: raw bytes, no base64.
                            download_file_direct(sftp, &remote_path, &local_dest, file_size, &name, api, &state)
                        } else {
                            // Fallback: query_plugin IPC with base64.
                            let name2 = name.clone();
                            let progress_cb = move |downloaded: u64, total: u64| {
                                if total > 0 {
                                    let frac = (downloaded as f32 / total as f32).min(1.0);
                                    let label = format!("{name2} — {} / {}", format_bytes(downloaded), format_bytes(total));
                                    host_set_status(api, &label, 0, frac);
                                }
                            };
                            let cb: Option<&dyn Fn(u64, u64)> = if file_size > 0 {
                                Some(&progress_cb)
                            } else {
                                None
                            };
                            match remote::read_file_with_progress(api, session_id, &remote_path, file_size, cb) {
                                Ok(data) => std::fs::write(&local_dest, &data).map_err(|e| e.to_string()),
                                Err(e) => Err(e),
                            }
                        };

                        let cancelled = state.is_cancelled();
                        match result {
                            _ if cancelled => {
                                host_set_status(api, "Download cancelled", 1, -1.0);
                                state.finish(
                                    "Download cancelled".into(),
                                    TextStyle::Warn,
                                    true,
                                    false,
                                );
                            }
                            Ok(()) => {
                                host_set_status(api, &format!("Downloaded: {name}"), 3, -1.0);
                                state.finish(
                                    format!("Downloaded: {name}"),
                                    TextStyle::Secondary,
                                    true,
                                    false,
                                );
                            }
                            Err(e) => {
                                host_set_status(api, &format!("Download failed: {e}"), 2, -1.0);
                                state.finish(
                                    format!("Download failed: {e}"),
                                    TextStyle::Error,
                                    false,
                                    false,
                                );
                            }
                        }
                    })
                    .ok();
            }
            PaneMode::Local => {
                // Local-to-local copy: fast enough to do inline.
                let result = if is_dir {
                    copy_dir_local(&remote_path, &local_dest)
                } else {
                    std::fs::copy(&remote_path, &local_dest)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                };
                match result {
                    Ok(()) => {
                        self.transfer.set_message(
                            format!("Downloaded: {remote_name}"),
                            TextStyle::Secondary,
                        );
                        self.local_pane.refresh(Some(self.api));
                    }
                    Err(e) => {
                        self.transfer.set_message(
                            format!("Download failed: {e}"),
                            TextStyle::Error,
                        );
                    }
                }
            }
        }
    }

    /// Upload: local pane selection → remote pane directory.
    fn do_upload(&mut self) {
        if self.transfer.is_active() {
            self.transfer.set_message("Transfer already in progress".into(), TextStyle::Warn);
            return;
        }

        let Some(local_name) = self.local_pane.selected_row.clone() else {
            self.transfer.set_message("No file selected in local pane".into(), TextStyle::Warn);
            return;
        };

        let local_path = self.local_pane.selected_path().unwrap();
        let remote_dest = if self.remote_pane.current_path == "." {
            local_name.clone()
        } else {
            join_path(&self.remote_pane.current_path, &local_name)
        };
        let is_dir = self.local_pane.selected_is_dir();

        match &self.remote_pane.mode {
            PaneMode::Remote { session_id, .. } => {
                let session_id = *session_id;
                let api = self.api;
                let state = self.transfer.clone();

                state.start();
                let file_size_upload = self.local_pane.selected_size();
                let size_label = if file_size_upload > 0 { format_bytes(file_size_upload) } else { "?".into() };
                let init_label = format!("{local_name} — 0 B / {size_label}");
                host_set_status(api, &init_label, 0, 0.0);
                state.set_message(init_label, TextStyle::Secondary);

                let name = local_name.clone();
                std::thread::Builder::new()
                    .name("plugin:File Explorer".into())
                    .spawn(move || {
                        // Try direct SFTP vtable first, fall back to query_plugin IPC.
                        let sftp = sftp_direct::SftpAccess::acquire(api, session_id);
                        if sftp.is_some() {
                            host_log(api, 1, "upload: using direct SFTP vtable");
                        } else {
                            host_log(api, 1, "upload: falling back to query_plugin IPC");
                        }

                        let result = if is_dir {
                            upload_dir(api, session_id, &local_path, &remote_dest, &state, sftp.as_ref())
                        } else if let Some(ref sftp) = sftp {
                            upload_file_direct(sftp, &local_path, &remote_dest, &name, api, &state)
                        } else {
                            match std::fs::read(&local_path) {
                                Ok(data) => remote::write_file(api, session_id, &remote_dest, &data),
                                Err(e) => Err(e.to_string()),
                            }
                        };

                        let cancelled = state.is_cancelled();
                        match result {
                            _ if cancelled => {
                                host_set_status(api, "Upload cancelled", 1, -1.0);
                                state.finish(
                                    "Upload cancelled".into(),
                                    TextStyle::Warn,
                                    false,
                                    true,
                                );
                            }
                            Ok(()) => {
                                host_set_status(api, &format!("Uploaded: {name}"), 3, -1.0);
                                state.finish(
                                    format!("Uploaded: {name}"),
                                    TextStyle::Secondary,
                                    false,
                                    true,
                                );
                            }
                            Err(e) => {
                                host_set_status(api, &format!("Upload failed: {e}"), 2, -1.0);
                                state.finish(
                                    format!("Upload failed: {e}"),
                                    TextStyle::Error,
                                    false,
                                    false,
                                );
                            }
                        }
                    })
                    .ok();
            }
            PaneMode::Local => {
                // Local-to-local copy: fast enough to do inline.
                let result = if is_dir {
                    copy_dir_local(&local_path, &remote_dest)
                } else {
                    std::fs::copy(&local_path, &remote_dest)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                };
                match result {
                    Ok(()) => {
                        self.transfer.set_message(
                            format!("Uploaded: {local_name}"),
                            TextStyle::Secondary,
                        );
                        self.remote_pane.refresh(Some(self.api));
                    }
                    Err(e) => {
                        self.transfer.set_message(
                            format!("Upload failed: {e}"),
                            TextStyle::Error,
                        );
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Rendering
    // -------------------------------------------------------------------

    fn render(&self) -> Vec<Widget> {
        let local_widgets = self.local_pane.render_widgets();
        let remote_widgets = self.remote_pane.render_widgets();
        let (_msg, active) = self.transfer.snapshot();

        // Empty heading suppresses the default "Files" panel header.
        let mut all = vec![Widget::Heading { text: "".into() }];

        // Top half: remote pane.
        let remote_container = Widget::Vertical {
            id: Some("remote_container".into()),
            children: remote_widgets,
            spacing: Some(2.0),
        };

        // Transfer buttons (centered). Disabled during active transfer.
        let mut transfer_children = vec![
            Widget::Button {
                id: "transfer_upload".into(),
                label: "".into(),
                icon: Some("go-up".into()),
                enabled: Some(self.local_pane.selected_row.is_some() && !active),
            },
            Widget::Button {
                id: "transfer_download".into(),
                label: "".into(),
                icon: Some("go-down".into()),
                enabled: Some(self.remote_pane.selected_row.is_some() && !active),
            },
        ];

        // Show cancel button during active transfer.
        if active {
            transfer_children.push(Widget::Button {
                id: "transfer_cancel".into(),
                label: "Cancel".into(),
                icon: None,
                enabled: Some(true),
            });
        }

        let transfer_buttons = Widget::Horizontal {
            id: Some("transfer_bar".into()),
            children: transfer_children,
            spacing: Some(8.0),
            centered: Some(true),
        };

        let transfer_bar = Widget::Vertical {
            id: Some("transfer_section".into()),
            children: vec![transfer_buttons],
            spacing: Some(2.0),
        };

        let local_container = Widget::Vertical {
            id: Some("local_container".into()),
            children: local_widgets,
            spacing: Some(2.0),
        };

        // Bottom half includes transfer bar above local pane.
        let bottom = Widget::Vertical {
            id: Some("bottom_half".into()),
            children: vec![transfer_bar, local_container],
            spacing: Some(4.0),
        };

        all.push(Widget::SplitPane {
            id: "file_split".into(),
            direction: SplitDirection::Vertical,
            ratio: 0.47,
            resizable: false,
            left: Box::new(remote_container),
            right: Box::new(bottom),
        });
        all
    }

    fn handle_query(&mut self, _method: &str, _args: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "status": "error", "message": "not implemented" })
    }
}

/// Join a directory path and a file/folder name.
fn join_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// Recursively copy a local directory to another local path.
fn copy_dir_local(src: &str, dest: &str) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("mkdir {dest}: {e}"))?;
    let entries = std::fs::read_dir(src).map_err(|e| format!("read_dir {src}: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let src_path = join_path(src, &name);
        let dst_path = join_path(dest, &name);
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_local(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path).map_err(|e| format!("copy {name}: {e}"))?;
        }
    }
    Ok(())
}

/// Count files recursively in a remote directory via SFTP.
fn count_remote_files(api: &HostApi, session_id: u64, dir: &str) -> Result<usize, String> {
    let entries = remote::list_dir(api, session_id, dir)?;
    let mut count = 0usize;
    for entry in &entries {
        if entry.is_dir {
            count += count_remote_files(api, session_id, &join_path(dir, &entry.name))?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}

/// Count files recursively in a local directory.
fn count_local_files(dir: &str) -> Result<usize, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir {dir}: {e}"))?;
    let mut count = 0usize;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            count += count_local_files(&entry.path().to_string_lossy())?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}

/// Download a single file via direct SFTP vtable (raw bytes, no base64).
fn download_file_direct(
    sftp: &sftp_direct::SftpAccess,
    remote_path: &str,
    local_dest: &str,
    file_size: u64,
    name: &str,
    api: &HostApi,
    state: &SharedTransferState,
) -> Result<(), String> {
    use std::io::Write;

    let mut file = std::fs::File::create(local_dest).map_err(|e| format!("create {local_dest}: {e}"))?;
    let chunk_size: u64 = 1024 * 1024;
    let mut offset: u64 = 0;

    loop {
        if state.is_cancelled() {
            return Err("cancelled".into());
        }

        let chunk = sftp.read_chunk(remote_path, offset, chunk_size)?;
        let n = chunk.len();
        if n == 0 {
            break;
        }
        file.write_all(&chunk).map_err(|e| format!("write: {e}"))?;
        offset += n as u64;

        if file_size > 0 {
            let frac = (offset as f32 / file_size as f32).min(1.0);
            let label = format!("{} — {} / {}", name, format_bytes(offset), format_bytes(file_size));
            host_set_status(api, &label, 0, frac);
            state.set_message(label, TextStyle::Secondary);
        } else {
            let label = format!("{} — {}", name, format_bytes(offset));
            host_set_status(api, &label, 0, -1.0);
            state.set_message(label, TextStyle::Secondary);
        }

        if (n as u64) < chunk_size {
            break;
        }
    }

    Ok(())
}

/// Upload a single file via direct SFTP vtable with chunked writes and progress.
fn upload_file_direct(
    sftp: &sftp_direct::SftpAccess,
    local_path: &str,
    remote_dest: &str,
    name: &str,
    api: &HostApi,
    state: &SharedTransferState,
) -> Result<(), String> {
    use std::io::Read;

    let file_meta = std::fs::metadata(local_path).map_err(|e| format!("stat {name}: {e}"))?;
    let total = file_meta.len();
    let total_label = format_bytes(total);
    let chunk_size: usize = 1024 * 1024; // 1 MB chunks

    let mut file = std::fs::File::open(local_path).map_err(|e| format!("open {name}: {e}"))?;
    let mut offset: u64 = 0;
    let mut buf = vec![0u8; chunk_size];
    let mut first = true;

    loop {
        if state.is_cancelled() {
            return Err("cancelled".into());
        }

        let n = file.read(&mut buf).map_err(|e| format!("read {name}: {e}"))?;
        if n == 0 {
            break;
        }

        sftp.write_at(remote_dest, &buf[..n], offset, first)?;
        first = false;
        offset += n as u64;

        let frac = if total > 0 { (offset as f32 / total as f32).min(1.0) } else { -1.0 };
        let label = format!("{name} — {} / {total_label}", format_bytes(offset));
        host_set_status(api, &label, 0, frac);
        state.set_message(label, TextStyle::Secondary);
    }

    Ok(())
}

/// Format a byte count as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Download a directory, using direct SFTP if available, else IPC fallback.
fn download_dir(
    api: &HostApi,
    session_id: u64,
    remote_dir: &str,
    local_dir: &str,
    state: &SharedTransferState,
    sftp: Option<&sftp_direct::SftpAccess>,
) -> Result<(), String> {
    let total = if let Some(sftp) = sftp {
        count_remote_files_direct(sftp, remote_dir).unwrap_or(1).max(1)
    } else {
        count_remote_files(api, session_id, remote_dir).unwrap_or(1).max(1)
    };
    let mut completed = 0usize;
    download_dir_inner(api, session_id, remote_dir, local_dir, state, &mut completed, total, sftp)
}

fn download_dir_inner(
    api: &HostApi,
    session_id: u64,
    remote_dir: &str,
    local_dir: &str,
    state: &SharedTransferState,
    completed: &mut usize,
    total: usize,
    sftp: Option<&sftp_direct::SftpAccess>,
) -> Result<(), String> {
    if state.is_cancelled() {
        return Err("cancelled".into());
    }

    std::fs::create_dir_all(local_dir).map_err(|e| format!("mkdir {local_dir}: {e}"))?;

    let entries = if let Some(sftp) = sftp {
        sftp.list_dir(remote_dir)?
    } else {
        remote::list_dir(api, session_id, remote_dir)?
    };

    for entry in &entries {
        if state.is_cancelled() {
            return Err("cancelled".into());
        }

        let remote_path = join_path(remote_dir, &entry.name);
        let local_path = join_path(local_dir, &entry.name);
        if entry.is_dir {
            download_dir_inner(api, session_id, &remote_path, &local_path, state, completed, total, sftp)?;
        } else {
            let data = if let Some(sftp) = sftp {
                // Direct: read in chunks, assemble full file.
                let mut all = Vec::new();
                let mut offset = 0u64;
                let chunk_size = 1024 * 1024u64;
                loop {
                    if state.is_cancelled() {
                        return Err("cancelled".into());
                    }
                    let chunk = sftp.read_chunk(&remote_path, offset, chunk_size)?;
                    let n = chunk.len();
                    all.extend_from_slice(&chunk);
                    if (n as u64) < chunk_size {
                        break;
                    }
                    offset += n as u64;
                }
                all
            } else {
                remote::read_file(api, session_id, &remote_path)?
            };
            std::fs::write(&local_path, &data).map_err(|e| format!("write {}: {e}", entry.name))?;
            *completed += 1;
            let fraction = *completed as f32 / total as f32;
            let label = format!("Downloading: {}/{} files", completed, total);
            host_set_status(api, &label, 0, fraction);
        }
    }
    Ok(())
}

/// Count remote files using direct SFTP vtable.
fn count_remote_files_direct(sftp: &sftp_direct::SftpAccess, dir: &str) -> Result<usize, String> {
    let entries = sftp.list_dir(dir)?;
    let mut count = 0usize;
    for entry in &entries {
        if entry.is_dir {
            count += count_remote_files_direct(sftp, &join_path(dir, &entry.name))?;
        } else {
            count += 1;
        }
    }
    Ok(count)
}

/// Upload a directory, using direct SFTP if available, else IPC fallback.
fn upload_dir(
    api: &HostApi,
    session_id: u64,
    local_dir: &str,
    remote_dir: &str,
    state: &SharedTransferState,
    sftp: Option<&sftp_direct::SftpAccess>,
) -> Result<(), String> {
    let total = count_local_files(local_dir).unwrap_or(1).max(1);
    let mut completed = 0usize;
    upload_dir_inner(api, session_id, local_dir, remote_dir, state, &mut completed, total, sftp)
}

fn upload_dir_inner(
    api: &HostApi,
    session_id: u64,
    local_dir: &str,
    remote_dir: &str,
    state: &SharedTransferState,
    completed: &mut usize,
    total: usize,
    sftp: Option<&sftp_direct::SftpAccess>,
) -> Result<(), String> {
    if state.is_cancelled() {
        return Err("cancelled".into());
    }

    if let Some(sftp) = sftp {
        sftp.mkdir(remote_dir)?;
    } else {
        remote::mkdir(api, session_id, remote_dir)?;
    }

    let entries = std::fs::read_dir(local_dir).map_err(|e| format!("read_dir {local_dir}: {e}"))?;
    for entry in entries {
        if state.is_cancelled() {
            return Err("cancelled".into());
        }

        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let local_path = join_path(local_dir, &name);
        let remote_path = join_path(remote_dir, &name);
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            upload_dir_inner(api, session_id, &local_path, &remote_path, state, completed, total, sftp)?;
        } else {
            let data = std::fs::read(&local_path).map_err(|e| format!("read {name}: {e}"))?;
            if let Some(sftp) = sftp {
                sftp.write_file(&remote_path, &data)?;
            } else {
                remote::write_file(api, session_id, &remote_path, &data)?;
            }
            *completed += 1;
            let fraction = *completed as f32 / total as f32;
            let label = format!("Uploading: {}/{} files", completed, total);
            host_set_status(api, &label, 0, fraction);
        }
    }
    Ok(())
}

declare_plugin!(
    info: PluginInfo {
        name: c"File Explorer".as_ptr(),
        description: c"Browse local and remote files".as_ptr(),
        version: c"0.3.0".as_ptr(),
        plugin_type: PluginType::Panel,
        panel_location: PanelLocation::Left,
        dependencies: std::ptr::null(),
        num_dependencies: 0,
    },
    state: FilesPlugin,
    setup: |api| FilesPlugin::new(api),
    event: |state, event| state.handle_event(event),
    render: |state| state.render(),
    query: |state, method, args| state.handle_query(method, args),
);
