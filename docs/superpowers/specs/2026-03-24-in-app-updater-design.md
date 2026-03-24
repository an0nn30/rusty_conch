# In-App Updater Design Spec

## Goal

Add automatic update checking and in-place update installation to Conch on macOS and Windows, using Tauri's built-in updater plugin and GitHub Releases as the distribution endpoint.

## Scope

- macOS and Windows only (Linux users update via package managers)
- Check for updates on startup (configurable) and via manual menu item
- Download and install updates in-place with user-controlled restart
- Use Tauri's Ed25519 signing for update verification (no Apple/Microsoft code signing initially)

## User Experience

### Startup Check

1. App launches, UI loads normally
2. After ~3 second delay (so UI is responsive), background check hits GitHub for `latest.json`
3. If a newer version exists, show a toast: **"Conch vX.Y.Z is available"** with an "Update Now" action button (dismiss/close button serves as "Later")
4. If up to date or check fails (no internet, API error), do nothing silently

### Manual Check

- **Conch > Check for Updates** (macOS) or **Help > Check for Updates** (Windows) menu item, always available regardless of settings
- If update available: same toast as startup flow
- If up to date: toast "You're running the latest version"
- If check fails: toast "Unable to check for updates"

### Download & Install

1. User clicks "Update Now"
2. Progress toast appears: "Downloading update..." with a progress indicator
3. Download completes, progress toast dismissed
4. Prompt dialog (using `ssh-overlay`/`ssh-form` pattern): **"Update installed. Restart now to apply?"**
   - **"Restart Now"** — app relaunches, update applied on relaunch
   - **"Restart Later"** — dismiss, update applies next time the user quits and reopens

### Settings

- New toggle in **Settings > Advanced**: "Check for updates on startup" (default: on)
- Backed by `config.conch.check_for_updates` (bool, default `true`)
- The menu item works regardless of this setting

## Architecture

### New File: `crates/conch_tauri/src/updater.rs`

Encapsulates all update logic.

**Managed state:** The `tauri-plugin-updater` `Update` object is opaque and cannot be serialized to the frontend. The check and install steps must share it via managed state:

```rust
pub(crate) struct PendingUpdate(pub Mutex<Option<tauri_plugin_updater::Update>>);
```

**Tauri commands:**

- `check_for_update(app: AppHandle, state: State<PendingUpdate>) -> Result<Option<UpdateInfo>, String>` — queries the GitHub endpoint via `tauri-plugin-updater`. If an update is available, stores the `Update` object in `PendingUpdate` and returns a serializable `UpdateInfo { version, body }`. Returns `Ok(None)` if up to date. Swallows network errors gracefully.
- `install_update(app: AppHandle, state: State<PendingUpdate>) -> Result<(), String>` — retrieves the stored `Update` from managed state, downloads and installs it. Emits `update-progress` events to the frontend during download. Does NOT restart.
- `restart_app(app: AppHandle)` — triggers app relaunch via `tauri_plugin_process::relaunch()` (not `app.restart()` which has a known macOS bug where it quits but does not relaunch — tauri-apps/tauri#13923).

**UpdateInfo (serializable to frontend):**

```rust
#[derive(Serialize)]
pub(crate) struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
}
```

### Startup Integration: `crates/conch_tauri/src/lib.rs`

In the existing `.setup()` closure, after app initialization:

```rust
if config.conch.check_for_updates {
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        // check for update, emit "update-available" event if found
    });
}
```

Guarded with `#[cfg(not(target_os = "linux"))]`.

### Plugin Registration: `crates/conch_tauri/src/lib.rs`

Add to the Tauri builder chain alongside existing plugins:

```rust
.plugin(tauri_plugin_updater::Builder::new().build())
```

Also add `tauri-plugin-process` for the reliable `relaunch()` function:

```rust
.plugin(tauri_plugin_process::init())
```

### Config: `crates/conch_core/src/config.rs`

Add to the `ConchConfig` struct:

```rust
#[serde(default = "default_true")]
pub check_for_updates: bool,
```

Default: `true`. Serialized as `[conch] check_for_updates = true` in TOML. A bare field at the `ConchConfig` level (alongside `keyboard`, `ui`, `plugins`) is fine for a single boolean — a sub-section would be over-engineering.

### Frontend: `crates/conch_tauri/frontend/index.html`

- Listen for `update-available` event → show toast via `toast.js` with version info and "Update Now" action button
- "Update Now" → `invoke('install_update')`, show progress toast, listen for `update-progress` events
- On download complete → show restart prompt dialog (using `ssh-overlay`/`ssh-form` pattern) with "Restart Now" / "Restart Later"
- "Restart Now" → `invoke('restart_app')`
- "Restart Later" → dismiss dialog
- Manual menu action `"check-for-updates"` → `invoke('check_for_update')`, show appropriate toast

The existing `toast.js` supports a single action button — "Update Now" maps to the action, and the existing close (X) button serves as "Later". No changes to `toast.js` needed.

### Menu: `crates/conch_tauri/src/lib.rs`

- **macOS:** Add "Check for Updates" to the **App** (Conch) menu, after "About Conch"
- **Windows/Linux:** Add a new **Help** menu with "Check for Updates"

Both emit a `menu-action` event with action `"check-for-updates"`.

### Tauri Config: `crates/conch_tauri/tauri.conf.json`

```json
{
  "bundle": {
    "createUpdaterArtifacts": "v2Compatible"
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/an0nn30/conch/releases/latest/download/latest.json"
      ],
      "pubkey": "<ED25519_PUBLIC_KEY>"
    }
  }
}
```

The `createUpdaterArtifacts` setting is required for `cargo tauri build` to produce the signed `.tar.gz` / `.sig` (macOS) and `.nsis.zip` / `.sig` (Windows) update artifacts.

### Capabilities: `crates/conch_tauri/capabilities/default.json`

Add permissions:

```json
"updater:default",
"process:allow-restart"
```

### Dependencies: `crates/conch_tauri/Cargo.toml`

```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

## Build Pipeline Migration

Switching from `cargo build` to `cargo tauri build` is the most significant infrastructure change. This section details what changes and what stays the same.

### What changes

**`cargo tauri build` replaces `cargo build --release -p conch_tauri`** on macOS and Windows. The Tauri bundler handles `.app` creation, DMG packaging, and code signing. It also produces the updater artifacts (`.tar.gz` + `.sig` on macOS, `.msi.zip` + `.sig` on Windows) when `createUpdaterArtifacts` is set.

**`bundle.active` must be `true`** in `tauri.conf.json` (currently `false`). The bundle section needs icon paths, identifier, and platform-specific settings.

**macOS universal binary:** The current pipeline manually does `cargo build` for two targets then `lipo` to create a universal binary. With `cargo tauri build`, use the `--target universal-apple-darwin` flag instead — Tauri's bundler handles the lipo internally and produces a universal `.app` + DMG + signed `.tar.gz` update artifact in one step.

**Windows installer:** The current pipeline uses a custom WiX `.wxs` file (`packaging/windows/conch.wxs`) with `wix build`. With `cargo tauri build`, Tauri's bundler can produce either NSIS or WiX-based installers. The simplest path: switch to Tauri's NSIS bundler (default) which also produces the NSIS `.exe` + `.sig` update artifact automatically. The custom WiX file becomes unnecessary.

### What stays the same

**Linux builds** stay on `cargo build` — no updater artifacts needed for Linux, and `cargo-deb`/`cargo-generate-rpm` work from the existing binary.

**Java SDK JAR build** — unchanged, runs before the Tauri build.

**`upload_asset.sh`** — unchanged, still uploads assets to the draft release.

**`latest.json` generation** — a new step that runs after macOS and Windows builds complete. It reads the `.sig` files produced by the bundler, assembles the JSON, and uploads it as a release asset. Alternatively, consider using `tauri-apps/tauri-action` which handles building, signing, and `latest.json` generation automatically.

### One-time setup

1. Generate Ed25519 keypair: `cargo tauri signer generate -w ~/.tauri/conch.key`
2. Add `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as GitHub repo secrets
3. Embed the public key in `tauri.conf.json` under `plugins.updater.pubkey`

### `latest.json` format (Tauri v2 standard)

```json
{
  "version": "2.1.0",
  "notes": "Bug fixes and improvements",
  "pub_date": "2026-03-24T00:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "<base64 sig from .sig file>",
      "url": "https://github.com/an0nn30/conch/releases/download/v2.1.0/Conch.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "<base64 sig from .sig file>",
      "url": "https://github.com/an0nn30/conch/releases/download/v2.1.0/Conch.app.tar.gz"
    },
    "windows-x86_64": {
      "signature": "<base64 sig from .sig file>",
      "url": "https://github.com/an0nn30/conch/releases/download/v2.1.0/Conch-v2.1.0-setup.exe"
    }
  }
}
```

Both `darwin-aarch64` and `darwin-x86_64` point to the same universal binary `.tar.gz`.

## Platform Notes

- **Linux excluded:** Linux users install via deb/rpm and update through their package manager. The updater check is skipped on Linux (`#[cfg(not(target_os = "linux"))]`).
- **macOS universal binary:** `cargo tauri build --target universal-apple-darwin` produces a single universal `.app` bundle and update artifact.
- **Windows:** Tauri's NSIS bundler produces the installer and update artifact. The existing custom WiX pipeline is replaced.
- **Rate limiting:** The `latest.json` endpoint is a static file served from GitHub Releases (not the GitHub API), so standard API rate limits do not apply.

## Testing

- Unit test: `check_for_updates` config default is `true`, serialization round-trip
- Unit test: `UpdateInfo` serialization round-trip
- Unit test: `PendingUpdate` managed state — store and retrieve
- Manual test: startup check shows toast when update available
- Manual test: "Check for Updates" menu item works
- Manual test: download progress shown, restart prompt appears
- Manual test: "Restart Later" dismisses, "Restart Now" relaunches
- Manual test: disabling setting suppresses startup check
- Manual test: no errors when offline or GitHub unreachable

## Documentation

Update `README.md`:
- Add **Auto-Updates** to the Features section: built-in update checking on macOS and Windows, configurable startup check, manual check via menu
- Add `check_for_updates` to the Configuration TOML example

## Out of Scope

- Apple notarization (follow-up, uses the developer account)
- Windows code signing (follow-up, requires EV certificate)
- Linux auto-updates
- Delta/incremental updates
- Update channels (stable/beta)
