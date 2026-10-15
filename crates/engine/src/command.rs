//! Every user-visible action is a [`CommandSpec`] (docs/PLAN.md §3.3). The UI, the command
//! line, MCP and the assistant all find commands here and run them through
//! [`Session::run`](crate::Session::run).

use serde_json::Value;

use crate::params::{ParamSpec, Params, input_schema};
use crate::{CommandError, Session, Tx, commands};

/// The body of an editing command: reads parameters, writes through the transaction.
pub type EditFn = fn(&mut Tx<'_>, &Params<'_>) -> Result<(), CommandError>;

/// How a command runs.
#[derive(Debug, Clone, Copy)]
pub enum Runner {
    /// A normal command inside a transaction.
    Edit(EditFn),
    /// Reverts the latest change.
    Undo,
    /// Repeats the latest reverted change.
    Redo,
}

/// Description and implementation of one command.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    /// Stable id, `entity.verb` (e.g. `task.create`). Also the i18n key of its label.
    pub id: &'static str,
    /// One sentence in English for people and agents.
    pub summary: &'static str,
    /// Accepted parameters.
    pub params: &'static [ParamSpec],
    /// Whether it can run now; `Err` gives the reason.
    pub enabled: fn(&Session) -> Result<(), String>,
    /// What it does.
    pub run: Runner,
}

impl CommandSpec {
    /// JSON schema of the parameters.
    pub fn input_schema(&self) -> Value {
        input_schema(self.params)
    }
}

/// For commands that can always run.
pub fn always(_: &Session) -> Result<(), String> {
    Ok(())
}

static REGISTRY: &[CommandSpec] = &[
    commands::project::CREATE,
    commands::project::UPDATE,
    commands::project::ARCHIVE,
    commands::project::INSTANTIATE,
    commands::task::CREATE,
    commands::task::UPDATE,
    commands::task::MOVE,
    commands::task::REPARENT,
    commands::task::ASSIGN,
    commands::task::SET_STATUS,
    commands::task::RESCHEDULE,
    commands::task::SET_PROGRESS,
    commands::task::DELETE,
    commands::dependency::ADD,
    commands::dependency::REMOVE,
    commands::person::CREATE,
    commands::person::UPDATE,
    commands::comment::ADD,
    commands::history::UNDO,
    commands::history::REDO,
    commands::workspace::UPGRADE,
];

/// Commands of older versions: never run any more, but found by [`find`] so that the history of
/// older files can still be checked.
static LEGACY: &[CommandSpec] = &[
    commands::milestone::CREATE,
    commands::milestone::UPDATE,
    commands::milestone::REACH,
    commands::milestone::DELETE,
];

/// All commands.
pub fn registry() -> &'static [CommandSpec] {
    REGISTRY
}

/// A command by id.
pub fn find(id: &str) -> Option<&'static CommandSpec> {
    REGISTRY.iter().chain(LEGACY).find(|spec| spec.id == id)
}

/// The lower-case words of a search text.
pub(crate) fn search_words(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '_')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Commands matching the words of `query` in id, summary, parameter names or their help,
/// best first.
pub fn search(query: &str) -> Vec<&'static CommandSpec> {
    let words = search_words(query);
    let mut scored: Vec<(usize, &'static CommandSpec)> = REGISTRY
        .iter()
        .filter_map(|spec| {
            let haystack = format!(
                "{} {} {}",
                spec.id,
                spec.summary.to_lowercase(),
                spec.params
                    .iter()
                    .map(|p| format!("{} {}", p.name, p.help.to_lowercase()))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            let mut score: usize = words
                .iter()
                .filter(|w| haystack.contains(w.as_str()))
                .count();
            if words.iter().any(|w| w == spec.id) {
                score += 10;
            }
            (score > 0).then_some((score, spec))
        })
        .collect();
    scored.sort_by(|(a, x), (b, y)| b.cmp(a).then(x.id.cmp(y.id)));
    scored.into_iter().map(|(_, spec)| spec).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_well_formed() {
        let mut ids: Vec<&str> = registry().iter().map(|s| s.id).collect();
        for id in &ids {
            let mut parts = id.split('.');
            let entity = parts.next().unwrap_or_default();
            let verb = parts.next();
            assert!(
                !entity.is_empty() && entity.chars().all(|c| c.is_ascii_lowercase()),
                "{id}"
            );
            assert!(
                verb.is_none_or(
                    |v| !v.is_empty() && v.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                ),
                "{id}"
            );
            assert!(parts.next().is_none(), "{id}");
        }
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn every_command_has_a_summary_and_valid_schema() {
        for spec in registry() {
            assert!(spec.summary.ends_with('.'), "{}", spec.id);
            let schema = spec.input_schema();
            assert_eq!(schema["type"], "object", "{}", spec.id);
        }
    }

    #[test]
    fn search_ranks_exact_id_first() {
        let found = search("undo");
        assert_eq!(found.first().map(|s| s.id), Some("undo"));
        // Milestones are tasks now: the search leads there, never to the retired commands.
        let milestone = search("milestone");
        assert!(milestone.iter().any(|s| s.id == "task.create"));
        assert!(milestone.iter().all(|s| !s.id.starts_with("milestone.")));
        // Found by what a parameter means, not only by its name ("due" is the deadline).
        let ids: Vec<&str> = search("deadline").iter().map(|s| s.id).collect();
        assert!(
            ids.contains(&"task.update") && ids.contains(&"task.create"),
            "{ids:?}"
        );
        assert!(search("").is_empty());
        assert!(search("zzzzz").is_empty());
    }

    #[test]
    fn retired_commands_are_found_but_never_listed() {
        assert!(
            find("milestone.create").is_some(),
            "old histories stay checkable"
        );
        assert!(registry().iter().all(|s| !s.id.starts_with("milestone.")));
    }

    #[test]
    fn find_by_id() {
        assert!(find("task.create").is_some());
        assert!(find("task.fly").is_none());
    }
}
