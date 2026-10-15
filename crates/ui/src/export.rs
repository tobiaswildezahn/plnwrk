//! The schedule of a project as a table for Excel (CSV): one row per task in the order of the
//! list, with dates, status, who does it, length, float, critical path and what it waits for.
//! Separated by `;` with a byte order mark, as German Excel opens it directly. A file that is
//! (or was) VS-NfD carries the marking in its first line, like every export (PLAN §3.8).

use plnwrk_core::{Project, Workspace};
use plnwrk_engine::query::TaskIndex;
use plnwrk_schedule::{Calendar, Plan};
use plnwrk_store::export::{VS_NFD_MARK, field};

use crate::{dates, text, views};

/// The schedule of `project` as CSV text.
pub fn schedule_csv(ws: &Workspace, project: &Project, classified: bool) -> String {
    let calendar = Calendar::standard();
    let plan = Plan::of(ws, project.id, calendar);
    let analysis = plan.analyse();
    let index = TaskIndex::new(ws);
    let mut out = String::from("\u{FEFF}");
    if classified {
        out.push_str(VS_NFD_MARK);
        out.push_str("\r\n");
    }
    out.push_str(&text::schedule_csv_header().join(";"));
    out.push_str("\r\n");
    let yes = text::t().yes;
    for (task, depth) in views::schedule::rows(&index, project) {
        let parent = task
            .parent
            .and_then(|id| ws.task(id))
            .map_or("", |p| p.title.as_str());
        let assignee = task
            .assignee
            .and_then(|id| ws.person(id))
            .map_or("", |p| p.name.as_str());
        let length = plan
            .items()
            .find(|i| i.id == task.id)
            .and_then(plnwrk_schedule::Item::span)
            .filter(|_| !task.milestone)
            .map(|(start, end)| calendar.count(start, end).max(1).to_string())
            .unwrap_or_default();
        let float = analysis
            .float
            .get(&task.id)
            .map(ToString::to_string)
            .unwrap_or_default();
        let waits: Vec<&str> = ws
            .dependencies()
            .filter(|d| d.to == task.id)
            .filter_map(|d| ws.task(d.from))
            .map(|t| t.title.as_str())
            .collect();
        let row = [
            field(&task.title),
            (depth + 1).to_string(),
            field(parent),
            field(text::task_status(task.status)),
            field(assignee),
            dates::format_opt(task.start),
            dates::format_opt(task.due),
            dates::format_time_opt(task.due_time),
            length,
            float,
            if analysis.critical.contains(&task.id) {
                yes
            } else {
                ""
            }
            .to_owned(),
            if task.milestone { yes } else { "" }.to_owned(),
            field(&waits.join(", ")),
            task.progress.to_string(),
        ];
        out.push_str(&row.join(";"));
        out.push_str("\r\n");
    }
    out
}

/// A file name for the schedule of a project: its title without characters file systems
/// refuse, and "Zeitplan"/"Schedule".
pub fn file_name(title: &str) -> String {
    let clean: String = title
        .chars()
        .map(|c| {
            if c.is_control() || r#"/\\:*?"<>|"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    // File systems allow 255 bytes per name: 120 characters leave room for the rest.
    let clean: String = clean.chars().take(120).collect();
    let clean = clean.trim().trim_start_matches('.');
    let clean = if clean.is_empty() { "plnwrk" } else { clean };
    format!("{clean} – {}.csv", text::t().schedule_tab)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_have_no_characters_file_systems_refuse() {
        assert_eq!(
            file_name("SKA 23/1234: Sirene?"),
            format!("SKA 23_1234_ Sirene_ – {}.csv", text::t().schedule_tab)
        );
        assert!(file_name("..").starts_with("plnwrk"));
        assert!(file_name("  ").starts_with("plnwrk"));
    }
}
