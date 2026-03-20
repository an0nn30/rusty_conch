# Settings Dialog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an IntelliJ-style Settings dialog that exposes all `config.toml` options in a GUI, absorbing the Plugin Manager dialog.

**Architecture:** A new `settings.js` frontend IIFE renders a sidebar+content overlay dialog. Three new Tauri commands (`get_all_settings`, `save_settings`, `list_themes`) handle config I/O. `TauriState.config` becomes `Mutex<UserConfig>` so the backend can update config in-memory after save.

**Tech Stack:** Rust (Tauri v2 commands, serde), JavaScript (vanilla IIFE, DOM), CSS custom properties, TOML serialization.

**Spec:** `docs/superpowers/specs/2026-03-20-settings-dialog-design.md`

---

## File Map

### New Files
- `crates/conch_tauri/frontend/settings.js` — Settings dialog IIFE (`window.settings`)
- `crates/conch_tauri/src/settings.rs` — Settings Tauri commands and `needs_restart` logic (keeps `lib.rs` under ~1000 lines)

### Modified Files
- `crates/conch_core/src/config/mod.rs` — Add `#[serde(skip_serializing)]` on legacy `font` field
- `crates/conch_tauri/src/lib.rs` — `TauriState` mutex change, `mod settings`, menu items, event handlers
- `crates/conch_tauri/frontend/index.html` — Script tag, CSS, menu action handler, settings init
- `crates/conch_tauri/frontend/titlebar.js` — Add Settings to Windows custom titlebar menu

---

## Task 1: Legacy Font Serialization Guard

**Files:**
- Modify: `crates/conch_core/src/config/mod.rs` (line 43, the `font` field on `UserConfig`)
- Test: `crates/conch_core/src/config/mod.rs` (existing test module)

- [ ] **Step 1: Write failing test**

Add to the `#[cfg(test)] mod tests` in `crates/conch_core/src/config/mod.rs`:

```rust
#[test]
fn serialized_config_omits_legacy_font_section() {
    let config = UserConfig::default();
    let toml_str = toml::to_string_pretty(&config).unwrap();
    // The legacy [font] section should not appear in serialized output.
    // Only [terminal.font] should be present.
    assert!(
        !toml_str.contains("\n[font]\n"),
        "Legacy [font] section should not appear in serialized output, got:\n{toml_str}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p conch_core serialized_config_omits_legacy_font`
Expected: FAIL — the default serialization includes `[font]`.

- [ ] **Step 3: Add skip_serializing to the font field**

In `crates/conch_core/src/config/mod.rs`, on the `UserConfig` struct, add the attribute to the `font` field:

```rust
#[serde(skip_serializing)]
pub font: FontConfig,
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p conch_core`
Expected: All tests pass, including the new one.

- [ ] **Step 5: Commit**

```
git add crates/conch_core/src/config/mod.rs
git commit -m "Suppress legacy [font] section in config serialization"
```

---

## Task 2: Make TauriState.config Mutable via Mutex

**Files:**
- Modify: `crates/conch_tauri/src/lib.rs` (lines 68-71 — `TauriState` struct, plus every command that reads `state.config`)

This is a mechanical refactor: wrap `config` in `parking_lot::Mutex`, update all readers to lock.

- [ ] **Step 1: Change TauriState struct**

In `crates/conch_tauri/src/lib.rs`, change:

```rust
struct TauriState {
    ptys: Arc<Mutex<HashMap<String, PtyBackend>>>,
    config: Mutex<UserConfig>,
}
```

- [ ] **Step 2: Update TauriState construction in `run()`**

Find where `TauriState` is constructed (in `.manage(TauriState { ... })`), change:

```rust
.manage(TauriState {
    ptys: Arc::new(Mutex::new(HashMap::new())),
    config: Mutex::new(config),
})
```

- [ ] **Step 3: Update all commands that read state.config**

For each command that accesses `state.config`, add `.lock()`:

- `spawn_shell` — `let cfg = state.config.lock();` then use `cfg.terminal.shell`, `cfg.terminal.env`
- `get_app_config` — `let cfg = state.config.lock();` then use `cfg.window.decorations`, etc.
- `get_theme_colors` — `let cfg = state.config.lock();` then pass `&cfg` to `theme::resolve_theme_colors`
- `get_terminal_config` — `let cfg = state.config.lock();` then use `cfg.resolved_terminal_font()`, etc.
- `get_keyboard_shortcuts` — `let cfg = state.config.lock();` then use `cfg.conch.keyboard`
- `resolved_shell` usage in `spawn_shell`

For each, the pattern is: replace `state.config.xxx` with `{ let cfg = state.config.lock(); cfg.xxx }` or bind `let cfg = state.config.lock();` at the top of the function.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p conch_tauri`
Expected: Compiles with no new errors.

- [ ] **Step 5: Run all tests**

Run: `cargo test --workspace`
Expected: All 210+ tests pass.

- [ ] **Step 6: Commit**

```
git add crates/conch_tauri/src/lib.rs
git commit -m "Wrap TauriState.config in Mutex for runtime mutability"
```

---

## Task 3: New Tauri Commands (get_all_settings, save_settings, list_themes)

**Files:**
- Create: `crates/conch_tauri/src/settings.rs` — settings commands and `needs_restart` logic
- Modify: `crates/conch_tauri/src/lib.rs` — add `mod settings;`, make `TauriState` and helpers `pub(crate)`, register commands

Extracting to a separate module keeps `lib.rs` under ~1000 lines per CLAUDE.md.

- [ ] **Step 1: Create `settings.rs` with commands and needs_restart**

Create `crates/conch_tauri/src/settings.rs`:

```rust
//! Settings dialog Tauri commands.

use conch_core::config::{self, UserConfig};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::Emitter;

use crate::TauriState;

#[derive(Serialize)]
pub(crate) struct SaveSettingsResult {
    restart_required: bool,
}

#[tauri::command]
pub(crate) fn get_all_settings(state: tauri::State<'_, TauriState>) -> serde_json::Value {
    let cfg = state.config.lock();
    serde_json::to_value(&*cfg).unwrap_or_default()
}

#[tauri::command]
pub(crate) fn list_themes() -> Vec<String> {
    let mut themes: Vec<String> = conch_core::color_scheme::list_themes()
        .keys()
        .cloned()
        .collect();
    if !themes.iter().any(|t| t == "dracula") {
        themes.push("dracula".into());
    }
    themes.sort();
    themes
}

#[tauri::command]
pub(crate) fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, TauriState>,
    settings: serde_json::Value,
) -> Result<SaveSettingsResult, String> {
    let new_config: UserConfig =
        serde_json::from_value(settings).map_err(|e| format!("Invalid settings: {e}"))?;

    let restart_required = {
        let old_config = state.config.lock();
        needs_restart(&old_config, &new_config)
    };

    // Update in-memory config first (before disk write) to avoid watcher race.
    {
        let mut cfg = state.config.lock();
        *cfg = new_config.clone();
    }

    config::save_user_config(&new_config).map_err(|e| format!("Failed to save config: {e}"))?;

    let _ = app.emit("config-changed", ());

    // Rebuild menu to pick up keyboard shortcut changes.
    let kb = &new_config.conch.keyboard;
    if let Ok(menu) = crate::build_app_menu(&app, kb) {
        let _ = app.set_menu(menu);
    }

    Ok(SaveSettingsResult { restart_required })
}

/// Compare two configs and return true if any restart-required field differs.
pub(crate) fn needs_restart(old: &UserConfig, new: &UserConfig) -> bool {
    // Window
    if old.window.decorations != new.window.decorations { return true; }
    if old.window.dimensions.columns != new.window.dimensions.columns { return true; }
    if old.window.dimensions.lines != new.window.dimensions.lines { return true; }

    // Terminal font
    let old_font = old.resolved_terminal_font();
    let new_font = new.resolved_terminal_font();
    if old_font.normal.family != new_font.normal.family { return true; }
    if old_font.size != new_font.size { return true; }
    if old_font.offset.x != new_font.offset.x { return true; }
    if old_font.offset.y != new_font.offset.y { return true; }
    if old.terminal.scroll_sensitivity != new.terminal.scroll_sensitivity { return true; }

    // Shell
    if old.terminal.shell.program != new.terminal.shell.program { return true; }
    if old.terminal.shell.args != new.terminal.shell.args { return true; }
    if old.terminal.env != new.terminal.env { return true; }

    // Cursor
    if old.terminal.cursor != new.terminal.cursor { return true; }

    // Plugins
    if old.conch.plugins.enabled != new.conch.plugins.enabled { return true; }
    if old.conch.plugins.lua != new.conch.plugins.lua { return true; }
    if old.conch.plugins.java != new.conch.plugins.java { return true; }
    if old.conch.plugins.search_paths != new.conch.plugins.search_paths { return true; }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_configs_no_restart() {
        let a = UserConfig::default();
        let b = UserConfig::default();
        assert!(!needs_restart(&a, &b));
    }

    #[test]
    fn changed_decorations_needs_restart() {
        let a = UserConfig::default();
        let mut b = UserConfig::default();
        b.window.decorations = conch_core::config::WindowDecorations::None;
        assert!(needs_restart(&a, &b));
    }

    #[test]
    fn changed_theme_no_restart() {
        let a = UserConfig::default();
        let mut b = UserConfig::default();
        b.colors.theme = "monokai".into();
        assert!(!needs_restart(&a, &b), "Theme is hot-reloadable, should not require restart");
    }

    #[test]
    fn changed_terminal_font_needs_restart() {
        let a = UserConfig::default();
        let mut b = UserConfig::default();
        b.terminal.font.size = 18.0;
        assert!(needs_restart(&a, &b));
    }

    #[test]
    fn changed_shell_program_needs_restart() {
        let a = UserConfig::default();
        let mut b = UserConfig::default();
        b.terminal.shell.program = "/bin/bash".into();
        assert!(needs_restart(&a, &b));
    }

    #[test]
    fn changed_keyboard_shortcut_no_restart() {
        let a = UserConfig::default();
        let mut b = UserConfig::default();
        b.conch.keyboard.new_tab = "ctrl+n".into();
        assert!(!needs_restart(&a, &b), "Keyboard shortcuts are hot-reloadable");
    }

    #[test]
    fn changed_plugin_enabled_needs_restart() {
        let a = UserConfig::default();
        let mut b = UserConfig::default();
        b.conch.plugins.enabled = false;
        assert!(needs_restart(&a, &b));
    }
}
```

- [ ] **Step 2: Add `mod settings;` and make helpers accessible in lib.rs**

In `crates/conch_tauri/src/lib.rs`, add:

```rust
pub(crate) mod settings;
```

Make `TauriState` and `build_app_menu` accessible to the settings module:
- Change `struct TauriState` to `pub(crate) struct TauriState`
- Change `fn build_app_menu` to `pub(crate) fn build_app_menu`

- [ ] **Step 3: Register commands in invoke_handler**

Add to the `tauri::generate_handler![]` list:

```rust
settings::get_all_settings,
settings::save_settings,
settings::list_themes,
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p conch_tauri`
Expected: Compiles cleanly.

- [ ] **Step 5: Run all tests (including new needs_restart tests)**

Run: `cargo test --workspace`
Expected: All tests pass, including 7 new `needs_restart` tests.

- [ ] **Step 6: Commit**

```
git add crates/conch_tauri/src/settings.rs crates/conch_tauri/src/lib.rs
git commit -m "Add settings Tauri commands in dedicated module with needs_restart tests"
```

---

## Task 4: Add Settings Menu Item

**Files:**
- Modify: `crates/conch_tauri/src/lib.rs` — menu constants, `build_app_menu`, `build_app_menu_with_plugins`, `on_menu_event`
- Modify: `crates/conch_tauri/frontend/titlebar.js` — Windows custom titlebar menu def
- Modify: `crates/conch_tauri/frontend/index.html` — `handleMenuAction` handler

- [ ] **Step 1: Add menu constants**

Add after the existing `MENU_ACTION_MANAGE_TUNNELS` constant:

```rust
const MENU_SETTINGS_ID: &str = "app.settings";
const MENU_ACTION_SETTINGS: &str = "settings";
```

- [ ] **Step 2: Add Settings to `build_app_menu`**

In `build_app_menu`, add the Settings menu item. On macOS, add it to the App menu (after About + separator). On other platforms, add it to the File menu.

In the `#[cfg(target_os = "macos")]` block inside `build_app_menu`, add the Settings item to the app menu:

```rust
let settings = MenuItem::with_id(app, MENU_SETTINGS_ID, "Settings\u{2026}", true, Some("CmdOrCtrl+Comma"))?;
```

Add it to the macOS app menu items (after the About separator):

```rust
let app_menu = Submenu::with_items(app, app_name, true, &[
    &PredefinedMenuItem::about(app, None, None)?,
    &PredefinedMenuItem::separator(app)?,
    &settings,
    &PredefinedMenuItem::separator(app)?,
    &PredefinedMenuItem::hide(app, None)?,
    &PredefinedMenuItem::hide_others(app, None)?,
    &PredefinedMenuItem::separator(app)?,
    &PredefinedMenuItem::quit(app, None)?,
])?;
```

For `#[cfg(not(target_os = "macos"))]`, add Settings to the File menu.

- [ ] **Step 3: Add Settings to `build_app_menu_with_plugins`**

Mirror the same changes in the plugin-rebuilt menu function to avoid the drift bug. Add the Settings menu item in the same position (macOS app menu / other platforms file menu).

- [ ] **Step 4: Add menu event handler**

In the `on_menu_event` match, add:

```rust
MENU_SETTINGS_ID => emit_menu_action_to_focused_window(app, MENU_ACTION_SETTINGS),
```

- [ ] **Step 5: Remove Plugin Manager menu item**

Remove the `MENU_PLUGIN_MANAGER_ID` menu item creation from both `build_app_menu` and `build_app_menu_with_plugins`. Remove its `on_menu_event` match arm. Keep the constants for now (they may be referenced elsewhere); remove them in a cleanup pass if unused.

- [ ] **Step 6: Add handleMenuAction handler in index.html**

In `handleMenuAction` in `index.html`, add:

```javascript
if (action === 'settings') {
  if (window.settings) window.settings.open();
  return;
}
```

Remove the `plugin-manager` action handler.

- [ ] **Step 7: Update Windows titlebar menu in titlebar.js**

In `titlebar.js`, in the Tools menu definition, replace the Plugin Manager entry with Settings:

```javascript
{ id: 'settings', label: 'Settings\u2026', shortcut: `${ctrl}+,` },
```

Remove the Plugin Manager entry.

- [ ] **Step 8: Verify compilation**

Run: `cargo check -p conch_tauri`
Expected: Compiles cleanly.

- [ ] **Step 9: Commit**

```
git add crates/conch_tauri/src/lib.rs crates/conch_tauri/frontend/index.html crates/conch_tauri/frontend/titlebar.js
git commit -m "Add Settings menu item, remove Plugin Manager menu entry"
```

---

## Task 5: Settings Dialog — Frontend Shell (Sidebar + Overlay)

**Files:**
- Create: `crates/conch_tauri/frontend/settings.js`
- Modify: `crates/conch_tauri/frontend/index.html` — add script tag, CSS styles, init call

This task creates the dialog skeleton: overlay, sidebar navigation, content area switching, Apply/Cancel buttons. Content sections are empty placeholders — filled in subsequent tasks.

- [ ] **Step 1: Add CSS to index.html**

Add Settings dialog CSS in the `<style>` block in `index.html`. Use CSS custom properties for all colors. Key classes: `.settings-overlay`, `.settings-dialog`, `.settings-sidebar`, `.settings-sidebar-group`, `.settings-sidebar-item`, `.settings-content`, `.settings-footer`, `.settings-row`, `.settings-label`, `.settings-input`, `.settings-select`, `.settings-toggle-group`, `.settings-toggle`, `.settings-switch`, `.settings-divider`, `.settings-group-label`.

- [ ] **Step 2: Create settings.js with IIFE skeleton**

Create `crates/conch_tauri/frontend/settings.js` following the plugin-manager.js IIFE pattern:

```javascript
(function (exports) {
  'use strict';

  let invoke = null;
  let listenFn = null;
  let currentSection = 'appearance';
  let pendingSettings = null;   // In-memory copy of settings being edited
  let originalSettings = null;  // Snapshot for diffing on cancel

  function init(opts) {
    invoke = opts.invoke;
    listenFn = opts.listen;
  }

  async function open() {
    // Toggle: if already open, close
    if (document.getElementById('settings-overlay')) { close(); return; }

    // Load current settings + themes + plugins
    const [settings, themes, plugins] = await Promise.all([
      invoke('get_all_settings'),
      invoke('list_themes'),
      invoke('scan_plugins'),
    ]);

    originalSettings = JSON.parse(JSON.stringify(settings));
    pendingSettings = JSON.parse(JSON.stringify(settings));

    renderDialog(themes, plugins);
  }

  function close() {
    const el = document.getElementById('settings-overlay');
    if (el) el.remove();
    pendingSettings = null;
    originalSettings = null;
  }

  function renderDialog(themes, plugins) {
    // ... builds the full overlay DOM ...
    // Sidebar with grouped sections
    // Content area (swapped by selectSection)
    // Footer with Cancel + Apply
  }

  function selectSection(name) {
    currentSection = name;
    // Update sidebar active state
    // Swap content area
  }

  async function applySettings() {
    try {
      const result = await invoke('save_settings', { settings: pendingSettings });
      close();
      if (result.restart_required) {
        window.toast && window.toast.show('Some changes require a restart to take effect', 'info', 5000);
      }
    } catch (e) {
      window.toast && window.toast.show('Failed to save settings: ' + e, 'error');
    }
  }

  // Section renderers (one per section — implemented in Tasks 6-8)
  function renderAppearance(container, themes) { /* Task 6 */ }
  function renderKeyboard(container) { /* Task 6 */ }
  function renderTerminal(container) { /* Task 7 */ }
  function renderShell(container) { /* Task 7 */ }
  function renderCursor(container) { /* Task 7 */ }
  function renderPlugins(container, plugins) { /* Task 8 */ }
  function renderAdvanced(container) { /* Task 7 */ }

  exports.settings = { init, open, close };
})(window);
```

- [ ] **Step 3: Add script tag and init call in index.html**

Add the script tag after `plugin-manager.js`:

```html
<script src="settings.js"></script>
```

Add init call near the plugin manager init:

```javascript
if (window.settings) {
  window.settings.init({ invoke, listen: listenOnCurrentWindow });
}
```

- [ ] **Step 4: Add Escape handler in settings.js**

In `renderDialog`, add a capture-phase keydown listener. Store the reference so `close()` can remove it to avoid listener leaks:

```javascript
let escapeHandler = null;

function renderDialog(themes, plugins) {
  // ... build overlay DOM ...

  escapeHandler = function onKey(e) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  };
  document.addEventListener('keydown', escapeHandler, true);
}

function close() {
  const el = document.getElementById('settings-overlay');
  if (el) el.remove();
  if (escapeHandler) {
    document.removeEventListener('keydown', escapeHandler, true);
    escapeHandler = null;
  }
  pendingSettings = null;
  originalSettings = null;
}
```

- [ ] **Step 5: Verify the dialog opens and closes**

Run: `cargo tauri dev`
Press Cmd+, — the settings overlay should appear with sidebar and empty content. Cancel/Escape should close it.

- [ ] **Step 6: Commit**

```
git add crates/conch_tauri/frontend/settings.js crates/conch_tauri/frontend/index.html
git commit -m "Add Settings dialog shell with sidebar navigation and Apply/Cancel"
```

---

## Task 6: Appearance + Keyboard Sections

**Files:**
- Modify: `crates/conch_tauri/frontend/settings.js` — implement `renderAppearance` and `renderKeyboard`

- [ ] **Step 1: Implement renderAppearance**

Renders: Theme dropdown, Appearance Mode toggle group (Dark/Light/System), Window Decorations dropdown, Native Menu Bar toggle (macOS only — check `navigator.platform`), UI Font Family input, UI Font Size input.

Each control reads from `pendingSettings` and writes back on change. Dropdown arrows flush-right in the select box.

- [ ] **Step 2: Implement renderKeyboard**

Renders the 8 keyboard shortcuts in two sub-groups (Tab & Window, View). Each shortcut shows the current binding in a styled monospace box.

Implement the shortcut recorder: clicking a shortcut field enters recording mode (text changes to "Press keys...", background highlights). On the next keydown, capture the key combo, normalize to config format (`cmd+shift+z`), update `pendingSettings`, exit recording mode. Escape cancels recording.

- [ ] **Step 3: Manual test**

Run: `cargo tauri dev`
Open Settings, verify Appearance section shows current theme/mode/decorations/font. Change values — verify they update `pendingSettings` (not yet saved). Click Keyboard Shortcuts — verify shortcuts display and the recorder works.

- [ ] **Step 4: Commit**

```
git add crates/conch_tauri/frontend/settings.js
git commit -m "Implement Appearance and Keyboard Shortcuts settings sections"
```

---

## Task 7: Terminal, Shell, Cursor, and Advanced Sections

**Files:**
- Modify: `crates/conch_tauri/frontend/settings.js` — implement `renderTerminal`, `renderShell`, `renderCursor`, `renderAdvanced`

- [ ] **Step 1: Implement renderTerminal**

Renders: Font Family input, Font Size input, Font Offset X/Y inputs, Scroll Sensitivity input. Sub-groups: "Font", "Scrolling".

- [ ] **Step 2: Implement renderShell**

Renders: Shell Program input, Arguments input (comma-separated), Environment Variables key-value editor with add/remove rows. Note about TERM/COLORTERM below.

- [ ] **Step 3: Implement renderCursor**

Renders: Shape toggle group (Block/Underline/Beam), Blinking toggle switch. Vi Mode Override sub-group: Shape toggle group with "None" option, Blinking switch. Vi mode controls start with "None" selected if the config value is null.

- [ ] **Step 4: Implement renderAdvanced**

Renders: Window Columns/Lines inputs, UI Chrome Font Sizes (Small/List/Normal) inputs with descriptive sub-text.

- [ ] **Step 5: Manual test**

Run: `cargo tauri dev`
Open Settings, navigate each section. Verify all controls display current values and update pendingSettings.

- [ ] **Step 6: Commit**

```
git add crates/conch_tauri/frontend/settings.js
git commit -m "Implement Terminal, Shell, Cursor, and Advanced settings sections"
```

---

## Task 8: Plugins Section (Absorbing Plugin Manager)

**Files:**
- Modify: `crates/conch_tauri/frontend/settings.js` — implement `renderPlugins`
- Modify: `crates/conch_tauri/frontend/index.html` — remove plugin-manager.js script tag and init
- Delete: `crates/conch_tauri/frontend/plugin-manager.js` (functionality absorbed)

- [ ] **Step 1: Implement renderPlugins**

Renders:
- "Plugin System" sub-group: Enable Plugins master toggle
- "Plugin Types" sub-group: Lua Plugins toggle, Java Plugins toggle (with JVM note)
- "Search Paths" sub-group: Path list with add/remove, "+ Add Path" button
- "Installed Plugins" sub-group: Rescan button, list of discovered plugins with type badge, name, version, path, Enable/Disable button

The Rescan button calls `invoke('scan_plugins')` and refreshes the plugin list. Enable/Disable buttons call the existing `enable_plugin`/`disable_plugin` commands (or manipulate `pendingSettings` if plugin state is part of the settings save).

Note: Plugin enable/disable state is persisted in `state.toml` (not `config.toml`), so these actions take effect immediately via existing commands, independent of Apply/Cancel.

- [ ] **Step 2: Remove plugin-manager.js**

Remove the `<script src="plugin-manager.js"></script>` tag from `index.html`. Remove the `pluginManager.init(...)` call. Delete `plugin-manager.js`.

- [ ] **Step 3: Manual test**

Run: `cargo tauri dev`
Open Settings > Plugins. Verify toggles, search paths, and plugin list render correctly. Enable/disable a plugin — verify it works. Rescan — verify list refreshes.

- [ ] **Step 4: Commit**

```
git add crates/conch_tauri/frontend/settings.js crates/conch_tauri/frontend/index.html
git rm crates/conch_tauri/frontend/plugin-manager.js
git commit -m "Implement Plugins settings section and remove standalone Plugin Manager"
```

---

## Task 9: End-to-End Save Flow and Toast

**Files:**
- Modify: `crates/conch_tauri/frontend/settings.js` — wire up Apply to `save_settings`

- [ ] **Step 1: Wire Apply button to save_settings**

The `applySettings` function (already stubbed in Task 5) sends `pendingSettings` to the backend, closes the dialog, and shows a toast if `restart_required` is true.

Verify the full flow:
1. Open Settings
2. Change a hot-reloadable setting (e.g., theme) and a restart-required setting (e.g., terminal font)
3. Click Apply
4. Verify config.toml is updated
5. Verify the hot-reloadable setting took effect immediately
6. Verify the toast appears: "Some changes require a restart to take effect"
7. Reopen Settings — verify the saved values are shown

- [ ] **Step 2: Verify Cancel discards changes**

1. Open Settings
2. Change several values
3. Click Cancel (or press Escape)
4. Reopen Settings — verify original values are shown

- [ ] **Step 3: Commit**

```
git add crates/conch_tauri/frontend/settings.js
git commit -m "Wire Settings Apply/Cancel with save flow and restart-required toast"
```

---

## Task 10: Final Cleanup and Testing

**Files:**
- All modified files — review pass

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 2: Compile check**

Run: `cargo check -p conch_tauri`
Expected: No errors, no new warnings.

- [ ] **Step 3: Manual smoke test checklist**

1. Cmd+, opens Settings dialog
2. Cmd+, again toggles it closed
3. Escape closes the dialog
4. Each sidebar section shows correct current values
5. Changing theme + Apply → theme changes immediately
6. Changing terminal font + Apply → toast about restart
7. Cancel after changes → changes discarded
8. Plugins section: enable/disable works, rescan works
9. Keyboard shortcuts: recorder captures new bindings
10. New window (Cmd+Shift+N) → Settings menu works there too

- [ ] **Step 4: Push branch**

```
git push -u origin feat/settings-dialog
```
