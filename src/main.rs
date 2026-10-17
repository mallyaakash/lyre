#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod app;
pub mod audio;
pub mod config;
pub mod tray;
pub mod ui;

use eframe::egui::{self, ViewportBuilder};

use crate::app::LyreApp;
use crate::audio::AudioEngine;
use crate::config::load_or_init_config;
use crate::tray::{load_embedded_egui_icon, TrayManager};

fn main() -> eframe::Result<()> {
    let _ = env_logger::try_init();

    // 1. Initialize Windows Core Audio COM engine & enumerate active Render/Capture endpoints
    let audio = AudioEngine::new().expect("Failed to initialize Windows Core Audio COM engine");

    // 2. Load configuration from %APPDATA%/Lyre/config.json (or auto-seed buttons on first run)
    let mut app_config = load_or_init_config(audio.snapshot());
    app_config.window.visible = true;

    // 3. Initialize Windows System Tray icon & native right-click context menu
    let tray_manager = TrayManager::new(app_config.window.always_on_top).ok();

    // 4. Configure eframe Viewport (hidden from Windows Taskbar, frameless in Minimalist Mode)
    let mut viewport = ViewportBuilder::default()
        .with_title("Lyre — Audio Switcher")
        .with_app_id("com.lyre.audioswitcher")
        .with_taskbar(false)
        .with_decorations(!app_config.window.minimalist_mode)
        .with_inner_size([app_config.window.width, app_config.window.height])
        .with_min_inner_size([110.0, 68.0])
        .with_visible(true);

    if app_config.window.always_on_top {
        viewport = viewport.with_always_on_top();
    }

    if let (Some(x), Some(y)) = (app_config.window.x, app_config.window.y) {
        viewport = viewport.with_position(egui::pos2(x, y));
    }

    if let Some(icon) = load_embedded_egui_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Lyre",
        native_options,
        Box::new(move |cc| Ok(Box::new(LyreApp::new(cc, audio, app_config, tray_manager)))),
    )
}
