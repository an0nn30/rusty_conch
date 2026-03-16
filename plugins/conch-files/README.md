# conch-files

A native dual-pane file explorer plugin for Conch with upload/download support. The panel is split into two halves — the top pane shows the remote (SFTP) filesystem when an SSH tab is active, and the bottom pane always shows the local filesystem.

## Features

- **Dual-pane layout** — top pane (remote/local) and bottom pane (local) split the panel
- **Direct SFTP transfers** — uses the `SftpVtable` C ABI for raw-bytes file transfer, bypassing JSON/base64 overhead
- **Chunked progress** — downloads and uploads stream in 1 MB chunks with real-time progress in the status bar
- **Cancellation** — transfers can be cancelled mid-stream via the cancel button
- **Directory transfers** — recursive upload/download of entire directories with file-count progress
- **IPC fallback** — falls back to `query_plugin` JSON/base64 path when the direct SFTP vtable is unavailable
- **Auto-switching** — the remote pane reacts to `ssh.session_ready`, `ssh.session_closed`, and `app.tab_changed` bus events to follow the focused session
- **Path resolution** — remote pane resolves the actual home directory path via SFTP `realpath`
- **Navigation** — each pane has independent back/forward history, home button, and path text input
- **Sorting** — click column headers to sort by name, extension, size, or modified date
- **Column visibility** — right-click column headers to toggle Ext/Size/Modified columns
- **Context menu** — right-click rows for New Folder, Rename, Delete, Copy Path

## Architecture

```
src/
  lib.rs          — plugin entry point, dual-pane orchestration, transfer logic
  pane.rs         — reusable single-pane file browser (navigation, events, rendering)
  local.rs        — local filesystem listing (std::fs::read_dir)
  remote.rs       — SFTP operations via query_plugin("SSH Manager", ...) IPC fallback
  sftp_direct.rs  — safe wrapper around SftpHandle for direct SFTP vtable access
  format.rs       — file size, date, and extension formatting helpers
```

The plugin is a `cdylib` built with the `declare_plugin!` macro from `conch_plugin_sdk`. It registers a left-side panel and communicates with the host through:

- **Widget events** — button clicks, table interactions, toolbar input (prefixed per pane)
- **Bus events** — subscribes to `ssh.session_ready`, `ssh.session_closed`, `app.tab_changed`
- **Direct SFTP vtable** — acquires an `SftpHandle` from the host's SFTP registry for raw-bytes SFTP operations (no serialization overhead)
- **Plugin queries** — fallback path calling `conch-ssh` SFTP services (`list_dir`, `read_file`, `write_file`, `mkdir`, `rename`, `delete`, `realpath`)

## Transfer flow

1. Select a file in one pane
2. Click the upload (up arrow) or download (down arrow) button
3. A background thread acquires a direct `SftpHandle` (or falls back to IPC)
4. Files stream in 1 MB chunks with progress updates to the status bar
5. Cancellation is checked between each chunk
6. For local-only mode: uses `std::fs::copy`

## Dependencies

- `conch_plugin_sdk` — plugin ABI, widget types, and SFTP vtable types
- `conch-ssh` — provides SFTP operations for remote browsing (direct vtable or IPC fallback)
- `hostname` — resolves local machine name for the pane title
- `dirs` — resolves the user's home directory
- `serde_json` — JSON serialization for IPC fallback path
- `base64` — encoding/decoding for IPC fallback file transfer
