//! plnwrk command line. Every answer is JSON on standard output, made for people and agents.

mod system;

use std::io::Write as _;
use std::process::ExitCode;

use clap::{Arg, ArgMatches, Command};
use plnwrk_engine::query::{query_registry, search_queries};
use plnwrk_engine::{CommandSpec, Session, fixture, registry, search};
use serde_json::{Value, json};

/// The command line definition (clap's builder API: its derive macro emits `#[allow]`s that
/// conflict with the forbidden lints of this workspace).
fn cli() -> Command {
    let search = || {
        Arg::new("search")
            .long("search")
            .value_name("WORDS")
            .help("Only entries matching these words")
    };
    Command::new("plnwrk-cli")
        .version(env!("CARGO_PKG_VERSION"))
        .about("plnwrk – project and case management. Prints JSON.")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("commands")
                .about("Lists every command with its parameters (JSON schema)")
                .arg(search()),
        )
        .subcommand(
            Command::new("queries")
                .about("Lists every query with its parameters (JSON schema)")
                .arg(search()),
        )
        .subcommand(Command::new("demo").about(
            "Builds the sample workspace in memory and shows overview, board and overdue items",
        ))
}

fn command_json(spec: &CommandSpec) -> Value {
    json!({"id": spec.id, "summary": spec.summary, "input_schema": spec.input_schema()})
}

fn run(matches: &ArgMatches) -> Result<Value, String> {
    let words = |m: &ArgMatches| m.get_one::<String>("search").cloned();
    match matches.subcommand() {
        Some(("commands", sub)) => {
            let specs: Vec<&CommandSpec> = match words(sub) {
                Some(words) => search(&words),
                None => registry().iter().collect(),
            };
            Ok(Value::from(
                specs.into_iter().map(command_json).collect::<Vec<_>>(),
            ))
        }
        Some(("queries", sub)) => {
            let specs = match words(sub) {
                Some(words) => search_queries(&words),
                None => query_registry().iter().collect(),
            };
            Ok(Value::from(
                specs
                    .into_iter()
                    .map(|q| json!({"name": q.name, "summary": q.summary, "input_schema": q.input_schema()}))
                    .collect::<Vec<_>>(),
            ))
        }
        Some(("demo", _)) => {
            let mut session = Session::new(
                Box::new(system::SystemClock),
                Box::new(system::RandomIds::default()),
            );
            let sample = fixture::sample(&mut session).map_err(|e| e.to_string())?;
            let project = json!({"project": sample.project.to_string()});
            let query =
                |name: &str, params: &Value| session.query(name, params).map_err(|e| e.to_string());
            Ok(json!({
                "overview": query("overview", &json!({}))?,
                "board": query("board", &project)?,
                "overdue": query("overdue", &json!({}))?,
                "case": query("project", &json!({"project": sample.case.to_string()}))?,
            }))
        }
        _ => Err("unknown subcommand; see --help".to_owned()),
    }
}

fn main() -> ExitCode {
    let matches = cli().get_matches();
    match run(&matches) {
        Ok(value) => {
            let text = serde_json::to_string_pretty(&value).unwrap_or_default();
            match writeln!(std::io::stdout().lock(), "{text}") {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::FAILURE,
            }
        }
        Err(message) => {
            eprintln!("plnwrk-cli: {message}");
            ExitCode::FAILURE
        }
    }
}
