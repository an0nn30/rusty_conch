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
    pub input_bg: String,
    pub active_highlight: String,
}

fn darken(hex: &str, amount: i32) -> String {
    let hex = hex.trim_start_matches('#');
    // Expand 3-char shorthand (#fff -> ffffff)
    let hex = if hex.len() == 3 {
        let b: Vec<u8> = hex.bytes().collect();
        format!(
            "{0}{0}{1}{1}{2}{2}",
            b[0] as char, b[1] as char, b[2] as char
        )
    } else if hex.len() < 6 {
        return format!("#{hex}");
    } else {
        hex.to_string()
    };
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

/// Resolve theme colors from a pre-loaded ColorScheme (no config needed).
pub(crate) fn resolve_theme_colors_from_scheme(scheme: &conch_core::color_scheme::ColorScheme) -> ThemeColors {
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
        input_bg: lighten(bg, 10),
        active_highlight: lighten(bg, 28),
    }
}

pub(crate) fn resolve_theme_colors(config: &UserConfig) -> ThemeColors {
    let scheme = conch_core::color_scheme::resolve_theme(&config.colors.theme);
    resolve_theme_colors_from_scheme(&scheme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conch_core::color_scheme::ColorScheme;

    #[test]
    fn resolve_from_scheme_uses_primary_colors() {
        let scheme = ColorScheme::default(); // Dracula
        let tc = resolve_theme_colors_from_scheme(&scheme);
        assert_eq!(tc.background, "#282a36");
        assert_eq!(tc.foreground, "#f8f8f2");
    }

    #[test]
    fn resolve_from_scheme_derives_panel_colors() {
        let scheme = ColorScheme::default();
        let tc = resolve_theme_colors_from_scheme(&scheme);
        // panel_bg should be darker than background
        assert_ne!(tc.panel_bg, tc.background);
        // tab_bar_bg should be darker than panel_bg
        assert_ne!(tc.tab_bar_bg, tc.panel_bg);
        // input_bg should be lighter than background
        assert_ne!(tc.input_bg, tc.background);
    }

    #[test]
    fn resolve_from_scheme_maps_ansi_colors() {
        let scheme = ColorScheme::default();
        let tc = resolve_theme_colors_from_scheme(&scheme);
        assert_eq!(tc.red, "#ff5555");
        assert_eq!(tc.green, "#50fa7b");
        assert_eq!(tc.bright_red, "#ff6e6e");
        assert_eq!(tc.bright_green, "#69ff94");
    }

    #[test]
    fn resolve_from_scheme_handles_cursor_colors() {
        let scheme = ColorScheme::default(); // has cursor colors
        let tc = resolve_theme_colors_from_scheme(&scheme);
        assert_eq!(tc.cursor_text, "#282a36");
        assert_eq!(tc.cursor_color, "#f8f8f2");
    }

    #[test]
    fn resolve_from_scheme_fallback_when_no_cursor() {
        let mut scheme = ColorScheme::default();
        scheme.cursor = None;
        let tc = resolve_theme_colors_from_scheme(&scheme);
        // Falls back to bg/fg
        assert_eq!(tc.cursor_text, scheme.primary.background);
        assert_eq!(tc.cursor_color, scheme.primary.foreground);
    }

    #[test]
    fn darken_expands_three_char_hex() {
        // #fff should expand to #ffffff then darken by 10
        let result = darken("#fff", 10);
        assert_eq!(result, "#f5f5f5");
    }

    #[test]
    fn lighten_expands_three_char_hex() {
        let result = lighten("#000", 10);
        assert_eq!(result, "#0a0a0a");
    }
}
