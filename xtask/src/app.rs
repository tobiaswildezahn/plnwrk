//! `cargo xtask app`: builds the desktop app in release mode and, on macOS, wraps it as
//! `target/plnwrk.app`, which starts with a double click and can be moved to Programme.

use std::fs;
use std::path::Path;

use crate::ci::{cargo, root, step};

const INFO_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>plnwrk</string>
  <key>CFBundleDisplayName</key><string>plnwrk</string>
  <key>CFBundleIdentifier</key><string>app.plnwrk.desktop</string>
  <key>CFBundleExecutable</key><string>plnwrk</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>VERSION</string>
  <key>CFBundleShortVersionString</key><string>VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#;

pub fn build() -> Result<(), String> {
    step("build (release)");
    cargo(&["build", "--release", "--locked", "-p", "plnwrk"])?;
    let target = root().join("target");
    let binary = target.join("release").join(binary_name());
    if !cfg!(target_os = "macos") {
        eprintln!("\nfertig: {}", binary.display());
        return Ok(());
    }
    step("bundle (macOS)");
    let bundle = target.join("plnwrk.app");
    let _ = fs::remove_dir_all(&bundle);
    let macos = bundle.join("Contents").join("MacOS");
    fs::create_dir_all(&macos).map_err(|e| format!("{}: {e}", macos.display()))?;
    copy(&binary, &macos.join("plnwrk"))?;
    let plist = INFO_PLIST.replace("VERSION", env!("CARGO_PKG_VERSION"));
    let plist_path = bundle.join("Contents").join("Info.plist");
    fs::write(&plist_path, plist).map_err(|e| format!("{}: {e}", plist_path.display()))?;
    eprintln!(
        "\nfertig: {} (Doppelklick startet plnwrk)",
        bundle.display()
    );
    Ok(())
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "plnwrk.exe"
    } else {
        "plnwrk"
    }
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} → {}: {e}", from.display(), to.display()))
}
