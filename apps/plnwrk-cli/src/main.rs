//! plnwrk command line. Phase 0: prints its version.

use std::io::Write as _;

fn main() -> std::process::ExitCode {
    let mut out = std::io::stdout().lock();
    match writeln!(out, "plnwrk-cli {}", env!("CARGO_PKG_VERSION")) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::FAILURE,
    }
}
