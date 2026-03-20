# Settings Dialog — Design Spec

## Overview

Add a comprehensive Settings dialog to Conch, replacing the need to hand-edit `config.toml`. The dialog follows an IntelliJ-style layout: a grouped navigation sidebar on the left with a content area on the right. It absorbs the existing Plugin Manager dialog.

## Entry Point

- **Menu item:** "Settings..." in the App menu on macOS (after the About separator), or File menu on other platforms
- **Accelerator:** `CmdOrCtrl+,`
- **Menu constants:** `MENU_SETTINGS_ID = "app.settings"`, `MENU_ACTION_SETTINGS = "settings"`
- **Removes:** "Plugin Manager..." from the Tools menu (absorbed into Plugins section)
- **Toggle behavior:** If the dialog is already open when triggered, close it

## Dialog Chrome

- **Presentation:** Full overlay on top of app content, using the existing `ssh-overlay` / `ssh-form` CSS pattern
- **Escape handler:** Capture-phase keydown listener (`addEventListener('keydown', handler, true)`) so it fires before xterm.js
- **Footer:** Apply and Cancel buttons
  - **Apply:** Writes all pending changes to `~/.config/conch/config.toml` at once. Hot-reloadable settings take effect immediately. If any changed settings require restart, shows a toast notification: "Some changes require a restart to take effect."
  - **Cancel:** Discards all pending changes and closes the dialog
  - **Escape:** Same as Cancel

## Sidebar Structure

Grouped navigation with category headers and indented section items.

```
General
  Appearance
  Keyboard Shortcuts
Editor
  Terminal
  Shell
  Cursor
Extensions
  Plugins
Advanced
```

Clicking a section highlights it and swaps the content area. The first section (Appearance) is selected by default on open.

## Section Content

### Appearance

| Setting | Control | Config Path | Default |
|---------|---------|-------------|---------|
| Theme | Dropdown (lists available themes — see `list_themes`) | `colors.theme` | `"dracula"` |
| Appearance Mode | Toggle group: Dark / Light / System | `colors.appearance_mode` | `"dark"` |
| Window Decorations | Dropdown: Full / Transparent / Buttonless / None | `window.decorations` | `"Full"` |
| Native Menu Bar | Toggle switch (macOS only — hidden on other platforms) | `conch.ui.native_menu_bar` | `true` |
| UI Font Family | Text input | `conch.ui.font_family` | `""` (system default) |
| UI Font Size | Numeric input (overall UI text size) | `conch.ui.font_size` | `13.0` |

**Sub-groups:** "Color Theme", "Window", "UI Font" — separated by dividers. The "Window" sub-group contains both Decorations and Native Menu Bar.

### Keyboard Shortcuts

| Setting | Control | Config Path | Default |
|---------|---------|-------------|---------|
| New Tab | Shortcut recorder | `conch.keyboard.new_tab` | `"cmd+t"` |
| Close Tab | Shortcut recorder | `conch.keyboard.close_tab` | `"cmd+w"` |
| New Window | Shortcut recorder | `conch.keyboard.new_window` | `"cmd+shift+n"` |
| Quit | Shortcut recorder | `conch.keyboard.quit` | `"cmd+q"` |
| Zen Mode | Shortcut recorder | `conch.keyboard.zen_mode` | `"cmd+shift+z"` |
| Toggle File Explorer | Shortcut recorder | `conch.keyboard.toggle_left_panel` | `"cmd+shift+e"` |
| Toggle Sessions Panel | Shortcut recorder | `conch.keyboard.toggle_right_panel` | `"cmd+shift+r"` |
| Toggle Bottom Panel | Shortcut recorder | `conch.keyboard.toggle_bottom_panel` | `"cmd+shift+j"` |

**Sub-groups:** "Tab & Window", "View" — separated by dividers.

**Shortcut recorder behavior:** Click the shortcut field to enter recording mode (field highlights, shows "Press keys..."). The next key combination pressed is captured and displayed. Press Escape to cancel recording without changing the value. Shortcut conflict detection is out of scope for this iteration.

**Key format normalization:** Browser key events are normalized to the config format: `event.metaKey` → `cmd` (the physical Cmd key on macOS), `event.ctrlKey` → `ctrl`, `event.shiftKey` → `shift`, `event.altKey` → `alt`. The key name uses `event.key.toLowerCase()`. Parts are joined with `+` in order: modifiers first (cmd, ctrl, shift, alt), then key. On macOS, users typically bind `cmd+` shortcuts; `ctrl+` shortcuts are also recordable for users who want them. The `config_key_to_accelerator` function maps both `cmd` and `ctrl` to `CmdOrCtrl` at the menu level, but the config preserves the user's original choice.

### Terminal

| Setting | Control | Config Path | Default |
|---------|---------|-------------|---------|
| Font Family | Text input | `terminal.font.normal.family` | `"JetBrains Mono"` |
| Font Size | Numeric input | `terminal.font.size` | `14.0` |
| Font Offset X | Numeric input | `terminal.font.offset.x` | `0.0` |
| Font Offset Y | Numeric input | `terminal.font.offset.y` | `0.0` |
| Scroll Sensitivity | Numeric input (0.0–1.0) | `terminal.scroll_sensitivity` | `0.15` |

**Sub-groups:** "Font", "Scrolling" — separated by dividers.

### Shell

| Setting | Control | Config Path | Default |
|---------|---------|-------------|---------|
| Shell Program | Text input | `terminal.shell.program` | `""` (uses $SHELL) |
| Arguments | Comma-separated text input | `terminal.shell.args` | `[]` |
| Environment Variables | Key-value list with add/remove | `terminal.env` | `{}` |

**Sub-groups:** "Program", "Environment Variables" — separated by dividers.

**Shell arguments:** Displayed and edited as a comma-separated string (e.g., `-l, -c, echo ok`). Parsed by splitting on commas and trimming whitespace. Empty string = empty array. Limitation: arguments containing literal commas are not supported through this UI — users needing that can edit `config.toml` directly.

**Environment variable editor:** Each row has a key input, "=" label, value input, and an X button to remove. An "+ Add Variable" button appends a new empty row. A note below: "TERM and COLORTERM are always set to xterm-256color and truecolor."

### Cursor

| Setting | Control | Config Path | Default |
|---------|---------|-------------|---------|
| Shape | Toggle group: Block / Underline / Beam | `terminal.cursor.style.shape` | `"Block"` |
| Blinking | Toggle switch | `terminal.cursor.style.blinking` | `true` |
| Vi Mode Shape | Toggle group: Block / Underline / Beam (optional, can be unset) | `terminal.cursor.vi_mode_style.shape` | unset |
| Vi Mode Blinking | Toggle switch | `terminal.cursor.vi_mode_style.blinking` | unset |

**Sub-groups:** "Style", "Vi Mode Override" — separated by dividers. Vi mode section has a note: "Optional cursor style when vi mode is active in your shell."

**Vi mode unset state:** The Rust type is `Option<CursorStyleConfig>`, serialized as `null` in JSON when unset. The frontend toggle group includes a "None" option (selected by default) to represent the unset state. Selecting Block/Underline/Beam sets the override; selecting "None" clears it back to `null`.

### Plugins

| Setting | Control | Config Path | Default |
|---------|---------|-------------|---------|
| Enable Plugins | Toggle switch | `conch.plugins.enabled` | `true` |
| Lua Plugins | Toggle switch | `conch.plugins.lua` | `true` |
| Java Plugins | Toggle switch | `conch.plugins.java` | `true` |
| Search Paths | Path list with add/remove | `conch.plugins.search_paths` | `[]` |

**Sub-groups:** "Plugin System", "Plugin Types", "Search Paths", "Installed Plugins" — separated by dividers.

Note: The `conch.plugins.native` config field exists but native plugin loading is not implemented (only Lua and Java are supported). The toggle is omitted from the UI to avoid confusion.

**Installed Plugins list:** Shows all discovered plugins with:
- Type badge (Lua/Java, color-coded)
- Plugin name (bold)
- Version and file path (secondary text)
- Enable/Disable button per plugin

A "Rescan" button above the list triggers plugin rediscovery. Plugin enabled/disabled state is persisted in `state.toml` (`loaded_plugins`) as it is today.

### Advanced

| Setting | Control | Config Path | Default |
|---------|---------|-------------|---------|
| Window Columns | Numeric input | `window.dimensions.columns` | `150` |
| Window Lines | Numeric input | `window.dimensions.lines` | `50` |
| UI Font Size (Small) | Numeric input | `conch.ui.font.small` | `12.0` |
| UI Font Size (List) | Numeric input | `conch.ui.font.list` | `14.0` |
| UI Font Size (Normal) | Numeric input | `conch.ui.font.normal` | `14.0` |

**Sub-groups:** "Initial Window Size", "UI Chrome Font Sizes" — separated by dividers. Each has descriptive secondary text explaining what the values control.

Note: The `conch.ui.font.*` sizes (small/list/normal) control individual UI element categories and are distinct from the overall `conch.ui.font_size` in the Appearance section, which sets the base font size for the UI.

## Hot-Reload vs Restart Required

Settings are classified into two categories:

**Hot-reloadable (apply immediately on Apply):**
- `colors.theme`
- `colors.appearance_mode`
- `conch.ui.font_family`
- `conch.ui.font_size`
- `conch.ui.native_menu_bar`
- `conch.ui.font.small`, `conch.ui.font.list`, `conch.ui.font.normal`
- All `conch.keyboard.*` shortcuts (menu is rebuilt)

**Restart required:**
- `window.decorations`
- `window.dimensions.columns`, `window.dimensions.lines`
- All `terminal.*` settings (font, offset, scroll sensitivity)
- All `terminal.shell.*` settings (program, args)
- All `terminal.env.*` settings
- All `terminal.cursor.*` settings
- All `conch.plugins.*` settings (enabled, type toggles, search paths)

When Apply is clicked and any restart-required setting was changed, show a toast: "Some changes require a restart to take effect."

## UI Styling

- Follows existing CSS custom property theming (`var(--bg)`, `var(--fg)`, etc.)
- Dropdown arrows positioned flush-right inside the dropdown box
- Toggle groups use the segmented button pattern (adjacent rounded buttons, active one highlighted)
- Toggle switches for boolean values
- Text/numeric inputs use the standard `ssh-form` input styling
- Overlay escape handler uses capture phase (`addEventListener('keydown', handler, true)`)

## Data Flow

1. **On open:** Read current `config.toml` values via `get_all_settings`. Also call `list_themes` to populate the theme dropdown, and `scan_plugins` to populate the plugin list. Populate all fields.
2. **While editing:** Track changes in-memory in the frontend. No writes until Apply.
3. **On Apply:** Send the full settings object to `save_settings`. The backend:
   a. Loads the current config from disk for comparison
   b. Diffs old vs new config to determine if any restart-required field changed
   c. Updates the in-memory `TauriState.config` (under the mutex lock)
   d. Writes the new config to `config.toml` (omitting the legacy `[font]` section — see Legacy Font Handling below)
   e. Emits `config-changed` event directly (rather than waiting for the 2-second watcher poll) to trigger immediate hot-reload of applicable settings
   f. Returns `{ restart_required: bool }`

Note: In-memory update (step c) happens before disk write (step d) to avoid a race where the 2-second watcher detects the disk change and causes the frontend to re-query stale in-memory data.
4. **On Cancel / Escape:** Discard in-memory changes, close overlay.

## Tauri Commands

### New Commands

- `get_all_settings() -> UserConfig` — Returns the current `UserConfig` as its natural nested JSON structure (matching the serde serialization of the Rust struct). The frontend navigates the nested object to populate fields.
- `save_settings(settings: UserConfig) -> SaveResult` — Diffs incoming settings against current config on disk, writes to `config.toml`, updates in-memory state, emits `config-changed`, returns `{ restart_required: bool }`.
- `list_themes() -> Vec<String>` — Lists available theme names from the themes directory. Always includes the built-in `"dracula"` theme even if no `dracula.toml` file exists (it is hardcoded as a fallback).

### Reused Commands

- `scan_plugins` — Already returns `Vec<DiscoveredPlugin>` with name, version, type, source, path, and loaded status. Reused as-is for the Installed Plugins list.

## Legacy Font Handling

The `UserConfig` struct has a top-level `font: FontConfig` field (the legacy `[font]` section) alongside `terminal.font`. When `save_settings` writes the config, it must suppress the legacy `[font]` section to avoid confusion. Approach: add `#[serde(skip_serializing)]` to the `font` field on `UserConfig`. This preserves the ability to *read* legacy configs (for backward compatibility via `resolved_terminal_font()`) while ensuring new saves only write `[terminal.font]`.

## In-Memory Config Staleness

`TauriState` stores a `UserConfig` loaded once at startup. After `save_settings` writes to disk, this snapshot becomes stale. To address this:

- `save_settings` updates `TauriState.config` in-place after writing. This requires changing `TauriState.config` from `UserConfig` to `parking_lot::Mutex<UserConfig>` (matching the existing `parking_lot` usage in the codebase).
- This is a cross-cutting change: all existing commands that read `state.config` (`spawn_shell`, `get_app_config`, `get_theme_colors`, `get_terminal_config`, `get_keyboard_shortcuts`) will need to acquire the lock. These are all short reads, so contention is negligible.
- `get_all_settings` also reads from this mutex (not from disk) for consistency.
- The `config-changed` event is emitted directly by `save_settings` so the frontend can re-query as needed.

## New Frontend Files

- `frontend/settings.js` — Settings dialog IIFE, exposes `window.settings.open()` / `window.settings.close()`
- Settings CSS added to `index.html` inline styles (following existing pattern)

## Menu Changes

- **macOS:** Add "Settings..." menu item with `CmdOrCtrl+Comma` accelerator to the App menu, after the About item and separator
- **Other platforms:** Add "Settings..." to the File menu
- Remove "Plugin Manager..." from the Tools menu
- Add constants: `MENU_SETTINGS_ID`, `MENU_ACTION_SETTINGS`
- Add handler in `on_menu_event` match arm
- Update `build_app_menu_with_plugins` to include Settings in the rebuilt menu (to avoid the same drift bug that affected zoom)

## Out of Scope

- Settings search/filter (stretch goal for later — no stubbing)
- Theme preview/live preview while browsing themes
- Font picker (system font dialog)
- Import/export settings
- Per-profile settings
- Shortcut conflict detection (may add later)
