# Conch v3.0.0

Conch v3.0.0 is a major release focused on turning the app into a more flexible, IDE-like terminal workspace while dramatically expanding the plugin platform.

Compared with `v2.0.2`, this release adds split panes, IntelliJ-style tool windows, a much more capable settings experience, a richer plugin SDK for both Lua and Java, better startup and interaction polish, and a large internal refactor that makes the frontend and backend easier to extend.

## Highlights

- IntelliJ-style tool windows with dockable left, right, and bottom zones
- Full split-pane terminal layout with keyboard navigation and pane management
- A dedicated settings window with search, better grouping, and richer shortcut editing
- A much more capable plugin platform with permissions, API compatibility, HTML widgets, dialogs, inter-plugin messaging, and better runtime behavior
- A new command palette, notification history, and bottom-panel tab system
- Faster startup and a large frontend/runtime modularization pass

## Tool Windows And Layout

- Introduced a full tool-window manager with dockable zones on the left, right, and bottom.
- Added drag-and-drop tool-window movement with animated docking hints and better visual feedback.
- Persisted tool-window locations, split ratios, panel widths, and panel visibility across restarts.
- Changed panel shortcuts so sidebar shortcuts toggle the panel itself, not just a single built-in tool window.
- Added per-tool-window shortcut customization in settings.
- Removed empty sidebar border gaps when a side has no active tool windows.
- Improved plugin tool-window lifecycle and restore behavior so enabled plugins rehydrate reliably on restart.
- Routed bottom-panel plugins into the bottom-panel tab system instead of separate ad hoc UI.

## Split Panes And Terminal Workspace

- Added split-pane layout support for local and SSH sessions.
- Added vertical and horizontal splits, pane close, and tree simplification when panes are removed.
- Added keyboard pane navigation.
- Added split-related menu items, context-menu entries, and settings-visible shortcuts.
- Added SSH multi-channel support so split SSH panes can share an underlying connection.
- Improved pane focus tracking so supporting panels stay in sync with the active pane.
- Made dividers visible and improved split-pane interaction polish.

## Settings, Shortcuts, And Discoverability

- Reworked settings into a dedicated window instead of an overlay dialog.
- Reorganized settings into more intuitive groups inspired by IDE-style preferences.
- Added settings search in the section rail, including keyboard navigation with arrow keys and Enter-to-jump behavior.
- Added animated “jump to setting” highlighting so search results are easy to spot.
- Added keyboard-shortcut filtering and a better shortcut browsing experience.
- Added per-plugin and per-tool-window shortcut handling improvements.
- Added system font enumeration and switched font settings from free-text inputs to real font pickers.
- Added better settings-window focus behavior and cleanup on main-window close.

## Plugin Platform

This is one of the biggest areas of change in the release.

- Added host-side plugin API compatibility checks with `plugin-api` metadata.
- Added capability-based plugin permissions with enforced runtime checks.
- Expanded Lua and Java API parity across menu commands, dialogs, config storage, clipboard, session access, networking, and plugin bus features.
- Added `ui.panel_html` / HTML widget support for custom plugin UI.
- Added `ui.request_render()` and better push-render behavior for tool-window plugins.
- Added more complete widget coverage, better event routing, and richer plugin runtime behavior.
- Added plugin query timeouts and safer callback handling.
- Added safer Lua sandboxing with a reduced standard-library surface.
- Improved plugin dialogs, including duplicate-dialog prevention.
- Improved plugin disable cleanup, panel cleanup, and resource teardown.
- Fixed plugin shortcut edge cases and plugin menu/titlebar UX.
- Improved runtime handling for tool-window plugin restores, push updates, focus preservation, and plugin-created tabs.
- Removed the older docked plugin view type and simplified the plugin/runtime surface around tool windows.

## Plugin SDK And Documentation

- Significantly expanded the SDK docs for both Java and Lua.
- Added comprehensive API signature reference documentation.
- Documented plugin security model and capability mapping.
- Added more realistic SDK guidance and example plugin patterns.
- Updated example plugins and added permission-probe samples.
- Renamed the plugin type terminology from `panel` to `tool_window`, while preserving backward compatibility aliases.

## Command Palette, Notifications, And App UX

- Added a command palette for quickly invoking app and plugin actions.
- Added notification history tracking and a tabbed bottom panel for notifications and plugin views.
- Made the bottom panel resizable and connected it to persisted layout state.
- Improved plugin menu integration across the native menu bar and custom titlebar.
- Added a dismissible global error banner.
- Added configurable notification position and native OS notifications.
- Improved About dialog behavior and general dialog polish.

## Terminal And Input Polish

- Improved terminal clipboard copy/paste reliability, especially on macOS.
- Fixed macOS Meta-arrow tmux input behavior.
- Fixed terminal focus restoration issues after settings and other UI interactions.
- Improved file drag-and-drop path insertion into the terminal.
- Improved new-tab startup timing so the terminal feels more immediate.

## Files, Sessions, Tunnels, And Supporting Panels

- Improved the built-in Files and Sessions tool windows to fit the new layout model.
- Added startup guards and lifecycle fixes so built-in panels behave correctly under deferred initialization.
- Refined tunnel-manager and settings UX.
- Improved session handling, folder handling, and related remote-management behavior.

## Themes, Fonts, And Appearance

- Added instant system font enumeration on macOS via Core Text.
- Added font dropdowns for terminal and UI fonts.
- Improved light-theme contrast and derived more UI colors from the active theme.
- Added UI chrome font-size settings with hot reload.
- Added Linux custom titlebar behavior to avoid GTK menu-bar issues.

## Startup, Stability, And Performance

- Improved startup rendering so the terminal appears sooner while heavier chrome work finishes later.
- Switched several read-heavy structures to `RwLock` for better concurrency behavior.
- Replaced unsafe production `unwrap`/`expect` patterns with stronger error handling.
- Added atomic config writes more broadly, including permission preservation on Unix.
- Added structured remote errors in `conch_remote`.
- Improved cleanup when windows close and when PTY / SSH resources are torn down.
- Added better plugin query timeout handling.

## Internal Refactor And Architecture

- Modularized the frontend substantially, breaking the monolithic shell into focused runtime modules under `frontend/app/`.
- Split large Rust modules such as `lib.rs`, `remote/mod.rs`, and Lua API internals into smaller focused modules.
- Added generated TypeScript types for frontend/backend contracts.
- Improved schema generation and capability metadata.

## CI, Build, And Developer Experience

- Expanded CI with formatting, audit, and cross-platform clippy checks.
- Made the Java SDK JAR optional in builds instead of hard-failing when unavailable.
- Documented per-platform build requirements.
- Added additional tests across core config, color, font, and persistence modules.

## Upgrade Notes

- This release is large enough that existing layouts, shortcuts, and plugin behavior may feel meaningfully different from `v2.0.2`.
- Plugin authors should prefer `tool_window` terminology going forward; legacy `panel` naming remains supported for compatibility.
- Plugins now participate in API compatibility and capability checks, so older or more permissive plugins may need metadata updates.
- Tool-window placement and panel behavior are now much more layout-driven and persistent than in earlier releases.
