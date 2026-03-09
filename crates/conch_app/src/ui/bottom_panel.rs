//! Bottom panel area for bottom-panel plugins (tabbed, resizable).

use std::collections::HashMap;

use egui::{Context, RichText};
use egui_extras::{Column, TableBuilder};

/// Minimum height of the bottom panel in logical pixels.
const BOTTOM_PANEL_MIN_HEIGHT: f32 = 80.0;
/// Maximum height of the bottom panel in logical pixels.
const BOTTOM_PANEL_MAX_HEIGHT: f32 = 500.0;

/// Actions that can be triggered by the bottom panel.
pub enum BottomPanelAction {
    None,
    /// A button was clicked in a bottom panel plugin.
    PanelButtonClick { plugin_idx: usize, button_id: String },
    /// Deactivate (stop) a bottom panel plugin.
    DeactivatePanel(usize),
}

/// Render the bottom panel strip with tabs and content.
///
/// Returns an action if the user interacted with a panel control.
pub fn show_bottom_panel(
    ctx: &Context,
    tabs: &[usize],
    active_tab: &mut Option<usize>,
    panel_widgets: &HashMap<usize, Vec<conch_plugin::PanelWidget>>,
    panel_names: &HashMap<usize, String>,
    height: &mut f32,
    visible: &mut bool,
) -> BottomPanelAction {
    let mut action = BottomPanelAction::None;

    egui::TopBottomPanel::bottom("bottom_panel")
        .resizable(true)
        .default_height(*height)
        .height_range(BOTTOM_PANEL_MIN_HEIGHT..=BOTTOM_PANEL_MAX_HEIGHT)
        .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {
            // Track the actual height after user resize.
            *height = ui.available_rect_before_wrap().height();

            // Tab bar at the top of the bottom panel.
            let tab_bar_height = 24.0;
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), tab_bar_height),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    // Subtle separator line at top.
                    let rect = ui.max_rect();
                    ui.painter().hline(
                        rect.x_range(),
                        rect.top(),
                        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    );

                    for &idx in tabs {
                        let name = panel_names.get(&idx).map(|s| s.as_str()).unwrap_or("Panel");
                        let is_active = *active_tab == Some(idx);

                        let text = if is_active {
                            RichText::new(name).size(11.0).strong()
                        } else {
                            RichText::new(name).size(11.0)
                        };

                        let resp = ui.selectable_label(is_active, text);
                        if resp.clicked() {
                            *active_tab = Some(idx);
                        }

                        // Context menu to close the tab.
                        resp.context_menu(|ui| {
                            if ui.button("Close").clicked() {
                                action = BottomPanelAction::DeactivatePanel(idx);
                                ui.close_menu();
                            }
                        });
                    }

                    // Right-aligned collapse button.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("\u{2715}").on_hover_text("Hide panel").clicked() {
                            *visible = false;
                        }
                    });
                },
            );

            ui.separator();

            // Render the active tab's content.
            if let Some(idx) = *active_tab {
                let widgets = panel_widgets.get(&idx).map(|v| v.as_slice()).unwrap_or(&[]);
                let inner = render_panel_widgets(ui, idx, widgets);
                if !matches!(inner, BottomPanelAction::None) {
                    action = inner;
                }
            }
        });

    action
}

/// Render a panel plugin's declarative widgets.
fn render_panel_widgets(
    ui: &mut egui::Ui,
    plugin_idx: usize,
    widgets: &[conch_plugin::PanelWidget],
) -> BottomPanelAction {
    let mut action = BottomPanelAction::None;

    if widgets.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.weak("Loading...");
        });
        return action;
    }

    // Check if any widget is ScrollText — if so, split rendering to avoid
    // nested vertical ScrollAreas (inner would get unbounded height from outer).
    let has_scroll_text = widgets.iter().any(|w| matches!(w, conch_plugin::PanelWidget::ScrollText(_)));

    let render_regular = |ui: &mut egui::Ui, action: &mut BottomPanelAction| {
        for widget in widgets {
            match widget {
                conch_plugin::PanelWidget::ScrollText(_) => {} // rendered separately
                conch_plugin::PanelWidget::Heading(text) => {
                    ui.add_space(4.0);
                    ui.strong(text);
                    ui.add_space(2.0);
                }
                conch_plugin::PanelWidget::Text(text) => {
                    ui.label(RichText::new(text).monospace().size(11.0));
                }
                conch_plugin::PanelWidget::Label(text) => {
                    ui.label(text);
                }
                conch_plugin::PanelWidget::Separator => {
                    ui.separator();
                }
                conch_plugin::PanelWidget::Table { columns, rows } => {
                    let num_cols = columns.len();
                    if num_cols > 0 {
                        TableBuilder::new(ui)
                            .striped(true)
                            .resizable(true)
                            .columns(Column::remainder().at_least(40.0), num_cols)
                            .header(16.0, |mut header| {
                                for col in columns {
                                    header.col(|ui| {
                                        ui.label(RichText::new(col).strong().size(10.0));
                                    });
                                }
                            })
                            .body(|body| {
                                body.rows(16.0, rows.len(), |mut row| {
                                    let idx = row.index();
                                    if let Some(cells) = rows.get(idx) {
                                        for cell in cells {
                                            row.col(|ui| {
                                                ui.label(
                                                    RichText::new(cell).size(11.0).monospace(),
                                                );
                                            });
                                        }
                                    }
                                });
                            });
                    }
                }
                conch_plugin::PanelWidget::Progress { label, fraction, text } => {
                    ui.label(RichText::new(label).size(11.0));
                    let bar = egui::ProgressBar::new(*fraction)
                        .text(text)
                        .desired_width(ui.available_width());
                    ui.add(bar);
                }
                conch_plugin::PanelWidget::Button { id, label } => {
                    if ui.button(label).clicked() {
                        *action = BottomPanelAction::PanelButtonClick {
                            plugin_idx,
                            button_id: id.clone(),
                        };
                    }
                }
                conch_plugin::PanelWidget::KeyValue { key, value } => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(key).strong().size(11.0));
                        ui.label(RichText::new(value).size(11.0).monospace());
                    });
                }
            }
        }
    };

    if has_scroll_text {
        render_regular(ui, &mut action);
        for widget in widgets {
            if let conch_plugin::PanelWidget::ScrollText(lines) = widget {
                egui::ScrollArea::vertical()
                    .id_salt(("bottom_scroll_text", plugin_idx))
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in lines {
                            ui.label(RichText::new(line).monospace().size(11.0));
                        }
                    });
            }
        }
    } else {
        egui::ScrollArea::vertical()
            .id_salt(("bottom_panel_plugin", plugin_idx))
            .show(ui, |ui| {
                render_regular(ui, &mut action);
            });
    }

    action
}
