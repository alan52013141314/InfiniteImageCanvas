#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod i18n;
mod importer;
mod media;
mod model;
#[cfg(windows)]
mod native_input;
mod storage;
mod theme;
fn main() {
    if let Err(error) = run() {
        rfd::MessageDialog::new()
            .set_title("InfiniteImageCanvas — Startup error / 啟動錯誤")
            .set_description(format!(
                "{error}\nOpenGL 3.3 support is required. / 請確認顯示驅動程式支援 OpenGL 3.3。"
            ))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
fn run() -> eframe::Result {
    let directory = std::env::current_exe()
        .expect("exe path")
        .parent()
        .unwrap()
        .join("save file");
    let initial = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        &format!("InfiniteImageCanvas v{}", env!("CARGO_PKG_VERSION")),
        options,
        Box::new(move |cc| Ok(Box::new(app::Workspace::new(cc, directory, initial)))),
    )
}
