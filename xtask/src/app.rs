//! `cargo xtask app`: builds the desktop app in release mode and, on macOS, wraps it as
//! `target/plnwrk.app`, which starts with a double click and can be moved to Programme.
//! `cargo xtask app --universal` (macOS): one app for Apple silicon and Intel, as released.

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
  <key>CFBundleIconFile</key><string>plnwrk</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>VERSION</string>
  <key>CFBundleShortVersionString</key><string>VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#;

/// The two Mac targets of a universal app.
const MAC_TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

pub fn build(universal: bool) -> Result<(), String> {
    if universal && !cfg!(target_os = "macos") {
        return Err("--universal builds a macOS app and works only on a Mac".to_owned());
    }
    let target = root().join("target");
    let binary = if universal {
        step("build (release, universal)");
        let mut parts = Vec::new();
        for triple in MAC_TARGETS {
            cargo(&[
                "build",
                "--release",
                "--locked",
                "-p",
                "plnwrk",
                "--target",
                triple,
            ])
            .map_err(|e| format!("{e} (target missing? rustup target add {triple})"))?;
            parts.push(target.join(triple).join("release").join("plnwrk"));
        }
        let joined = target.join("plnwrk-universal");
        let status = std::process::Command::new("lipo")
            .arg("-create")
            .args(&parts)
            .arg("-output")
            .arg(&joined)
            .status()
            .map_err(|e| format!("lipo: {e}"))?;
        if !status.success() {
            return Err("lipo failed".to_owned());
        }
        joined
    } else {
        step("build (release)");
        cargo(&["build", "--release", "--locked", "-p", "plnwrk"])?;
        target.join("release").join(binary_name())
    };
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
    let resources = bundle.join("Contents").join("Resources");
    fs::create_dir_all(&resources).map_err(|e| format!("{}: {e}", resources.display()))?;
    copy(
        &root().join("apps/plnwrk/assets/plnwrk.icns"),
        &resources.join("plnwrk.icns"),
    )?;
    let plist = INFO_PLIST.replace("VERSION", env!("CARGO_PKG_VERSION"));
    let plist_path = bundle.join("Contents").join("Info.plist");
    fs::write(&plist_path, plist).map_err(|e| format!("{}: {e}", plist_path.display()))?;
    // Sealed ad hoc (no certificate): resources and Info.plist belong to the signature, so
    // macOS treats a downloaded copy as unsigned rather than as damaged.
    let status = std::process::Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&bundle)
        .status()
        .map_err(|e| format!("codesign: {e}"))?;
    if !status.success() {
        return Err("codesign failed".to_owned());
    }
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
