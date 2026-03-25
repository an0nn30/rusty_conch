# Notification History & Bottom Panel Tabs Design Spec

## Goal

Add a tabbed bottom panel with a built-in Notifications tab that logs all toast notifications from the current session. Plugins can register additional bottom panel tabs alongside it.

## Scope

- Bottom panel tab infrastructure (built-in + plugin tabs)
- Built-in "Notifications" tab with in-memory history log
- Plugin bottom panel tabs via the existing SDK panel registration system
- No persistence across sessions — history resets on restart

## Bottom Panel Tab Infrastructure

### Tab Bar

The bottom panel header becomes a tab bar. The first tab is always "Notifications" (built-in, cannot be removed). Plugins that register with `panel_location: Bottom` get additional tabs appended after it.

```
[Notifications] [Plugin A] [Plugin B]
─────────────────────────────────────
│          active tab content        │
─────────────────────────────────────
```

- Clicking a tab switches the content area
- Active tab is visually highlighted (same `pw-tab-btn` / toggle styling used elsewhere)
- Only one tab's content is visible at a time
- If all plugin tabs are removed (plugins disabled), only "Notifications" remains

### Plugin Tab Lifecycle

When a plugin calls `register_panel(Bottom, name, icon)`:
1. The frontend receives a `plugin-widgets-updated` event (existing mechanism)
2. A new tab is created in the bottom panel tab bar with the plugin's name
3. The tab's content area renders the plugin's widget tree (same `renderWidgets()` from `plugin-widgets.js`)

When a plugin is disabled:
1. Its tab is removed from the tab bar
2. If that tab was active, switch to "Notifications"

### HTML Structure

Replace the current bottom panel content:

```html
<div id="bottom-panel" class="hidden">
  <div id="bottom-panel-header">
    <div id="bottom-panel-tabs"></div>
  </div>
  <div id="bottom-panel-content"></div>
</div>
```

The tab bar (`#bottom-panel-tabs`) contains tab buttons. The content area (`#bottom-panel-content`) shows the active tab's content.

### State Persistence

Add `bottom_panel_visible` to `SavedLayout` and persist it in `state.toml` alongside the existing panel visibility flags. The bottom panel height is already defined in CSS (150px) — persistence of custom height via resize handle is out of scope for now.

## Notification History Tab

### In-Memory Log

`toast.js` maintains an array of notification records:

```javascript
{ timestamp: Date, level: string, title: string, body: string }
```

Every call to `show()` (and the convenience methods) appends to this array before displaying the toast or sending a native notification. The array grows unbounded during a session — in practice, notifications are infrequent enough that memory is not a concern.

### Rendering

The Notifications tab content is a scrollable list of entries, newest first. Each entry shows:

- **Timestamp** — `HH:MM:SS` format, muted color
- **Level icon** — Small colored dot or the same SVG icon used in toasts
- **Title** — Bold, primary text color
- **Body** — Secondary text color, shown on the same line or below if present

Styling follows the existing panel patterns — `var(--ui-font-small)` for text, theme-aware colors, no hardcoded hex values.

### Live Updates

When the bottom panel is open and the Notifications tab is active, new entries appear at the top immediately. The tab content re-renders (or prepends the new entry) when `show()` is called.

### Clear Button

A small "Clear" button in the Notifications tab (top-right of the content area or in the tab bar) empties the log array and clears the rendered list.

### API Addition to `toast.js`

Export a `getHistory()` function that returns the log array, and an `onNotification(callback)` function for live updates:

```javascript
exports.toast = {
  show, showInApp, dismiss, configure,
  info, success, error, warn,
  getHistory,        // () => Array<{timestamp, level, title, body}>
  onNotification,    // (callback) => void — called on each new notification
  clearHistory,      // () => void — empties the log
};
```

## Files Changed

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `crates/conch_tauri/frontend/toast.js` | Add history array, getHistory, onNotification, clearHistory |
| Modify | `crates/conch_tauri/frontend/index.html` | Bottom panel tab bar HTML + CSS, notification history renderer, tab switching logic, bottom panel state persistence |
| Modify | `crates/conch_tauri/frontend/plugin-widgets.js` | Route bottom-panel plugin widgets to bottom panel tabs instead of a separate panel |
| Modify | `crates/conch_tauri/src/lib.rs` | Add `bottom_panel_visible` to SavedLayout |

## What's NOT Changing

- The toast display itself (position, styling, auto-dismiss, native notifications) — unchanged
- Plugin SDK (`conch_plugin_sdk`) — no API changes needed, plugins already specify `PanelLocation::Bottom`
- Plugin host (`conch_plugin`) — no changes, panel registration already works
- Config (`conch_core`) — no new config fields

## Testing

- Unit test: `SavedLayout` round-trip with `bottom_panel_visible`
- Manual test: toggle bottom panel → Notifications tab visible with empty state
- Manual test: trigger a toast → entry appears in history
- Manual test: clear button empties the list
- Manual test: close and reopen panel → history preserved within session
- Manual test: restart app → history is empty
- Manual test: native notification (when unfocused) also logged in history
- Manual test: enable a bottom-panel plugin → new tab appears
- Manual test: disable that plugin → tab removed, switches to Notifications
