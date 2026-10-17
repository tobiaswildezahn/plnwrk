//! plnwrk desktop app: opens the window, provides the system file dialogs and hands everything
//! to `plnwrk_ui`. A file given on the command line is opened right away.

// A window program on Windows: no console window next to it (closing that would end plnwrk
// and lose what is typed).
#![cfg_attr(windows, windows_subsystem = "windows")]

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

    fn pick_export(&mut self, suggested_name: &str, extension: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter(extension, &[extension])
            .set_file_name(suggested_name)
            .save_file()
    }
}

fn main() -> ExitCode {
    let file = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(plnwrk_ui::APP_NAME)
            .with_icon(std::sync::Arc::new(eframe::egui::IconData {
                rgba: include_bytes!("../assets/icon-128.rgba").to_vec(),
                width: 128,
                height: 128,
            }))
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([720.0, 480.0]),
        event_loop_builder: Some(Box::new(|builder| {
            // winit's default menu quits on Cmd+Q without a close request, which would skip
            // the question about unsaved changes; plnwrk handles Cmd+Q itself.
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::EventLoopBuilderExtMacOS as _;
                builder.with_default_menu(false);
            }
            let _ = builder;
        })),
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
