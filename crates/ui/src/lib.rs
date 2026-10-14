//! egui user interface of plnwrk (layer 4), shared by the desktop and web apps.
//!
//! See `docs/PLAN.md` §3.6.

/// Name shown in the window title and the app header.
pub const APP_NAME: &str = "plnwrk";

/// The whole application state of the user interface.
#[derive(Debug, Default)]
pub struct PlnwrkApp {}

impl PlnwrkApp {
    /// Creates the app when the desktop or web shell starts it.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Draws one frame. Kept apart from [`eframe::App`] so tests can run it without a window.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.heading(APP_NAME);
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(8.0);
                ui.label("Fundament steht. Projekte und Vorgänge folgen in Phase 1.");
            });
        });
    }
}

impl eframe::App for PlnwrkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(output: &egui::FullOutput) -> String {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Runs two frames without a window (the first one only measures) and returns the second.
    fn render(app: &mut PlnwrkApp) -> egui::FullOutput {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show(ui));
        output.textures_delta.clear();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.show(ui));
        output.textures_delta.clear();
        output
    }

    #[test]
    fn shows_name_and_version_without_a_window() {
        let shown = texts(&render(&mut PlnwrkApp::default()));
        assert!(shown.contains(APP_NAME), "{shown}");
        assert!(shown.contains(env!("CARGO_PKG_VERSION")), "{shown}");
    }
}
