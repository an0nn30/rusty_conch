//! Preferences window — multi-section settings editor.

use conch_core::color_scheme;
use conch_core::config::{self, UserConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencesSection {
    Terminal,
    Appearance,
    General,
}

impl PreferencesSection {
    const ALL: [Self; 3] = [Self::Terminal, Self::Appearance, Self::General];

    fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Appearance => "Appearance",
            Self::General => "General",
        }
    }
}

pub struct PreferencesForm {
    active_section: PreferencesSection,
    // Terminal
    pub terminal_font_family: String,
    pub terminal_font_size: String,
    pub theme_name: String,
    pub available_themes: Vec<String>,
    pub shell_program: String,
    pub shell_args: String,
    pub startup_command: String,
    pub use_tmux: bool,
    // Window
    pub window_columns: String,
    pub window_lines: String,
    // Appearance
    pub ui_font_family: String,
    pub ui_font_size: String,
}

impl PreferencesForm {
    pub fn from_config(config: &UserConfig) -> Self {
        let mut themes: Vec<String> = color_scheme::list_themes()
            .keys()
            .cloned()
            .collect();
        if !themes.iter().any(|t| t == "dracula") {
            themes.push("dracula".into());
        }
        themes.sort();

        Self {
            active_section: PreferencesSection::Terminal,
            terminal_font_family: config.font.normal.family.clone(),
            terminal_font_size: format!("{}", config.font.size),
            theme_name: config.colors.theme.clone(),
            available_themes: themes,
            shell_program: config.terminal.shell.program.clone(),
            shell_args: config.terminal.shell.args.join(" "),
            startup_command: config.terminal.shell.startup_command.clone(),
            use_tmux: config.terminal.shell.use_tmux,
            window_columns: format!("{}", config.window.dimensions.columns),
            window_lines: format!("{}", config.window.dimensions.lines),
            ui_font_family: config.conch.ui.font_family.clone(),
            ui_font_size: format!("{}", config.conch.ui.font_size),
        }
    }

    pub fn apply_to_config(&self, config: &mut UserConfig) {
        config.font.normal.family = self.terminal_font_family.clone();
        if let Ok(size) = self.terminal_font_size.trim().parse::<f32>() {
            if size > 0.0 {
                config.font.size = size;
            }
        }
        config.colors.theme = self.theme_name.clone();

        // [terminal.shell]
        config.terminal.shell.program = self.shell_program.trim().to_string();
        config.terminal.shell.args = if self.shell_args.trim().is_empty() {
            Vec::new()
        } else {
            self.shell_args.split_whitespace().map(String::from).collect()
        };
        config.terminal.shell.startup_command = self.startup_command.clone();
        config.terminal.shell.use_tmux = self.use_tmux;

        // [window.dimensions]
        if let Ok(cols) = self.window_columns.trim().parse::<u16>() {
            config.window.dimensions.columns = cols;
        }
        if let Ok(lines) = self.window_lines.trim().parse::<u16>() {
            config.window.dimensions.lines = lines;
        }

        // [conch.ui]
        config.conch.ui.font_family = self.ui_font_family.clone();
        if let Ok(size) = self.ui_font_size.trim().parse::<f32>() {
            if size > 0.0 {
                config.conch.ui.font_size = size;
            }
        }
    }
}

pub enum PreferencesAction {
    None,
    Save,
    Cancel,
}

pub fn show_preferences(ctx: &egui::Context, form: &mut PreferencesForm) -> PreferencesAction {
    let mut action = PreferencesAction::None;

    egui::Window::new("Preferences")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([550.0, 420.0])
        .show(ctx, |ui| {
            let avail = ui.available_size();

            // Main content: left section list + right form.
            ui.horizontal(|ui| {
                // Left column — section list.
                ui.vertical(|ui| {
                    ui.set_width(130.0);
                    for section in PreferencesSection::ALL {
                        let selected = form.active_section == section;
                        if ui
                            .add(egui::SelectableLabel::new(selected, section.label()))
                            .clicked()
                        {
                            form.active_section = section;
                        }
                    }
                });

                ui.separator();

                // Right column — section content.
                ui.vertical(|ui| {
                    ui.set_min_width(avail.x - 150.0);
                    match form.active_section {
                        PreferencesSection::Terminal => show_terminal_section(ui, form),
                        PreferencesSection::Appearance => show_appearance_section(ui, form),
                        PreferencesSection::General => show_general_section(ui),
                    }
                });
            });

            // Bottom buttons.
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::ui::widgets::dialog_button(ui, "Save").clicked() {
                        action = PreferencesAction::Save;
                    }
                    if crate::ui::widgets::dialog_button(ui, "Cancel").clicked() {
                        action = PreferencesAction::Cancel;
                    }
                });
            });
        });

    action
}

fn show_terminal_section(ui: &mut egui::Ui, form: &mut PreferencesForm) {
    use crate::ui::widgets::text_edit;
    egui::Grid::new("prefs_terminal")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Font Family:");
            ui.add(text_edit(&mut form.terminal_font_family).desired_width(200.0));
            ui.end_row();

            ui.label("Font Size:");
            ui.add(text_edit(&mut form.terminal_font_size).desired_width(60.0));
            ui.end_row();

            ui.label("Color Theme:");
            egui::ComboBox::from_id_salt("theme_combo")
                .selected_text(&form.theme_name)
                .show_ui(ui, |ui| {
                    for theme in &form.available_themes {
                        ui.selectable_value(&mut form.theme_name, theme.clone(), theme);
                    }
                });
            ui.end_row();

            ui.separator();
            ui.separator();
            ui.end_row();

            ui.label("Shell Program:");
            ui.add(text_edit(&mut form.shell_program).desired_width(200.0).hint_text("$SHELL"));
            ui.end_row();

            ui.label("Shell Args:");
            ui.add(text_edit(&mut form.shell_args).desired_width(200.0).hint_text("-l"));
            ui.end_row();

            ui.label("Startup Command:");
            ui.add(text_edit(&mut form.startup_command).desired_width(200.0));
            ui.end_row();

            ui.label("Use tmux:");
            ui.checkbox(&mut form.use_tmux, "Attach/create tmux session on startup");
            ui.end_row();
        });
}

fn show_appearance_section(ui: &mut egui::Ui, form: &mut PreferencesForm) {
    use crate::ui::widgets::text_edit;
    egui::Grid::new("prefs_appearance")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("UI Font Family:");
            ui.add(text_edit(&mut form.ui_font_family).desired_width(200.0));
            ui.end_row();

            ui.label("UI Font Size:");
            ui.add(text_edit(&mut form.ui_font_size).desired_width(60.0));
            ui.end_row();

            ui.separator();
            ui.separator();
            ui.end_row();

            ui.label("Window Columns:");
            ui.add(text_edit(&mut form.window_columns).desired_width(60.0));
            ui.end_row();

            ui.label("Window Lines:");
            ui.add(text_edit(&mut form.window_lines).desired_width(60.0));
            ui.end_row();
        });

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Window size takes effect on next launch.")
            .italics()
            .weak()
            .size(11.0),
    );
}

fn show_general_section(ui: &mut egui::Ui) {
    let config_dir = config::config_dir();
    ui.horizontal(|ui| {
        ui.label("Config Directory:");
        ui.label(config_dir.display().to_string());
    });
    ui.add_space(8.0);
    if ui.button("Open Config Folder").clicked() {
        let _ = std::process::Command::new("open")
            .arg(&config_dir)
            .spawn();
    }
}
