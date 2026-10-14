//! The quality gate. Each step prints its name and stops the gate on failure.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{layers, purity};

/// Crates that must build for the browser (docs/PLAN.md §2, §3.1).
const WASM_CRATES: &[&str] = &[
    "plnwrk-core",
    "plnwrk-engine",
    "plnwrk-store",
    "plnwrk-schedule",
    "plnwrk-assistant",
    "plnwrk-ui",
    "plnwrk-web",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn step(name: &str) {
    eprintln!("\n== {name}");
}

fn cargo(args: &[&str]) -> Result<(), String> {
    let status = Command::new(env!("CARGO"))
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| format!("cannot run cargo {}: {e}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {} failed", args.join(" ")))
    }
}

/// Fails with an install hint when a cargo subcommand is missing.
fn require_tool(probe: &[&str], install: &str) -> Result<(), String> {
    let ok = Command::new(env!("CARGO"))
        .args(probe)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(format!("missing tool; install it with: {install}"))
    }
}

pub fn layers() -> Result<(), String> {
    step("layering");
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root())
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed".to_owned());
    }
    let json = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    report(layers::check(&layers::parse_metadata(&json)?))
}

pub fn purity() -> Result<(), String> {
    step("purity of core and engine");
    let mut errors = Vec::new();
    for dir in purity::PURE_CRATES {
        visit_rs(&root().join(dir), &mut |path, contents| {
            errors.extend(purity::scan(path, contents));
        })?;
    }
    report(errors)
}

fn visit_rs(dir: &Path, visit: &mut dyn FnMut(&Path, &str)) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            visit_rs(&path, visit)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let contents =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            visit(&path, &contents);
        }
    }
    Ok(())
}

fn report(errors: Vec<String>) -> Result<(), String> {
    if errors.is_empty() {
        return Ok(());
    }
    for error in &errors {
        eprintln!("  {error}");
    }
    Err(format!("{} violation(s)", errors.len()))
}

pub fn all() -> Result<(), String> {
    step("format");
    cargo(&["fmt", "--all", "--check"])?;
    layers()?;
    purity()?;
    step("clippy (native)");
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ])?;
    step("clippy (wasm32)");
    let mut wasm = vec!["clippy", "--target", "wasm32-unknown-unknown", "--locked"];
    for krate in WASM_CRATES {
        wasm.extend(["-p", krate]);
    }
    wasm.extend(["--", "-D", "warnings"]);
    cargo(&wasm)?;
    step("tests");
    cargo(&["test", "--workspace", "--locked"])?;
    step("dependencies (cargo-deny)");
    require_tool(&["deny", "--version"], "cargo install --locked cargo-deny")?;
    cargo(&["deny", "--all-features", "check"])?;
    eprintln!("\nall gates passed");
    Ok(())
}
