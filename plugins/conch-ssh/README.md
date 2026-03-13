# conch-ssh

Native SSH session manager plugin for Conch. Built as a C ABI plugin using the Conch Plugin SDK, it provides a full SSH connection manager in the sidebar panel.

## Features

### Connection Management
- **Server tree panel** — sidebar tree view with folders, servers, and `~/.ssh/config` hosts
- **Quick connect** — search bar for fast hostname-based connections
- **User-defined folders** — organize servers into collapsible folders with create, rename, and delete support
- **Add/edit server dialog** — rich form with host, port, username, auth method, key path, proxy command, and proxy jump fields
- **Duplicate and delete** — context menu actions on any server entry
- **Persistent config** — server list and folder structure saved to plugin config storage

### SSH Connectivity
- **Password and key-based auth** — supports both authentication methods with masked password prompts
- **Host key verification** — interactive fingerprint confirmation dialog on first connect, with `known_hosts` persistence
- **`~/.ssh/config` import** — automatically parses and displays hosts from the user's SSH config
- **Proxy support** — `ProxyCommand` and `ProxyJump` directives from SSH config are parsed and stored
- **PTY allocation** — opens a full terminal session tab on successful connection
- **Session lifecycle** — clean disconnect handling with session cleanup

### UI
- **Animated collapsible folders** — smooth expand/collapse via egui `CollapsingState`
- **Icon support** — folder and computer icons from the shared icon cache with theme-aware tinting
- **Context menus** — right-click on servers (connect, edit, duplicate, copy hostname, delete) and folders (rename, delete)
- **Toolbar** — right-aligned "New Folder" button in the panel header
- **Pinned footer** — "+ New Connection" button anchored to the bottom of the panel
- **Connected indicator** — active connections shown in bold

## Architecture

The plugin is compiled as a shared library (`.dylib`/`.so`/`.dll`) and communicates with the host app through the C ABI plugin vtable (`HostApi`). It uses:

- **`russh`** — async SSH2 client library for connections
- **`conch_plugin_sdk`** — declarative widget tree (JSON-serialized `Widget` enum) for UI rendering
- **Tokio runtime** — internal async runtime for SSH operations
- **Host bridge functions** — `show_form`, `show_prompt`, `show_confirm` for dialogs; `open_session` for terminal tabs; `get_config`/`set_config` for persistence

## File Structure

| File | Purpose |
|------|---------|
| `lib.rs` | Plugin entry point, event handling, connection flow |
| `config.rs` | Server/folder data model and serialization |
| `server_tree.rs` | Builds the declarative widget tree for the Sessions panel |
| `session_backend.rs` | SSH session state and russh channel management |
| `ssh_config_parser.rs` | Parses `~/.ssh/config` into `ServerEntry` values |
| `known_hosts.rs` | Host key storage and verification |
