//! Centralized UI theme engine.
//!
//! Generates a consistent visual style across the entire application from
//! the terminal color scheme. Every widget — built-in egui or custom —
//! pulls its colors and metrics from here.
//!
//! Designed for plugin SDK access: plugins will be able to query the
//! theme to render UI that matches the host application.

use conch_core::config::AppearanceMode;
use egui::{
    Color32, CornerRadius, Shadow, Stroke, Visuals,
    style::{WidgetVisuals, Widgets},
};

use crate::terminal::color::ResolvedColors;

/// Pre-computed UI theme derived from the terminal color scheme.
///
/// Created once (and recreated on theme change), then shared by reference
/// across all rendering code.
#[derive(Debug, Clone)]
pub struct UiTheme {
    // ── Core palette ──
    /// Terminal background.
    pub bg: Color32,
    /// Slightly elevated surface (panels, tab bar, menu bar).
    pub surface: Color32,
    /// More elevated surface (active tab, hovered items).
    pub surface_raised: Color32,
    /// Highest elevation surface (pressed/active elements).
    pub surface_top: Color32,
    /// Primary text color (terminal foreground).
    pub text: Color32,
    /// Secondary/dimmed text.
    pub text_secondary: Color32,
    /// Very dim text or subtle dividers.
    pub text_muted: Color32,
    /// Accent color for highlights, active indicators, links.
    pub accent: Color32,
    /// Divider/border color.
    pub border: Color32,
    /// Warning color.
    pub warn: Color32,
    /// Error color.
    pub error: Color32,

    // ── Metrics ──
    /// Standard corner rounding for widgets.
    pub rounding: u8,
    /// Small font size (labels, tab titles).
    pub font_small: f32,
    /// Normal font size (body text, buttons).
    pub font_normal: f32,
    /// Minimum width for menu popups (menu bar and context menus).
    pub menu_width: f32,

    // ── Mode ──
    /// Whether we are in dark mode.
    pub dark_mode: bool,
}

impl UiTheme {
    /// Build a theme from the terminal color scheme and appearance mode.
    pub fn from_colors(colors: &ResolvedColors, appearance: AppearanceMode) -> Self {
        let bg = to_color32(colors.background);
        let fg = to_color32(colors.foreground);

        let bg_r = bg.r();
        let bg_g = bg.g();
        let bg_b = bg.b();

        // For System mode, infer dark/light from background luminance.
        let dark_mode = match appearance {
            AppearanceMode::Dark => true,
            AppearanceMode::Light => false,
            AppearanceMode::System => {
                let luminance = 0.299 * bg_r as f32 + 0.587 * bg_g as f32 + 0.114 * bg_b as f32;
                luminance < 128.0
            }
        };

        // Surface offsets: lighten in dark mode, darken in light mode.
        let surface = offset_color(bg, dark_mode, 15);
        let surface_raised = offset_color(bg, dark_mode, 30);
        let surface_top = offset_color(bg, dark_mode, 45);
        let border = offset_color(bg, dark_mode, 50);

        Self {
            bg,
            surface,
            surface_raised,
            surface_top,
            text: fg,
            text_secondary: Color32::from_rgb(
                (colors.foreground[0] * 180.0) as u8,
                (colors.foreground[1] * 180.0) as u8,
                (colors.foreground[2] * 180.0) as u8,
            ),
            text_muted: Color32::from_rgb(
                (colors.foreground[0] * 90.0) as u8,
                (colors.foreground[1] * 90.0) as u8,
                (colors.foreground[2] * 90.0) as u8,
            ),
            accent: to_color32(colors.normal[4]), // blue
            border,
            warn: to_color32(colors.normal[3]),  // yellow
            error: to_color32(colors.normal[1]), // red
            rounding: 0,
            font_small: 11.0,
            font_normal: 13.0,
            menu_width: 120.0,
            dark_mode,
        }
    }

    /// Generate egui `Visuals` from this theme so built-in widgets
    /// (buttons, menus, text inputs, etc.) automatically match.
    pub fn to_visuals(&self) -> Visuals {
        let rounding = CornerRadius::same(self.rounding);

        let noninteractive = WidgetVisuals {
            bg_fill: self.surface,
            weak_bg_fill: self.surface,
            bg_stroke: Stroke::new(1.0, self.border),
            corner_radius: rounding,
            fg_stroke: Stroke::new(1.0, self.text_secondary),
            expansion: 0.0,
        };

        let inactive = WidgetVisuals {
            bg_fill: self.surface,
            weak_bg_fill: self.surface,
            bg_stroke: Stroke::new(1.0, self.border),
            corner_radius: rounding,
            fg_stroke: Stroke::new(1.0, self.text),
            expansion: 0.0,
        };

        let hovered = WidgetVisuals {
            bg_fill: self.surface_raised,
            weak_bg_fill: self.surface_raised,
            bg_stroke: Stroke::new(1.0, self.accent),
            corner_radius: rounding,
            fg_stroke: Stroke::new(1.5, self.text),
            expansion: 0.0,
        };

        let active = WidgetVisuals {
            bg_fill: self.surface_top,
            weak_bg_fill: self.surface_top,
            bg_stroke: Stroke::new(1.0, self.accent),
            corner_radius: rounding,
            fg_stroke: Stroke::new(2.0, self.text),
            expansion: 0.0,
        };

        let open = WidgetVisuals {
            bg_fill: self.surface_raised,
            weak_bg_fill: self.surface_raised,
            bg_stroke: Stroke::new(1.0, self.border),
            corner_radius: rounding,
            fg_stroke: Stroke::new(1.0, self.text),
            expansion: 0.0,
        };

        Visuals {
            dark_mode: self.dark_mode,
            override_text_color: None,
            widgets: Widgets {
                noninteractive,
                inactive,
                hovered,
                active,
                open,
            },
            selection: egui::style::Selection {
                bg_fill: self.accent.linear_multiply(0.4),
                stroke: Stroke::new(1.0, self.accent),
            },
            hyperlink_color: self.accent,
            faint_bg_color: self.surface,
            extreme_bg_color: self.bg,
            code_bg_color: self.surface,
            warn_fg_color: self.warn,
            error_fg_color: self.error,
            window_corner_radius: CornerRadius::ZERO,
            window_shadow: Shadow::NONE,
            window_fill: self.surface,
            window_stroke: Stroke::new(1.0, self.border),
            window_highlight_topmost: true,
            menu_corner_radius: CornerRadius::ZERO,
            panel_fill: self.surface,
            popup_shadow: Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: Color32::from_black_alpha(60),
            },
            resize_corner_size: 12.0,
            clip_rect_margin: 3.0,
            button_frame: true,
            collapsing_header_frame: false,
            indent_has_left_vline: true,
            striped: false,
            slider_trailing_fill: true,
            handle_shape: egui::style::HandleShape::Circle,
            interact_cursor: None,
            image_loading_spinners: true,
            numeric_color_space: egui::style::NumericColorSpace::GammaByte,
            text_cursor: Default::default(),
        }
    }

    /// Return the background color with a custom alpha (0–255).
    /// Useful for translucent overlays like the buttonless drag region.
    pub fn bg_with_alpha(&self, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(self.bg.r(), self.bg.g(), self.bg.b(), alpha)
    }

    /// Apply this theme to an egui context.
    ///
    /// Sets both `Visuals` (colors, rounding, shadows) and `Spacing`
    /// (menu width). This covers all egui popups — menu bar dropdowns
    /// and right-click context menus use the same pipeline.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals = self.to_visuals();
        style.spacing.menu_width = self.menu_width;
        ctx.set_style(style);
    }
}

/// Offset a color by `amount` — lighten in dark mode, darken in light mode.
fn offset_color(base: Color32, dark_mode: bool, amount: u8) -> Color32 {
    if dark_mode {
        Color32::from_rgb(
            base.r().saturating_add(amount),
            base.g().saturating_add(amount),
            base.b().saturating_add(amount),
        )
    } else {
        Color32::from_rgb(
            base.r().saturating_sub(amount),
            base.g().saturating_sub(amount),
            base.b().saturating_sub(amount),
        )
    }
}

/// Convert `[f32; 4]` RGBA to `Color32`.
fn to_color32(c: [f32; 4]) -> Color32 {
    Color32::from_rgb(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    )
}
