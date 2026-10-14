//! Project tooling: `cargo xtask ci` runs every quality gate of docs/PLAN.md §2.

mod ci;
mod layers;
mod purity;

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "ci" => ci::all(),
        "layers" => ci::layers(),
        "purity" => ci::purity(),
        _ => Err("usage: cargo xtask <ci|layers|purity>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}
