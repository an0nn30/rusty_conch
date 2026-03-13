//! Plugin panel renderer — converts Widget trees to egui UI.
//!
//! Takes a `Vec<Widget>` from `conch_plugin_sdk::widgets` and renders each
//! widget into an egui `Ui`. Interactive widgets (buttons, text inputs,
//! checkboxes, combo boxes) generate `WidgetEvent`s that are returned to the
//! caller for delivery back to the plugin.

use std::collections::HashMap;

use conch_plugin_sdk::widgets::{
    BadgeVariant, SplitDirection, TableCell, TableColumn, TableRow, TextStyle, Widget, WidgetEvent,
};
use egui::RichText;

use crate::icons::IconCache;
use crate::ui_theme::UiTheme;

/// Render a list of widgets into an egui Ui, collecting widget events.
///
/// If the widget list ends with `Separator + <widgets...>`, the trailing
/// widgets are pinned to the bottom of the available area so they stay
/// anchored regardless of content height (like a footer).
pub fn render_widgets(
    ui: &mut egui::Ui,
    widgets: &[Widget],
    theme: &UiTheme,
    text_input_state: &mut HashMap<String, String>,
    icon_cache: Option<&IconCache>,
) -> Vec<WidgetEvent> {
    let mut events = Vec::new();

    // Detect a trailing footer: find the last Separator and treat everything
    // after it as a bottom-pinned footer.
    let footer_split = widgets.iter().rposition(|w| matches!(w, Widget::Separator));
    let (body, footer) = if let Some(sep_idx) = footer_split {
        // Only treat as footer if it's not the very first widget and there
        // are widgets after it.
        if sep_idx > 0 && sep_idx < widgets.len() - 1 {
            (&widgets[..sep_idx], Some(&widgets[sep_idx + 1..]))
        } else {
            (widgets, None)
        }
    } else {
        (widgets, None)
    };

    // Render footer first so egui reserves bottom space before the main content.
    if let Some(footer_widgets) = footer {
        egui::TopBottomPanel::bottom(ui.id().with("__footer"))
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                ui.separator();
                for widget in footer_widgets {
                    render_footer_widget(ui, widget, theme, &mut events, icon_cache);
                }
            });
    }

    // Render main body widgets.
    for widget in body {
        render_widget(ui, widget, theme, text_input_state, &mut events, icon_cache);
    }

    events
}

/// Render a panel header row with the panel name and, if the first widget is a
/// Toolbar, merge its items into the same row. Returns the remaining widgets
/// (skipping the toolbar if it was consumed).
///
/// Renders: `[ PanelName ...spacer... toolbar items ]` then a separator.
pub fn render_panel_header<'a>(
    ui: &mut egui::Ui,
    panel_name: &str,
    widgets: &'a [Widget],
    theme: &UiTheme,
    text_input_state: &mut HashMap<String, String>,
    events: &mut Vec<WidgetEvent>,
    icon_cache: Option<&IconCache>,
) -> &'a [Widget] {
    // Check if the first widget is a Toolbar.
    let (toolbar_items, rest) = match widgets.first() {
        Some(Widget::Toolbar { items, .. }) => (Some(items.as_slice()), &widgets[1..]),
        _ => (None, widgets),
    };

    ui.horizontal(|ui| {
        // Panel name on the left.
        ui.label(
            egui::RichText::new(panel_name)
                .size(theme.font_normal + 1.0)
                .strong()
                .color(theme.text),
        );

        // If there are toolbar items, render them right-aligned.
        if let Some(items) = toolbar_items {
            // Filter out leading spacers — we handle alignment ourselves.
            let items: Vec<_> = items
                .iter()
                .filter(|i| !matches!(i, conch_plugin_sdk::widgets::ToolbarItem::Spacer))
                .collect();
            if !items.is_empty() {
                let available = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(available, ui.spacing().interact_size.y),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        for item in &items {
                            render_toolbar_item(ui, item, theme, text_input_state, events, icon_cache);
                        }
                    },
                );
            }
        }
    });
    ui.separator();

    rest
}

/// Render a single widget, recursing into layout containers.
fn render_widget(
    ui: &mut egui::Ui,
    widget: &Widget,
    theme: &UiTheme,
    text_input_state: &mut HashMap<String, String>,
    events: &mut Vec<WidgetEvent>,
    icon_cache: Option<&IconCache>,
) {
    match widget {
        // -- Layout Containers ------------------------------------------------

        Widget::Horizontal {
            children, spacing, ..
        } => {
            ui.horizontal(|ui| {
                if let Some(sp) = spacing {
                    ui.spacing_mut().item_spacing.x = *sp;
                }
                for child in children {
                    render_widget(ui, child, theme, text_input_state, events, icon_cache);
                }
            });
        }

        Widget::Vertical {
            children, spacing, ..
        } => {
            ui.vertical(|ui| {
                if let Some(sp) = spacing {
                    ui.spacing_mut().item_spacing.y = *sp;
                }
                for child in children {
                    render_widget(ui, child, theme, text_input_state, events, icon_cache);
                }
            });
        }

        Widget::ScrollArea {
            children,
            max_height,
            ..
        } => {
            let mut scroll = egui::ScrollArea::vertical();
            if let Some(h) = max_height {
                scroll = scroll.max_height(*h);
            }
            scroll.show(ui, |ui| {
                for child in children {
                    render_widget(ui, child, theme, text_input_state, events, icon_cache);
                }
            });
        }

        // -- Data Display -----------------------------------------------------

        Widget::Heading { text } => {
            ui.label(
                RichText::new(text)
                    .size(theme.font_normal + 2.0)
                    .strong()
                    .color(theme.text),
            );
        }

        Widget::Label { text, style } => {
            let color = text_style_color(style.as_ref(), theme);
            ui.label(RichText::new(text).size(theme.font_normal).color(color));
        }

        Widget::Text { text } => {
            ui.label(
                RichText::new(text)
                    .monospace()
                    .size(theme.font_small)
                    .color(theme.text),
            );
        }

        Widget::ScrollText {
            text, max_height, ..
        } => {
            let max_h = max_height.unwrap_or(200.0);
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text)
                            .monospace()
                            .size(theme.font_small)
                            .color(theme.text),
                    );
                });
        }

        Widget::KeyValue { key, value } => {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(key)
                        .color(theme.text_secondary)
                        .size(theme.font_small),
                );
                ui.label(
                    RichText::new(value)
                        .color(theme.text)
                        .size(theme.font_small),
                );
            });
        }

        Widget::Separator => {
            ui.separator();
        }

        Widget::Spacer { size } => {
            ui.add_space(size.unwrap_or(8.0));
        }

        Widget::IconLabel { text, style, .. } => {
            // MVP: render as a plain label, ignoring the icon.
            let color = text_style_color(style.as_ref(), theme);
            ui.label(RichText::new(text).size(theme.font_normal).color(color));
        }

        Widget::Badge { text, variant } => {
            let color = match variant {
                BadgeVariant::Info => theme.accent,
                BadgeVariant::Success => theme.accent,
                BadgeVariant::Warn => theme.warn,
                BadgeVariant::Error => theme.error,
            };
            ui.label(
                RichText::new(text)
                    .size(theme.font_small)
                    .strong()
                    .color(color),
            );
        }

        Widget::Progress {
            fraction, label, ..
        } => {
            let mut bar = egui::ProgressBar::new(*fraction);
            if let Some(lbl) = label {
                bar = bar.text(lbl.as_str());
            }
            ui.add(bar);
        }

        Widget::Image { .. } => {
            ui.label(
                RichText::new("[Image]")
                    .size(theme.font_small)
                    .color(theme.text_muted),
            );
        }

        // -- Interactive Widgets ----------------------------------------------

        Widget::Button {
            id,
            label,
            icon,
            enabled,
        } => {
            let is_enabled = enabled.unwrap_or(true);
            let text_color = if is_enabled { theme.text } else { theme.text_muted };
            ui.add_enabled_ui(is_enabled, |ui| {
                ui.horizontal(|ui| {
                    // Render icon if provided.
                    if let Some(icon_name) = icon {
                        let dark_mode = ui.visuals().dark_mode;
                        if let Some(ic) = icon_cache {
                            if let Some(img) = ic.image_by_name(icon_name, dark_mode) {
                                ui.add(img);
                            }
                        }
                    }
                    let button = egui::Button::new(
                        RichText::new(label)
                            .size(theme.font_normal)
                            .color(text_color),
                    );
                    if ui.add(button).clicked() {
                        events.push(WidgetEvent::ButtonClick { id: id.clone() });
                    }
                });
            });
        }

        Widget::TextInput {
            id,
            value,
            hint,
            submit_on_enter,
        } => {
            // Initialize local edit buffer from the plugin's canonical value
            // if we haven't seen this widget before.
            let buf = text_input_state
                .entry(id.clone())
                .or_insert_with(|| value.clone());

            let mut te = egui::TextEdit::singleline(buf)
                .font(egui::TextStyle::Body)
                .margin(theme.text_edit_margin());
            if let Some(h) = hint {
                te = te.hint_text(h);
            }

            let response = ui.add(te);

            // Detect value change.
            if response.changed() {
                events.push(WidgetEvent::TextInputChanged {
                    id: id.clone(),
                    value: buf.clone(),
                });
            }

            // Detect Enter key submission.
            if submit_on_enter.unwrap_or(false) && response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
            {
                let submitted = buf.clone();
                // Clear local state so it re-syncs from the plugin next frame.
                text_input_state.remove(id);
                events.push(WidgetEvent::TextInputSubmit {
                    id: id.clone(),
                    value: submitted,
                });
            }
        }

        Widget::TextEdit {
            id, value, hint, lines,
        } => {
            let buf = text_input_state
                .entry(id.clone())
                .or_insert_with(|| value.clone());

            let desired_rows = lines.unwrap_or(4) as usize;
            let mut te = egui::TextEdit::multiline(buf)
                .font(egui::TextStyle::Monospace)
                .margin(theme.text_edit_margin())
                .desired_rows(desired_rows);
            if let Some(h) = hint {
                te = te.hint_text(h);
            }

            let response = ui.add(te);
            if response.changed() {
                events.push(WidgetEvent::TextEditChanged {
                    id: id.clone(),
                    value: buf.clone(),
                });
            }
        }

        Widget::Checkbox {
            id,
            label,
            checked,
        } => {
            let mut val = *checked;
            if ui.checkbox(&mut val, label).changed() {
                events.push(WidgetEvent::CheckboxChanged {
                    id: id.clone(),
                    checked: val,
                });
            }
        }

        Widget::ComboBox {
            id,
            selected,
            options,
        } => {
            let mut current = selected.clone();
            egui::ComboBox::from_id_salt(id)
                .selected_text(&current)
                .show_ui(ui, |ui| {
                    for opt in options {
                        ui.selectable_value(&mut current, opt.value.clone(), &opt.label);
                    }
                });
            if current != *selected {
                events.push(WidgetEvent::ComboBoxChanged {
                    id: id.clone(),
                    value: current,
                });
            }
        }

        // -- Complex Widgets (MVP placeholders) -------------------------------

        Widget::SplitPane {
            direction, left, right, ..
        } => {
            match direction {
                SplitDirection::Horizontal => {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            render_widget(ui, left, theme, text_input_state, events, icon_cache);
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            render_widget(ui, right, theme, text_input_state, events, icon_cache);
                        });
                    });
                }
                SplitDirection::Vertical => {
                    render_widget(ui, left, theme, text_input_state, events, icon_cache);
                    ui.separator();
                    render_widget(ui, right, theme, text_input_state, events, icon_cache);
                }
            }
        }

        Widget::Tabs { id, tabs, active } => {
            // Tab selector row.
            ui.horizontal(|ui| {
                for (i, tab) in tabs.iter().enumerate() {
                    let selected = i == *active;
                    if ui.selectable_label(selected, &tab.label).clicked() && !selected {
                        events.push(WidgetEvent::TabChanged {
                            id: id.clone(),
                            active: i,
                        });
                    }
                }
            });
            ui.separator();
            // Render active tab's children.
            if let Some(pane) = tabs.get(*active) {
                for child in &pane.children {
                    render_widget(ui, child, theme, text_input_state, events, icon_cache);
                }
            }
        }

        Widget::Toolbar { items, .. } => {
            // Check if items start with a Spacer — if so, render remaining items right-aligned.
            let has_leading_spacer = matches!(items.first(), Some(conch_plugin_sdk::widgets::ToolbarItem::Spacer));
            let items_to_render = if has_leading_spacer { &items[1..] } else { &items[..] };

            if has_leading_spacer {
                // Use horizontal wrapping to constrain height, then right-align inside.
                ui.horizontal(|ui| {
                    let available = ui.available_width();
                    ui.allocate_ui_with_layout(
                        egui::vec2(available, ui.spacing().interact_size.y),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            for item in items_to_render {
                                render_toolbar_item(ui, item, theme, text_input_state, events, icon_cache);
                            }
                        },
                    );
                });
            } else {
                ui.horizontal(|ui| {
                    for item in items_to_render {
                        render_toolbar_item(ui, item, theme, text_input_state, events, icon_cache);
                    }
                });
            }
        }

        Widget::Table {
            id, columns, rows, sort_column, sort_ascending, selected_row,
        } => {
            render_table(
                ui, id, columns, rows,
                sort_column.as_deref(), *sort_ascending,
                selected_row.as_deref(), theme, events,
            );
        }

        Widget::TreeView { id, nodes, selected } => {
            for node in nodes {
                render_tree_node(ui, id, node, selected.as_deref(), 0, theme, text_input_state, events, icon_cache);
            }
        }

        Widget::PathBar { id, segments } => {
            ui.horizontal(|ui| {
                for (i, segment) in segments.iter().enumerate() {
                    if i > 0 {
                        ui.label(
                            RichText::new("›")
                                .size(theme.font_small)
                                .color(theme.text_muted),
                        );
                    }
                    let resp = ui.add(
                        egui::Label::new(
                            RichText::new(segment)
                                .size(theme.font_small)
                                .color(theme.accent),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if resp.clicked() {
                        events.push(WidgetEvent::PathBarNavigate {
                            id: id.clone(),
                            segment_index: i,
                        });
                    }
                }
            });
        }

        Widget::DropZone { label, children, .. } => {
            ui.group(|ui| {
                ui.label(
                    RichText::new(label)
                        .size(theme.font_small)
                        .color(theme.text_muted),
                );
                for child in children {
                    render_widget(ui, child, theme, text_input_state, events, icon_cache);
                }
            });
        }

        Widget::ContextMenu { child, .. } => {
            // MVP: just render the child, ignore context menu.
            render_widget(ui, child, theme, text_input_state, events, icon_cache);
        }
    }
}

/// Render a single tree node (recursive for children).
///
/// Folder nodes (those with children) use `CollapsingState` for animated
/// expand/collapse. Leaf nodes render as a simple horizontal row with
/// click-to-select and double-click-to-activate.
fn render_tree_node(
    ui: &mut egui::Ui,
    tree_id: &str,
    node: &conch_plugin_sdk::widgets::TreeNode,
    selected: Option<&str>,
    depth: usize,
    theme: &UiTheme,
    text_input_state: &mut HashMap<String, String>,
    events: &mut Vec<WidgetEvent>,
    icon_cache: Option<&IconCache>,
) {
    let is_selected = selected == Some(node.id.as_str());
    let has_children = !node.children.is_empty();
    let is_bold = node.bold.unwrap_or(false);

    if has_children {
        // --- Folder node: animated collapsing header via CollapsingState ---
        let coll_id = ui.make_persistent_id(("tree_node", &node.id));
        let mut coll = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            coll_id,
            node.expanded.unwrap_or(false),
        );

        // Header row: icon + clickable label (click toggles expand/collapse).
        let header_resp = ui.horizontal(|ui| {
            // Icon
            if let Some(icon_name) = &node.icon {
                let dark_mode = ui.visuals().dark_mode;
                if let Some(ic) = icon_cache {
                    if let Some(img) = ic.image_by_name(icon_name, dark_mode) {
                        let icon_color = match node.icon_color.as_deref() {
                            Some("blue") => Some(theme.accent),
                            Some("muted" | "grey" | "gray") => Some(theme.text_muted),
                            _ => None,
                        };
                        let img = if let Some(color) = icon_color {
                            img.tint(color)
                        } else {
                            img
                        };
                        ui.add(img);
                    }
                }
            }

            // Label — clicking toggles the collapsing state.
            let mut label_text = RichText::new(&node.label)
                .size(theme.font_normal)
                .color(if is_selected { theme.accent } else { theme.text });
            if is_bold {
                label_text = label_text.strong();
            }
            let resp = ui.add(egui::Label::new(label_text).sense(egui::Sense::click()));
            if resp.clicked() {
                coll.toggle(ui);
                events.push(WidgetEvent::TreeToggle {
                    id: tree_id.to_string(),
                    node_id: node.id.clone(),
                    expanded: coll.is_open(),
                });
            }
            resp
        });

        // Context menu on the header row.
        if let Some(menu_items) = &node.context_menu {
            header_resp.inner.context_menu(|ui| {
                for item in menu_items {
                    let enabled = item.enabled.unwrap_or(true);
                    let btn = egui::Button::new(&item.label);
                    if ui.add_enabled(enabled, btn).clicked() {
                        events.push(WidgetEvent::TreeContextMenu {
                            id: tree_id.to_string(),
                            node_id: node.id.clone(),
                            action: item.id.clone(),
                        });
                        ui.close_menu();
                    }
                }
            });
        }

        // Animated body with indented children.
        coll.show_body_unindented(ui, |ui| {
            ui.indent(coll_id, |ui| {
                for child in &node.children {
                    render_tree_node(
                        ui,
                        tree_id,
                        child,
                        selected,
                        depth + 1,
                        theme,
                        text_input_state,
                        events,
                        icon_cache,
                    );
                }
            });
        });

        coll.store(ui.ctx());
    } else {
        // --- Leaf node: simple row with select / activate ---
        ui.horizontal(|ui| {
            // Icon
            if let Some(icon_name) = &node.icon {
                let dark_mode = ui.visuals().dark_mode;
                if let Some(ic) = icon_cache {
                    if let Some(img) = ic.image_by_name(icon_name, dark_mode) {
                        let icon_color = match node.icon_color.as_deref() {
                            Some("blue") => Some(theme.accent),
                            Some("muted" | "grey" | "gray") => Some(theme.text_muted),
                            _ => None,
                        };
                        let img = if let Some(color) = icon_color {
                            img.tint(color)
                        } else {
                            img
                        };
                        ui.add(img);
                    }
                }
            }

            // Clickable label (no selection highlight).
            let mut label_text = RichText::new(&node.label)
                .size(theme.font_normal)
                .color(theme.text);
            if is_bold {
                label_text = label_text.strong();
            }

            let response = ui.add(egui::Label::new(label_text).sense(egui::Sense::click()));
            if response.clicked() {
                events.push(WidgetEvent::TreeSelect {
                    id: tree_id.to_string(),
                    node_id: node.id.clone(),
                });
            }
            if response.double_clicked() {
                events.push(WidgetEvent::TreeActivate {
                    id: tree_id.to_string(),
                    node_id: node.id.clone(),
                });
            }

            // Badge (e.g., "connected").
            if let Some(badge) = &node.badge {
                ui.label(
                    RichText::new(badge)
                        .size(theme.font_small)
                        .strong()
                        .color(theme.accent),
                );
            }

            // Context menu on right-click.
            if let Some(menu_items) = &node.context_menu {
                response.context_menu(|ui| {
                    for item in menu_items {
                        let enabled = item.enabled.unwrap_or(true);
                        let btn = egui::Button::new(&item.label);
                        if ui.add_enabled(enabled, btn).clicked() {
                            events.push(WidgetEvent::TreeContextMenu {
                                id: tree_id.to_string(),
                                node_id: node.id.clone(),
                                action: item.id.clone(),
                            });
                            ui.close_menu();
                        }
                    }
                });
            }
        });
    }
}

/// Render a footer widget — buttons are rendered as clickable labels (no outline)
/// to match the native panel footer style.
fn render_footer_widget(
    ui: &mut egui::Ui,
    widget: &Widget,
    theme: &UiTheme,
    events: &mut Vec<WidgetEvent>,
    icon_cache: Option<&IconCache>,
) {
    match widget {
        Widget::Button { id, label, icon, .. } => {
            ui.horizontal(|ui| {
                if let Some(icon_name) = icon {
                    let dark_mode = ui.visuals().dark_mode;
                    if let Some(ic) = icon_cache {
                        if let Some(img) = ic.image_by_name(icon_name, dark_mode) {
                            ui.add(img);
                        }
                    }
                }
                if ui
                    .add(egui::Label::new(
                        RichText::new(label).size(theme.font_normal).color(theme.text),
                    ).sense(egui::Sense::click()))
                    .clicked()
                {
                    events.push(WidgetEvent::ButtonClick { id: id.clone() });
                }
            });
        }
        Widget::Separator => {
            ui.separator();
        }
        _ => {
            // Fallback: render normally.
            let mut text_state = HashMap::new();
            render_widget(ui, widget, theme, &mut text_state, events, icon_cache);
        }
    }
}

/// Render a single toolbar item.
fn render_toolbar_item(
    ui: &mut egui::Ui,
    item: &conch_plugin_sdk::widgets::ToolbarItem,
    theme: &UiTheme,
    text_input_state: &mut HashMap<String, String>,
    events: &mut Vec<WidgetEvent>,
    icon_cache: Option<&IconCache>,
) {
    match item {
        conch_plugin_sdk::widgets::ToolbarItem::Button {
            id, icon, label, tooltip, enabled,
        } => {
            let is_enabled = enabled.unwrap_or(true);
            let dark_mode = ui.visuals().dark_mode;

            let clicked = if label.is_none() {
                // Icon-only button.
                let mut did_click = false;
                if let Some(icon_name) = icon {
                    if let Some(ic) = icon_cache {
                        if let Some(img) = ic.image_by_name(icon_name, dark_mode) {
                            let resp = ui.add_enabled(is_enabled, egui::ImageButton::new(img));
                            if resp.clicked() {
                                did_click = true;
                            }
                            if let Some(tt) = tooltip {
                                resp.on_hover_text(tt);
                            }
                        }
                    }
                }
                did_click
            } else {
                // Icon + text button.
                if let Some(icon_name) = icon {
                    if let Some(ic) = icon_cache {
                        if let Some(img) = ic.image_by_name(icon_name, dark_mode) {
                            ui.add(img);
                        }
                    }
                }
                let resp = ui.add_enabled(is_enabled, egui::Button::new(label.as_deref().unwrap()));
                let did_click = resp.clicked();
                if let Some(tt) = tooltip {
                    resp.on_hover_text(tt);
                }
                did_click
            };
            if clicked {
                events.push(WidgetEvent::ButtonClick { id: id.clone() });
            }
        }
        conch_plugin_sdk::widgets::ToolbarItem::Separator => {
            ui.separator();
        }
        conch_plugin_sdk::widgets::ToolbarItem::Spacer => {
            // Mid-toolbar spacer — best effort.
            ui.add_space(8.0);
        }
        conch_plugin_sdk::widgets::ToolbarItem::TextInput { id, value, hint } => {
            let buf = text_input_state
                .entry(id.clone())
                .or_insert_with(|| value.clone());
            let mut te = egui::TextEdit::singleline(buf)
                .font(egui::TextStyle::Body)
                .margin(theme.text_edit_margin())
                .desired_width(120.0);
            if let Some(h) = hint {
                te = te.hint_text(h);
            }
            let response = ui.add(te);
            if response.changed() {
                events.push(WidgetEvent::ToolbarInputChanged {
                    id: id.clone(),
                    value: buf.clone(),
                });
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                events.push(WidgetEvent::ToolbarInputSubmit {
                    id: id.clone(),
                    value: buf.clone(),
                });
            }
        }
    }
}

/// Render a Table widget with sortable columns, selectable rows, and context menus.
fn render_table(
    ui: &mut egui::Ui,
    table_id: &str,
    columns: &[TableColumn],
    rows: &[TableRow],
    sort_column: Option<&str>,
    sort_ascending: Option<bool>,
    selected_row: Option<&str>,
    theme: &UiTheme,
    events: &mut Vec<WidgetEvent>,
) {
    // Header row.
    ui.horizontal(|ui| {
        for col in columns {
            let is_sorted = sort_column == Some(col.id.as_str());
            let asc = sort_ascending.unwrap_or(true);
            let label = if is_sorted {
                let arrow = if asc { " ▲" } else { " ▼" };
                format!("{}{arrow}", col.label)
            } else {
                col.label.clone()
            };

            let sortable = col.sortable.unwrap_or(false);
            let width = col.width.unwrap_or(100.0);
            let response = ui.add_sized(
                [width, ui.spacing().interact_size.y],
                egui::Label::new(
                    RichText::new(label)
                        .size(theme.font_small)
                        .strong()
                        .color(theme.text),
                )
                .sense(if sortable { egui::Sense::click() } else { egui::Sense::hover() }),
            );

            if sortable && response.clicked() {
                let new_asc = if is_sorted { !asc } else { true };
                events.push(WidgetEvent::TableSort {
                    id: table_id.to_string(),
                    column: col.id.clone(),
                    ascending: new_asc,
                });
            }
        }
    });

    ui.separator();

    // Data rows.
    for row in rows {
        let is_selected = selected_row == Some(row.id.as_str());

        let response = ui.horizontal(|ui| {
            for (i, cell) in row.cells.iter().enumerate() {
                let width = columns.get(i).and_then(|c| c.width).unwrap_or(100.0);
                let text = match cell {
                    TableCell::Text(t) => t.as_str(),
                    TableCell::Rich { text, .. } => text.as_str(),
                };
                ui.add_sized(
                    [width, ui.spacing().interact_size.y],
                    egui::SelectableLabel::new(is_selected, text),
                );
            }
        });

        if response.response.clicked() {
            events.push(WidgetEvent::TableSelect {
                id: table_id.to_string(),
                row_id: row.id.clone(),
            });
        }
        if response.response.double_clicked() {
            events.push(WidgetEvent::TableActivate {
                id: table_id.to_string(),
                row_id: row.id.clone(),
            });
        }

        if let Some(menu_items) = &row.context_menu {
            response.response.context_menu(|ui| {
                for item in menu_items {
                    let enabled = item.enabled.unwrap_or(true);
                    let btn = egui::Button::new(&item.label);
                    if ui.add_enabled(enabled, btn).clicked() {
                        events.push(WidgetEvent::TableContextMenu {
                            id: table_id.to_string(),
                            row_id: row.id.clone(),
                            action: item.id.clone(),
                        });
                        ui.close_menu();
                    }
                }
            });
        }
    }
}

/// Map an optional `TextStyle` to a theme color.
fn text_style_color(style: Option<&TextStyle>, theme: &UiTheme) -> egui::Color32 {
    match style {
        None | Some(TextStyle::Normal) => theme.text,
        Some(TextStyle::Secondary) => theme.text_secondary,
        Some(TextStyle::Muted) => theme.text_muted,
        Some(TextStyle::Accent) => theme.accent,
        Some(TextStyle::Warn) => theme.warn,
        Some(TextStyle::Error) => theme.error,
    }
}
