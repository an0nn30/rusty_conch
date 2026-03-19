//! Color theme loading — converts Alacritty .toml themes to CSS-compatible values.

use serde::Serialize;

use conch_core::config::UserConfig;

#[derive(Serialize)]
pub(crate) struct ThemeColors {
    pub background: String,
    pub foreground: String,
    pub cursor_text: String,
    pub cursor_color: String,
    pub selection_text: String,
    pub selection_bg: String,
    pub black: String, pub red: String, pub green: String, pub yellow: String,
    pub blue: String, pub magenta: String, pub cyan: String, pub white: String,
    pub bright_black: String, pub bright_red: String, pub bright_green: String, pub bright_yellow: String,
    pub bright_blue: String, pub bright_magenta: String, pub bright_cyan: String, pub bright_white: String,
    pub dim_fg: String,
    pub panel_bg: String,
    pub tab_bar_bg: String,
    pub tab_border: String,
    pub active_highlight: String,
}

fn darken(hex: &str, amount: i32) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 { return format!("#{hex}"); }
    let r = i32::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = i32::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = i32::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    format!("#{:02x}{:02x}{:02x}",
        (r - amount).clamp(0, 255),
        (g - amount).clamp(0, 255),
        (b - amount).clamp(0, 255))
}

fn lighten(hex: &str, amount: i32) -> String {
    darken(hex, -amount)
}

pub(crate) fn resolve_theme_colors(config: &UserConfig) -> ThemeColors {
    let scheme = conch_core::color_scheme::resolve_theme(&config.colors.theme);

    let bg = &scheme.primary.background;
    let fg = &scheme.primary.foreground;
    let cursor = scheme.cursor.as_ref();
    let selection = scheme.selection.as_ref();

    ThemeColors {
        background: bg.clone(),
        foreground: fg.clone(),
        cursor_text: cursor.map(|c| c.text.clone()).unwrap_or_else(|| bg.clone()),
        cursor_color: cursor.map(|c| c.cursor.clone()).unwrap_or_else(|| fg.clone()),
        selection_text: selection.map(|s| s.text.clone()).unwrap_or_else(|| fg.clone()),
        selection_bg: selection.map(|s| s.background.clone()).unwrap_or_else(|| lighten(bg, 30)),
        black: scheme.normal.black.clone(),
        red: scheme.normal.red.clone(),
        green: scheme.normal.green.clone(),
        yellow: scheme.normal.yellow.clone(),
        blue: scheme.normal.blue.clone(),
        magenta: scheme.normal.magenta.clone(),
        cyan: scheme.normal.cyan.clone(),
        white: scheme.normal.white.clone(),
        bright_black: scheme.bright.black.clone(),
        bright_red: scheme.bright.red.clone(),
        bright_green: scheme.bright.green.clone(),
        bright_yellow: scheme.bright.yellow.clone(),
        bright_blue: scheme.bright.blue.clone(),
        bright_magenta: scheme.bright.magenta.clone(),
        bright_cyan: scheme.bright.cyan.clone(),
        bright_white: scheme.bright.white.clone(),
        dim_fg: scheme.primary.dim_foreground.clone().unwrap_or_else(|| lighten(bg, 60)),
        panel_bg: darken(bg, 8),
        tab_bar_bg: darken(bg, 14),
        tab_border: lighten(bg, 18),
        active_highlight: lighten(bg, 28),
    }
}
