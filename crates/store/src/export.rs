//! Exports for people and other programs: JSON (everything), CSV (one row per task, for Excel)
//! and Markdown (readable overview). Templates are left out of CSV and Markdown.
//!
//! Classified content is marked: if anything in the workspace is VS-NfD, CSV and Markdown start
//! with the marking (docs/PLAN.md §3.8). Refusing a downgrading export is part of Phase 7.

use std::fmt::Write as _;

use plnwrk_core::{
    Classification, Milestone, MilestoneStatus, Project, Task, TaskId, TaskStatus, Workspace,
};

use crate::StoreError;

/// The marking for classified exports.
pub const VS_NFD_MARK: &str = "VS – NUR FÜR DEN DIENSTGEBRAUCH";

fn is_classified(ws: &Workspace) -> bool {
    ws.highest_classification() >= Classification::VsNfd
}

/// Live projects (no templates) by title.
fn projects(ws: &Workspace) -> Vec<&Project> {
    let mut projects: Vec<&Project> = ws.projects().filter(|p| !p.template).collect();
    projects.sort_by(|a, b| a.title.cmp(&b.title).then(a.id.cmp(&b.id)));
    projects
}

/// Tasks of a project in manual order.
fn tasks<'a>(ws: &'a Workspace, project: &Project) -> Vec<&'a Task> {
    let mut tasks: Vec<&Task> = ws.tasks_of(project.id).collect();
    tasks.sort_by(|a, b| a.rank.cmp(&b.rank));
    tasks
}

/// The whole workspace as JSON: `{"marking": …, "workspace": …}`; `marking` is the
/// classification marking or `null`.
pub fn json(ws: &Workspace) -> Result<String, StoreError> {
    let marking = is_classified(ws).then_some(VS_NFD_MARK);
    let value = serde_json::json!({ "marking": marking, "workspace": ws });
    serde_json::to_string_pretty(&value).map_err(|e| StoreError::corrupt(format!("export: {e}")))
}

/// A CSV field: quoted when needed; a leading `=`, `+`, `-`, `@`, tab or carriage return is
/// neutralised with `'` so spreadsheets never run it as a formula.
fn field(text: &str) -> String {
    let safe = if text.starts_with(['=', '+', '-', '@', '\t', '\r', '\n', '＝', '＋', '－', '＠'])
    {
        format!("'{text}")
    } else {
        text.to_owned()
    };
    if safe.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", safe.replace('"', "\"\""))
    } else {
        safe
    }
}

fn date(value: Option<jiff::civil::Date>) -> String {
    value.map(|d| d.to_string()).unwrap_or_default()
}

/// One row per task, separated by `;` (as spreadsheets in German locales expect), with a byte
/// order mark so that spreadsheets read the umlauts as UTF-8.
pub fn csv(ws: &Workspace) -> String {
    let mut out = String::from("\u{FEFF}");
    if is_classified(ws) {
        out.push_str(VS_NFD_MARK);
        out.push_str("\r\n");
    }
    out.push_str("Projekt;Aktenzeichen;Aufgabe;Übergeordnet;Status;Priorität;Bearbeitung;Beginn;Frist;Fortschritt;Schlagworte;Einstufung\r\n");
    for project in projects(ws) {
        for task in tasks(ws, project) {
            let parent = task
                .parent
                .and_then(|id| ws.task(id))
                .map(|p| p.title.as_str())
                .unwrap_or_default();
            let assignee = task
                .assignee
                .and_then(|id| ws.person(id))
                .map(|p| p.name.as_str())
                .unwrap_or_default();
            let row = [
                field(&project.title),
                field(project.reference.as_deref().unwrap_or_default()),
                field(&task.title),
                field(parent),
                task.status.name().to_owned(),
                task.priority.name().to_owned(),
                field(assignee),
                date(task.start),
                date(task.due),
                task.progress.to_string(),
                field(&task.tags.join(", ")),
                ws.effective_classification(task).name().to_owned(),
            ];
            out.push_str(&row.join(";"));
            out.push_str("\r\n");
        }
    }
    out
}

/// Escapes characters that Markdown would interpret, so titles cannot inject formatting, links
/// or HTML.
fn md(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '\\' | '`' | '*' | '_' | '[' | ']' | '#' | '<' | '>' | '|' | '!' | '~'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The task tree as a checklist, depth first, without recursion (trees can be deep).
fn task_lines(ws: &Workspace, project: &Project, all: &[&Task], out: &mut String) {
    let mut stack: Vec<(&Task, usize)> = all
        .iter()
        .rev()
        .filter(|t| t.parent.is_none())
        .map(|t| (*t, 0))
        .collect();
    while let Some((task, depth)) = stack.pop() {
        let mark = if task.status == TaskStatus::Done {
            "x"
        } else {
            " "
        };
        let mut details = Vec::new();
        if let Some(person) = task.assignee.and_then(|id| ws.person(id)) {
            details.push(md(&person.name));
        }
        if let Some(due) = task.due {
            details.push(format!("Frist {due}"));
        }
        if !matches!(task.status, TaskStatus::Todo | TaskStatus::Done) {
            details.push(task.status.name().replace('_', " "));
        }
        if project.classification < Classification::VsNfd
            && ws.effective_classification(task) >= Classification::VsNfd
        {
            details.push("VS-NfD".to_owned());
        }
        let suffix = if details.is_empty() {
            String::new()
        } else {
            format!(" — {}", details.join(", "))
        };
        let _ = writeln!(
            out,
            "{}- [{mark}] {}{suffix}",
            "  ".repeat(depth),
            md(&task.title)
        );
        let id: TaskId = task.id;
        stack.extend(
            all.iter()
                .rev()
                .filter(|t| t.parent == Some(id))
                .map(|t| (*t, depth + 1)),
        );
    }
}

/// A readable overview: one section per project with milestones and the task tree.
pub fn markdown(ws: &Workspace) -> String {
    let mut out = String::new();
    if is_classified(ws) {
        let _ = writeln!(out, "> **{VS_NFD_MARK}**\n");
    }
    out.push_str("# Arbeitsbereich\n");
    for project in projects(ws) {
        out.push('\n');
        match &project.reference {
            Some(reference) => {
                let _ = writeln!(out, "## {} ({})", md(&project.title), md(reference));
            }
            None => {
                let _ = writeln!(out, "## {}", md(&project.title));
            }
        }
        let mut facts = vec![format!(
            "Status: {}",
            project.status.name().replace('_', " ")
        )];
        if let Some(kind) = &project.kind {
            facts.insert(0, format!("Art: {}", md(kind)));
        }
        if let Some(due) = project.due {
            facts.push(format!("Frist: {due}"));
        }
        if let Some(owner) = project.owner.and_then(|id| ws.person(id)) {
            facts.push(format!("Verantwortlich: {}", md(&owner.name)));
        }
        if project.classification >= Classification::VsNfd {
            facts.push(VS_NFD_MARK.to_owned());
        }
        let _ = writeln!(out, "\n{}", facts.join(" · "));
        let mut milestones: Vec<&Milestone> = ws
            .milestones()
            .filter(|m| m.project == project.id)
            .collect();
        milestones.sort_by(|a, b| a.date.cmp(&b.date).then(a.title.cmp(&b.title)));
        if !milestones.is_empty() {
            out.push_str("\n### Meilensteine\n\n");
            for milestone in milestones {
                let mark = if milestone.status == MilestoneStatus::Reached {
                    "x"
                } else {
                    " "
                };
                let _ = writeln!(
                    out,
                    "- [{mark}] {} {}",
                    milestone.date,
                    md(&milestone.title)
                );
            }
        }
        let all = tasks(ws, project);
        if !all.is_empty() {
            out.push_str("\n### Aufgaben\n\n");
            task_lines(ws, project, &all, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_are_quoted_and_formulas_neutralised() {
        assert_eq!(field("einfach"), "einfach");
        assert_eq!(field("a;b"), "\"a;b\"");
        assert_eq!(field("Sag \"Hallo\""), "\"Sag \"\"Hallo\"\"\"");
        assert_eq!(field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(field("+49 40"), "'+49 40");
        assert_eq!(field("-1"), "'-1");
        assert_eq!(field("@SUM(A1)"), "'@SUM(A1)");
        assert_eq!(field("Zeile\nzwei"), "\"Zeile\nzwei\"");
        assert_eq!(field("\tx"), "'\tx");
        assert_eq!(field("\rx"), "\"'\rx\"");
        assert_eq!(field("＝1+1"), "'＝1+1");
    }

    #[test]
    fn markdown_cannot_be_injected() {
        assert_eq!(md("[klick](http://x)"), "\\[klick\\](http://x)");
        assert_eq!(md("<script>"), "\\<script\\>");
        assert_eq!(md("# Titel *fett*"), "\\# Titel \\*fett\\*");
        assert_eq!(md("a|b !c ~d"), "a\\|b \\!c \\~d");
    }
}
