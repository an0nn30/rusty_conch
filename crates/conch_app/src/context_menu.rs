//! Terminal right-click context menu.
//!
//! Only shown when the terminal is NOT in mouse mode — applications like
//! tmux capture right-click themselves, so we must not conflict.
//!
//! Designed for plugin SDK extensibility: plugins will be able to register
//! additional context menu items via `ContextMenuState` in a future phase.

use crate::menu_bar::MenuAction;

/// Persistent context menu state. Plugins will register items here later.
#[derive(Default)]
pub struct ContextMenuState {
    // Reserved for plugin-registered items in Phase 2.
}

/// Show a context menu on the terminal response.
///
/// `mouse_mode` — whether the terminal application is capturing the mouse.
/// When true, the context menu is suppressed entirely so applications like
/// tmux can handle right-click natively.
///
/// `has_selection` — whether there is an active text selection (enables Copy).
///
/// Returns any triggered action for the caller to process.
pub fn show(
    response: &egui::Response,
    _state: &mut ContextMenuState,
    mouse_mode: bool,
    has_selection: bool,
) -> Option<MenuAction> {
    // Don't show context menu when the terminal app captures the mouse.
    if mouse_mode {
        return None;
    }

    let mut action = None;
    let menu_width = response.ctx.style().spacing.menu_width;

    response.context_menu(|ui| {
        ui.set_min_width(menu_width);

        let copy_btn = ui.add_enabled(has_selection, egui::Button::new("Copy"));
        if copy_btn.clicked() {
            action = Some(MenuAction::Copy);
            ui.close_menu();
        }

        if ui.button("Paste").clicked() {
            action = Some(MenuAction::Paste);
            ui.close_menu();
        }

        ui.separator();

        if ui.button("Select All").clicked() {
            action = Some(MenuAction::SelectAll);
            ui.close_menu();
        }
    });

    action
}
