#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod export;
mod format;
mod localization;
mod model;

use app::RppApp;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RPP — Ran Plan Plan")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native("RPP", options, Box::new(|cc| Ok(Box::new(RppApp::new(cc)))))
}
