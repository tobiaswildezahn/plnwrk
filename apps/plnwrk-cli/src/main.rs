//! plnwrk command line. Phase 0: `--version`.

use std::io::Write as _;
use std::process::ExitCode;

const USAGE: &str = "usage: plnwrk-cli --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (text, code) = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--version" | "-V"] => (
            format!("plnwrk-cli {}", env!("CARGO_PKG_VERSION")),
            ExitCode::SUCCESS,
        ),
        ["--help" | "-h"] => (USAGE.to_owned(), ExitCode::SUCCESS),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match writeln!(std::io::stdout().lock(), "{text}") {
        Ok(()) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
