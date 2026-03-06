#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fonts;
mod icons;
mod input;
#[cfg(target_os = "macos")]
mod macos_menu;
mod state;
mod terminal;
mod ui;

use std::sync::Arc;

use app::ConchApp;
use conch_core::config;

fn load_app_icon() -> egui::IconData {
    let img = image::load_from_memory(include_bytes!("../icons/app-icon.png"))
        .expect("Failed to decode app icon")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}

/// Convert character-cell dimensions to pixel size for the initial window.
///
/// Uses rough estimates for cell size and UI chrome (tab bar, title bar padding).
/// The terminal will resize itself to fit once actual font metrics are measured.
fn window_size_from_config(cfg: &config::WindowDimensions) -> [f32; 2] {
    let cols = if cfg.columns == 0 { 150 } else { cfg.columns };
    let lines = if cfg.lines == 0 { 50 } else { cfg.lines };

    // Approximate cell size before font metrics are measured.
    let cell_w: f32 = 8.0;
    let cell_h: f32 = 16.0;
    // Extra chrome: tab bar (~30px), sidebar padding, window margins.
    let chrome_w: f32 = 40.0;
    let chrome_h: f32 = 50.0;

    [
        (cols as f32 * cell_w + chrome_w).max(600.0),
        (lines as f32 * cell_h + chrome_h).max(400.0),
    ]
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    // Load config early so we can size the window before creating the app.
    config::migrate_if_needed();
    let user_config = config::load_user_config().unwrap_or_default();
    let window_size = window_size_from_config(&user_config.window.dimensions);

    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime"),
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(window_size)
            .with_title_shown(true)
            .with_titlebar_shown(true)
            .with_icon(Arc::new(load_app_icon())),
        ..Default::default()
    };

    eframe::run_native(
        "Conch",
        options,
        Box::new(move |_cc| Ok(Box::new(ConchApp::new(rt)))),
    )
}
