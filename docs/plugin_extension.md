# Plugin System Extension: Panel Plugins

## Overview

Extend the plugin system so Lua plugins can register persistent UI panels
in the sidebar, not just run-once actions. Panel plugins describe their UI
declaratively and the Rust side renders it with egui.

---

## Phase 1: Sidebar Panels (complete)

**Status:** Fully implemented.

Panel plugins declare `plugin-type: panel` and get their own sidebar tab with
a declarative widget set and a periodic refresh loop.

### Features implemented

- Plugin header metadata: `plugin-name`, `plugin-description`, `plugin-version`, `plugin-type`, `plugin-icon`, `plugin-keybind`
- Lifecycle: `setup()`, `render()`, `on_click(button_id)`, `on_keybind(action)`
- Declarative widget API: heading, text, label, separator, table, progress, button, key-value
- Silent command execution via separate SSH channels / local subprocesses
- Platform detection (`session.platform()`)
- Custom plugin icons with validation (extension, size, magic bytes, decode)
- Plugin keybindings with priority resolution and config overrides
- Event polling: buttons and keybinds dispatched to plugin handlers between render cycles
- Five API modules: `session`, `app`, `ui`, `crypto`, `net`
- Networking API: TCP port scanning, DNS resolution, timing
- Load/unload persistence in `state.toml`
- Three example plugins: System Info, Port Scanner, Encrypt/Decrypt

---

## Phase 2: Interactive Widgets

**Goal:** Add interactive widgets to panels — combo selectors, text inputs,
toggle switches — with event callbacks flowing back to the plugin.

### Additional widgets

| Function | Description |
|----------|-------------|
| `ui.panel_combo(id, label, options, default)` | Dropdown selector |
| `ui.panel_text_input(id, label, default)` | Editable text field |
| `ui.panel_toggle(id, label, default)` | On/off toggle |
| `ui.panel_color(label, r, g, b)` | Colored status indicator |

### Callback model

```lua
function on_change(widget_id, value)
    -- Called when a combo, text_input, or toggle changes
end
```

Widget state is stored Rust-side so the panel retains interactive state
between render cycles. Changes are sent back via
`PluginCommand::PanelWidgetChanged { id, value }`.

---

## Phase 3: Bottom Panels (complete)

**Status:** Fully implemented.

Bottom panel plugins render in a resizable area below the terminal, ideal for
log tailing, container monitoring, build output, and other wide-format content.

### Features implemented

- Plugin type declaration: `-- plugin-type: bottom-panel`
- `PluginType::BottomPanel` variant — same lifecycle as sidebar panels (`setup`, `render`, `on_click`, `on_keybind`)
- Tabbed bottom panel area with multiple concurrent panels
- Resizable height (drag handle, range 80–500px)
- Collapsible via View menu toggle or close button
- Height and collapsed state persisted in `state.toml`
- `ui.panel_scroll_text(lines)` widget — scrollable monospace area that auto-scrolls to bottom
- "bottom" badge in the Plugins sidebar list
- Shared state infrastructure: bottom panels reuse `panel_widgets`, `panel_names`, `panel_button_events`, `panel_event_waiters` (keyed by plugin index)
- Activate/deactivate on load/unload with persistence
- Keybinding support: `open_panel` action shows/focuses the bottom panel

### Architecture

- `BottomPanel` is a separate `PluginType`, not a placement modifier — simpler to reason about
- Bottom panel area uses `egui::TopBottomPanel::bottom()` inserted before `CentralPanel`
- Tab bar at top of the panel strip, content rendered below
- No drag-between-positions yet (deferred to future work)
