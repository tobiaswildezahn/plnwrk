//! plnwrk desktop app: opens the window, provides the system file dialogs and hands everything
//! to `plnwrk_ui`. A file given on the command line is opened right away.

use std::path::PathBuf;
use std::process::ExitCode;

/// The file dialogs of the operating system.
struct SystemDialogs {
    /// Folder of the last file chosen, to start the next dialog there.
    folder: Option<PathBuf>,
}

impl SystemDialogs {
    fn dialog(&self) -> rfd::FileDialog {
        let dialog = rfd::FileDialog::new().add_filter("plnwrk", &["plnwrk"]);
        match &self.folder {
            Some(folder) => dialog.set_directory(folder),
            None => dialog,
        }
    }

    fn remember(&mut self, path: Option<PathBuf>) -> Option<PathBuf> {
        if let Some(folder) = path.as_deref().and_then(std::path::Path::parent) {
            self.folder = Some(folder.to_path_buf());
        }
        path
    }
}

impl plnwrk_ui::Host for SystemDialogs {
    fn pick_open(&mut self) -> Option<PathBuf> {
        let path = self.dialog().pick_file();
        self.remember(path)
    }

    fn pick_save(&mut self, suggested_name: &str) -> Option<PathBuf> {
        let path = self.dialog().set_file_name(suggested_name).save_file();
        self.remember(path)
    }
}

fn main() -> ExitCode {
    let file = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(plnwrk_ui::APP_NAME)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        plnwrk_ui::APP_NAME,
        options,
        Box::new(move |cc| {
            let mut app = plnwrk_ui::PlnwrkApp::new(cc, Box::new(SystemDialogs { folder: None }));
            if let Some(file) = file {
                app.open_path(file);
            }
            Ok(Box::new(app))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("plnwrk could not start: {error}");
            ExitCode::FAILURE
        }
    }
}
