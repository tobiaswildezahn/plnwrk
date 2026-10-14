//! plnwrk desktop app: opens the window and hands it to `plnwrk_ui`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(plnwrk_ui::APP_NAME)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([480.0, 320.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        plnwrk_ui::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(plnwrk_ui::PlnwrkApp::new(cc)))),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("plnwrk could not start: {error}");
            ExitCode::FAILURE
        }
    }
}
