# Split Pane Support — Design Spec

## Overview

Add nestable split-pane support to Conch alongside the existing tab system. Any terminal pane can be split horizontally (top/bottom) or vertically (left/right), forming a recursive binary tree within each tab. When splitting an SSH session, the new pane opens a new channel on the existing SSH connection. Local shell splits spawn a fresh local PTY.

## Architecture

**Hybrid approach:** JavaScript owns the split tree and DOM layout (nested flexbox). Rust owns session lifecycle (PTY spawn, SSH channel open, write, resize, close). A unique `paneId` links JS tree leaf nodes to Rust backend sessions.

## Data Model

### Split Tree (JS)

Each tab maps to a split tree root. Two node types:

```js
// Leaf — a terminal pane
{ type: 'leaf', paneId: Number }

// Split — a container with two children
{ type: 'split', direction: 'horizontal' | 'vertical', ratio: Number, children: [node, node] }
```

A separate `panes` Map keyed by `paneId` holds pane-level data: xterm.js Terminal instance, FitAddon, session type (`'local'` or `'ssh'`), `connectionId` (for SSH panes), spawned state, and cached dimensions.

### Backend Session Keys (Rust)

The existing `TauriState::ptys` HashMap changes its key from `window_label:tab_id` to `window_label:pane_id`. SSH sessions are similarly keyed by pane ID. No tree structure exists in Rust — just a flat map of sessions.

### Example

```
Tab "Tab 1"
└── SplitNode(vertical, 0.5)
    ├── Leaf(paneId: 1)        ← local shell
    └── SplitNode(horizontal, 0.6)
        ├── Leaf(paneId: 2)    ← local shell
        └── Leaf(paneId: 3)    ← local shell
```

## DOM Layout

Each tree node maps to a DOM element inside `#terminal-host`:

- **Leaf** → `div.terminal-pane` with `data-pane-id` attribute (contains xterm.js)
- **Split** → `div.split-container` with `flex-direction: row` (vertical split) or `column` (horizontal split)
- **Divider** → `div.split-divider` between the two children, 4px wide/tall

```html
<div class="split-container" style="flex-direction: row;">
  <div class="terminal-pane" style="flex: 0.5;" data-pane-id="1">
    <!-- xterm.js -->
  </div>
  <div class="split-divider vertical"></div>
  <div class="split-container" style="flex-direction: column; flex: 0.5;">
    <div class="terminal-pane" style="flex: 0.6;" data-pane-id="2">
      <!-- xterm.js -->
    </div>
    <div class="split-divider horizontal"></div>
    <div class="terminal-pane" style="flex: 0.4;" data-pane-id="3">
      <!-- xterm.js -->
    </div>
  </div>
</div>
```

Only the active tab's tree root has `display: block`; others are hidden. Each pane has a `ResizeObserver` that calls `fitAddon.fit()` and sends dimensions to the backend when its container size changes.

Minimum pane size: 80px in either dimension, enforced during divider drag.

## Splitting Flow

When a split is triggered:

1. Identify the active (focused) pane — the current leaf node.
2. Create a new split node replacing the leaf in its parent:
   - Original leaf becomes `children[0]`.
   - New leaf (fresh `paneId`) becomes `children[1]`.
   - Direction from user's choice. Initial ratio: `0.5`.
3. Rebuild the DOM subtree — wrap in `div.split-container`, insert divider, append both panes.
4. Spawn the new session based on the originating pane's type:
   - **Local shell:** `spawn_shell(paneId, cols, rows)` — same as new tab creation.
   - **SSH:** `ssh_open_channel(paneId, connectionId, cols, rows)` — opens a new channel on the existing SSH connection, spawns a shell, wires up `pty-output`/`pty-exit` events scoped to the new `paneId`.
5. Focus the new pane.

### SSH Multi-Channel Support

A new Tauri command `ssh_open_channel` opens an additional channel on an already-established SSH connection. The `RemoteState` is extended to:

- Look up a connection by `connectionId` (derived from the original pane that established it).
- Open a new `russh` channel on that connection.
- Spawn a reader/output-forwarder task emitting `pty-output` events tagged with the new `paneId`.
- Track a reference count of panes per connection for cleanup.

## Focus Management

### Focus Tracking

- A `focusedPaneId` variable tracks the active pane across the window.
- Clicking a pane's terminal area sets it as focused.
- Focused pane has a 2px accent border on the top edge (`var(--blue)`).
- Switching tabs restores focus to the last-focused pane in that tab.

### Keyboard Navigation

- `Cmd+Alt+Arrow` moves focus to the adjacent pane in that direction.
- Navigation is spatial — compares pane bounding rects to find the nearest neighbor in the arrow direction.
- No wrapping — if no pane exists in that direction, the shortcut is a no-op.

### Input Routing

- Only the focused pane receives keyboard input.
- `write_to_pty` / `ssh_write` route via the focused pane's `paneId`.
- All panes independently resize via their own ResizeObserver.

## Pane Close and Tree Simplification

When a pane is closed:

1. **Terminate session:** `close_pty(paneId)` or close the SSH channel for that pane.
2. **Remove the leaf** from the tree.
3. **Simplify the tree:** The leaf's sibling replaces the parent split node, inheriting the parent's position and flex value. Divider DOM element is removed. If the sibling is a split node, its subtree is preserved.
4. **Reclaim space:** The remaining sibling expands to fill 100% of the former split area.
5. **Refocus:** If the closed pane was focused, focus moves to the sibling (or sibling's first leaf if it's a split node).
6. **Last pane in tab:** Closing the only remaining pane closes the entire tab.

### SSH Connection Cleanup

- A reference count tracks panes per SSH connection.
- Closing a pane decrements the count and closes only that channel.
- When the count reaches zero, the SSH connection itself is disconnected and cleaned up from `RemoteState`.

## User Interaction

### Keyboard Shortcuts

Configurable via `[conch.keyboard]` in `config.toml`:

| Action | Default Shortcut |
|---|---|
| Split pane vertically (left/right) | `Cmd+D` |
| Split pane horizontally (top/bottom) | `Cmd+Shift+D` |
| Close pane | `Cmd+Shift+W` |
| Navigate to adjacent pane | `Cmd+Alt+Arrow` |

### Menu Bar Items

Under the Shell menu:
- Split Pane Vertically (`Cmd+D`)
- Split Pane Horizontally (`Cmd+Shift+D`)
- Close Pane (`Cmd+Shift+W`)

### Right-Click Context Menu

- "Split Vertically" and "Split Horizontally" in a terminal context menu.
- Only appears when the terminal does NOT have mouse tracking enabled (tmux right-click bridge takes precedence when mouse mode is active).

## State Persistence

**Not persisted across app restarts.** Each launch starts fresh with a single local shell tab. Split trees live in JS memory for the duration of the session — switching tabs preserves layout, but closing the app discards it.

No tree serialization, no session restore logic.

## Testing Strategy

### Rust Unit Tests

- Session key generation with pane IDs (`window:paneId` format).
- SSH multi-channel management: open/close channels, reference counting, cleanup on last channel close.
- Existing PTY commands work unchanged with pane ID keys.

### JS Pure Function Tests

Split tree operations extracted as pure functions (no DOM dependency):
- Splitting a leaf produces correct tree structure.
- Removing a leaf simplifies the parent correctly.
- Adjacent-pane spatial lookup returns correct neighbors.
- Ratio clamping respects minimum pane size constraints.

### Manual Test Scenarios

- Split local shell — new shell spawns, both accept input independently.
- Split SSH session — new channel opens on same connection, independent shells.
- Nested splits (3+ levels deep) — layout renders correctly, resize works.
- Close middle pane — tree simplifies, sibling expands.
- Close all panes in SSH connection — connection disconnects.
- Divider drag — both adjacent panes resize, xterm fits correctly.
- Window resize with active splits — all panes refit proportionally.
- Tab switching with splits — layout preserved, focus restored.

## Follow-Up (Out of Scope)

- **SSH split behavior setting:** Add a preference in Settings to choose between reusing the existing SSH connection (new channel) or opening a new independent connection when splitting an SSH pane.
