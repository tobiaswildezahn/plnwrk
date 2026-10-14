//! The quality gate. Each step prints its name and stops the gate on failure.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{layers, purity};

/// Crates that only run natively. Every other workspace crate must build for the browser
/// (docs/PLAN.md §2, §3.1), so a new crate is checked for wasm32 unless it is listed here.
const NATIVE_ONLY: &[&str] = &["plnwrk-mcp", "plnwrk", "plnwrk-cli", "xtask"];

/// Pure crates and the share of their lines the tests must cover (docs/PLAN.md §7 Phase 1).
const COVERAGE: (&[&str], &str) = (&["plnwrk-core", "plnwrk-engine"], "90");

/// Never-crash lints, forbidden on the command line so they hold for every crate regardless of
/// its manifest (docs/PLAN.md §2).
const FORBID: &[&str] = &[
    "unsafe_code",
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
    "clippy::indexing_slicing",
    "clippy::string_slice",
    "clippy::unreachable",
    "clippy::todo",
    "clippy::unimplemented",
    "clippy::exit",
    "clippy::disallowed_methods",
    "clippy::disallowed_types",
];

fn cargo_bin() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn step(name: &str) {
    eprintln!("\n== {name}");
}

fn cargo<S: AsRef<str>>(args: &[S]) -> Result<(), String> {
    let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
    let status = Command::new(cargo_bin())
        .args(&args)
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
    let ok = Command::new(cargo_bin())
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

fn metadata() -> Result<Vec<layers::Package>, String> {
    let output = Command::new(cargo_bin())
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root())
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed (is Cargo.lock up to date?): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let json = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    layers::parse_metadata(&json)
}

pub fn layers() -> Result<(), String> {
    step("layering and dependencies of pure crates");
    report(layers::check(&metadata()?))
}

pub fn purity() -> Result<(), String> {
    step("purity of core and engine (text backstop; clippy.toml is the precise check)");
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

/// `-- -D warnings -F <lint> …` for clippy.
fn lint_flags() -> Vec<String> {
    let mut flags = vec!["--".to_owned(), "-D".to_owned(), "warnings".to_owned()];
    for lint in FORBID {
        flags.push("-F".to_owned());
        flags.push((*lint).to_owned());
    }
    flags
}

pub fn all() -> Result<(), String> {
    step("tools");
    require_tool(&["deny", "--version"], "cargo install --locked cargo-deny")?;
    require_tool(
        &["llvm-cov", "--version"],
        "cargo install --locked cargo-llvm-cov (and: rustup component add llvm-tools)",
    )?;
    step("format");
    cargo(&["fmt", "--all", "--check"])?;
    let packages = metadata()?;
    layers()?;
    purity()?;

    step("clippy (native)");
    let mut native: Vec<String> = ["clippy", "--workspace", "--all-targets", "--locked"]
        .map(String::from)
        .to_vec();
    native.extend(lint_flags());
    cargo(&native)?;

    step("clippy (wasm32)");
    let mut wasm: Vec<String> = ["clippy", "--target", "wasm32-unknown-unknown", "--locked"]
        .map(String::from)
        .to_vec();
    for package in packages
        .iter()
        .filter(|p| !NATIVE_ONLY.contains(&p.name.as_str()))
    {
        wasm.push("-p".to_owned());
        wasm.push(package.name.clone());
    }
    wasm.extend(lint_flags());
    cargo(&wasm)?;

    step("build (wasm32 web app)");
    cargo(&[
        "build",
        "--target",
        "wasm32-unknown-unknown",
        "--locked",
        "-p",
        "plnwrk-web",
    ])?;

    step("tests");
    cargo(&["test", "--workspace", "--locked"])?;

    step("coverage of the pure crates");
    let mut coverage = vec![
        "llvm-cov",
        "--locked",
        "--summary-only",
        "--fail-under-lines",
        COVERAGE.1,
    ];
    for krate in COVERAGE.0 {
        coverage.extend(["-p", krate]);
    }
    cargo(&coverage)?;

    step("dependencies (cargo-deny)");
    cargo(&["deny", "--locked", "check"])?;
    eprintln!("\nall gates passed");
    Ok(())
}
