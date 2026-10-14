//! plnwrk command line. Every answer is JSON on standard output, made for people and agents;
//! failures are JSON too (`{"error": …}`) with exit code 1.

mod files;
mod system;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};
use files::Failure;
use plnwrk_engine::query::{query_registry, search_queries};
use plnwrk_engine::{CommandSpec, Origin, Session, fixture, registry, search};
use plnwrk_store::{Document, Protection, export};
use serde_json::{Value, json};

fn file_arg() -> Arg {
    Arg::new("file")
        .required(true)
        .value_parser(clap::value_parser!(PathBuf))
        .help("The .plnwrk file")
}

fn params_arg() -> Arg {
    Arg::new("params")
        .default_value("{}")
        .help("Parameters as a JSON object")
}

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
        .after_help(format!(
            "Encrypted files: the passphrase comes from {} or is asked for.",
            files::PASSPHRASE_VAR
        ))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("new")
                .about("Creates a new workspace file")
                .arg(file_arg())
                .arg(
                    Arg::new("sample")
                        .long("sample")
                        .action(ArgAction::SetTrue)
                        .help("Fill it with the sample"),
                )
                .arg(
                    Arg::new("encrypt")
                        .long("encrypt")
                        .action(ArgAction::SetTrue)
                        .help("Protect it with a passphrase"),
                ),
        )
        .subcommand(
            Command::new("info")
                .about("Shows the readable head of a file (no passphrase needed)")
                .arg(file_arg()),
        )
        .subcommand(
            Command::new("verify")
                .about("Loads a file and checks everything: format, history, rules")
                .arg(file_arg()),
        )
        .subcommand(
            Command::new("run")
                .about("Runs a command on a file and saves it")
                .arg(file_arg())
                .arg(
                    Arg::new("command")
                        .required(true)
                        .help("Command id, e.g. task.create"),
                )
                .arg(params_arg()),
        )
        .subcommand(
            Command::new("query")
                .about("Runs a query on a file")
                .arg(file_arg())
                .arg(
                    Arg::new("name")
                        .required(true)
                        .help("Query name, e.g. board"),
                )
                .arg(params_arg()),
        )
        .subcommand(
            Command::new("export")
                .about("Exports a file as JSON, CSV or Markdown")
                .arg(file_arg())
                .arg(
                    Arg::new("format")
                        .long("format")
                        .required(true)
                        .value_parser(["json", "csv", "markdown"]),
                ),
        )
        .subcommand(
            Command::new("encrypt")
                .about("Protects a file with a passphrase")
                .arg(file_arg()),
        )
        .subcommand(
            Command::new("decrypt")
                .about("Removes the passphrase (refused for VS-NfD)")
                .arg(file_arg()),
        )
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

/// What a successful run prints: JSON, or text for CSV/Markdown exports.
enum Output {
    Json(Value),
    Text(String),
}

fn command_json(spec: &CommandSpec) -> Value {
    json!({"id": spec.id, "summary": spec.summary, "input_schema": spec.input_schema()})
}

fn path(matches: &ArgMatches) -> Result<&Path, Failure> {
    matches
        .get_one::<PathBuf>("file")
        .map(PathBuf::as_path)
        .ok_or_else(|| Failure::message("a file is required"))
}

fn params(matches: &ArgMatches) -> Result<Value, Failure> {
    let text = matches
        .get_one::<String>("params")
        .map_or("{}", String::as_str);
    serde_json::from_str(text)
        .map_err(|e| Failure::message(format!("parameters are not JSON: {e}")))
}

fn new_file(matches: &ArgMatches) -> Result<Output, Failure> {
    let path = path(matches)?;
    if path.exists() {
        return Err(Failure::message(format!(
            "{} already exists",
            path.display()
        )));
    }
    let mut session = files::session(Document::default());
    if matches.get_flag("sample") {
        fixture::sample(&mut session)?;
    }
    let protection = if matches.get_flag("encrypt") {
        Protection::Passphrase(files::passphrase(true)?)
    } else {
        Protection::None
    };
    files::save(path, &files::document(&session), &protection)?;
    Ok(Output::Json(
        json!({"created": path.display().to_string(), "encrypted": matches.get_flag("encrypt")}),
    ))
}

fn verify(matches: &ArgMatches) -> Result<Output, Failure> {
    let open = files::open(path(matches)?)?;
    Ok(Output::Json(json!({
        "ok": true,
        "manifest": open.manifest,
        "entities": open.doc.workspace.len(),
        "changes": open.doc.log.len(),
    })))
}

fn run_command(matches: &ArgMatches) -> Result<Output, Failure> {
    let path = path(matches)?;
    let command = matches
        .get_one::<String>("command")
        .map_or("", String::as_str);
    let params = params(matches)?;
    let open = files::open(path)?;
    let mut session: Session = files::session(open.doc);
    let outcome = session.run(command, &params, Origin::User)?;
    if outcome.change.is_some() {
        files::save(path, &files::document(&session), &open.protection)?;
    }
    Ok(Output::Json(
        json!({"outcome": outcome, "saved": outcome.change.is_some()}),
    ))
}

fn run_query(matches: &ArgMatches) -> Result<Output, Failure> {
    let open = files::open(path(matches)?)?;
    let name = matches.get_one::<String>("name").map_or("", String::as_str);
    let session = files::session(open.doc);
    Ok(Output::Json(session.query(name, &params(matches)?)?))
}

fn export_file(matches: &ArgMatches) -> Result<Output, Failure> {
    let open = files::open(path(matches)?)?;
    let ws = &open.doc.workspace;
    match matches.get_one::<String>("format").map(String::as_str) {
        Some("json") => Ok(Output::Text(export::json(ws)?)),
        Some("csv") => Ok(Output::Text(export::csv(ws))),
        Some("markdown") => Ok(Output::Text(export::markdown(ws))),
        _ => Err(Failure::message("unknown format")),
    }
}

fn encrypt(matches: &ArgMatches) -> Result<Output, Failure> {
    let path = path(matches)?;
    let open = files::open(path)?;
    if open.manifest.encrypted {
        return Err(Failure::message("the file is already encrypted"));
    }
    let protection = Protection::Passphrase(files::passphrase(true)?);
    files::save(path, &open.doc, &protection)?;
    Ok(Output::Json(json!({"encrypted": true})))
}

fn decrypt(matches: &ArgMatches) -> Result<Output, Failure> {
    let path = path(matches)?;
    let open = files::open(path)?;
    if !open.manifest.encrypted {
        return Err(Failure::message("the file is not encrypted"));
    }
    files::save(path, &open.doc, &Protection::None)?;
    Ok(Output::Json(json!({"encrypted": false})))
}

fn run(matches: &ArgMatches) -> Result<Output, Failure> {
    let words = |m: &ArgMatches| m.get_one::<String>("search").cloned();
    match matches.subcommand() {
        Some(("new", sub)) => new_file(sub),
        Some(("info", sub)) => {
            let bytes = plnwrk_store::file::read(path(sub)?)?;
            Ok(Output::Json(json!(plnwrk_store::peek(&bytes)?)))
        }
        Some(("verify", sub)) => verify(sub),
        Some(("run", sub)) => run_command(sub),
        Some(("query", sub)) => run_query(sub),
        Some(("export", sub)) => export_file(sub),
        Some(("encrypt", sub)) => encrypt(sub),
        Some(("decrypt", sub)) => decrypt(sub),
        Some(("commands", sub)) => {
            let specs: Vec<&CommandSpec> = match words(sub) {
                Some(words) => search(&words),
                None => registry().iter().collect(),
            };
            Ok(Output::Json(Value::from(
                specs.into_iter().map(command_json).collect::<Vec<_>>(),
            )))
        }
        Some(("queries", sub)) => {
            let specs = match words(sub) {
                Some(words) => search_queries(&words),
                None => query_registry().iter().collect(),
            };
            Ok(Output::Json(Value::from(
                specs
                    .into_iter()
                    .map(|q| json!({"name": q.name, "summary": q.summary, "input_schema": q.input_schema()}))
                    .collect::<Vec<_>>(),
            )))
        }
        Some(("demo", _)) => {
            let mut session = files::session(Document::default());
            let sample = fixture::sample(&mut session)?;
            let project = json!({"project": sample.project.to_string()});
            Ok(Output::Json(json!({
                "overview": session.query("overview", &json!({}))?,
                "board": session.query("board", &project)?,
                "overdue": session.query("overdue", &json!({}))?,
                "case": session.query("project", &json!({"project": sample.case.to_string()}))?,
            })))
        }
        _ => Err(Failure::message("unknown subcommand; see --help")),
    }
}

fn print(text: &str) -> ExitCode {
    match writeln!(std::io::stdout().lock(), "{text}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn main() -> ExitCode {
    let matches = cli().get_matches();
    match run(&matches) {
        Ok(Output::Json(value)) => print(&serde_json::to_string_pretty(&value).unwrap_or_default()),
        Ok(Output::Text(text)) => print(text.trim_end()),
        Err(Failure(value)) => {
            let _ = print(&serde_json::to_string_pretty(&value).unwrap_or_default());
            eprintln!(
                "plnwrk-cli: {}",
                value
                    .get("message")
                    .or_else(|| value.get("reason"))
                    .unwrap_or(&value)
            );
            ExitCode::FAILURE
        }
    }
}
