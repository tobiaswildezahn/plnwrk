//! The command palette (Cmd+K): every action of the menus and views, and every project and
//! task by name, found by typing. It only offers what the menus and buttons can do, through the
//! same [`Intent`]s, so questions (unsaved changes, deleting) still come.

use plnwrk_engine::query;

use crate::actions::Group;
use crate::doc::OpenDoc;
use crate::state::{State, View};
use crate::{Intent, text};

/// One line of the palette.
#[derive(Debug)]
pub struct Entry {
    /// What it does or what it is called.
    pub label: String,
    /// Where it belongs (menu, project of a task), shown greyed.
    pub detail: String,
    /// Its keyboard shortcut, if it has one.
    pub shortcut: Option<egui::KeyboardShortcut>,
    /// Commands of the register it runs, or whose place (form, view) it opens; their ids can be
    /// searched too. A test checks that every command it runs is named here.
    pub commands: &'static [&'static str],
    /// What happens when it is chosen.
    pub intents: Vec<Intent>,
}

/// Commands of the register the palette does not lead to, and why. Every other command must be
/// reachable from the palette (checked by a test), so a new command cannot be forgotten.
#[cfg(test)]
pub const ELSEWHERE: &[(&str, &str)] = &[
    (
        "task.update",
        "fields of the detail panel, renaming in the list",
    ),
    ("task.move", "dragging in the list and the board"),
    ("task.reparent", "dragging in the list"),
    ("task.assign", "field of the detail panel"),
    ("task.set_progress", "field of the detail panel"),
    ("dependency.add", "field waits-for in the detail panel"),
    ("dependency.remove", "field waits-for in the detail panel"),
    ("comment.add", "field of the detail panel"),
    (
        "workspace.upgrade",
        "runs by itself when an older file is opened",
    ),
];

struct List(Vec<Entry>);

impl List {
    fn add(
        &mut self,
        label: impl Into<String>,
        detail: impl Into<String>,
        shortcut: Option<egui::KeyboardShortcut>,
        commands: &'static [&'static str],
        intents: Vec<Intent>,
    ) {
        self.0.push(Entry {
            label: label.into(),
            detail: detail.into(),
            shortcut,
            commands,
            intents,
        });
    }
}

/// Everything the palette offers now: the actions of the ribbon that can be used (with their
/// place, "Start › Neu"), then places to go to.
pub fn entries(doc: Option<&OpenDoc>, state: &State) -> Vec<Entry> {
    let mut list = List(Vec::new());
    let chosen_task = doc
        .and_then(|d| state.selected.and_then(|id| d.session.workspace().task(id)))
        .map(|task| task.title.clone());
    for action in crate::actions::all(doc, state) {
        // The palette does not offer itself, nor what is already chosen or not possible.
        if !action.enabled || action.chosen || action.group == Group::Find {
            continue;
        }
        // What the actions of the tab Task act on: the chosen task, named.
        let detail = match (action.group.tab(), &chosen_task) {
            (crate::actions::RibbonTab::Task, Some(title)) => {
                format!("{} · {title}", action.group.place())
            }
            _ => action.group.place(),
        };
        list.add(
            action.full_name().to_owned(),
            detail,
            action.shortcut,
            action.commands,
            action.intents,
        );
    }
    if let Some(doc) = doc {
        places(&mut list, doc);
    }
    list.0
}

/// Every project, case, template and task, to go to.
fn places(list: &mut List, doc: &OpenDoc) {
    let t = text::t();
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let cards = query::overview(ws, now)
        .into_iter()
        .map(|c| (c.id, c.title, c.reference))
        .chain(
            query::templates(ws, now)
                .into_iter()
                .map(|c| (c.id, c.title, c.reference)),
        );
    let mut offered = std::collections::HashSet::new();
    for (id, title, reference) in cards {
        let label = match reference {
            Some(reference) => format!("{title} · {reference}"),
            None => title,
        };
        let kind = match ws.project(id) {
            Some(project) if project.template => t.templates,
            _ => t.projects,
        };
        let detail = format!("{} · {kind}", t.go_to);
        offered.insert(id);
        list.add(
            label,
            detail,
            None,
            &[],
            vec![Intent::Go(View::Project(id))],
        );
    }
    // Tasks of the projects offered (not of archived ones, which the sidebar hides too).
    for task in ws.tasks() {
        let Some(project) = ws.project(task.project).filter(|p| offered.contains(&p.id)) else {
            continue;
        };
        let detail = format!("{} · {}", t.go_to, project.title);
        let go = vec![
            Intent::Go(View::Project(project.id)),
            Intent::Select(Some(task.id)),
        ];
        list.add(task.title.clone(), detail, None, &[], go);
    }
}

/// Lower case without accents, so "ubersicht" finds "Übersicht".
fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'ä' => folded.push('a'),
            'ö' => folded.push('o'),
            'ü' => folded.push('u'),
            'ß' => folded.push_str("ss"),
            c => folded.push(c),
        }
    }
    folded
}

/// The entries matching `query`, best first: every word must occur (in the name, the detail or
/// a command id); words at the start of a word of the name count more. Without a query, all.
pub fn search(entries: Vec<Entry>, query: &str) -> Vec<Entry> {
    let query = fold(query.trim());
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() {
        return entries;
    }
    let mut found: Vec<(usize, usize, Entry)> = entries
        .into_iter()
        .enumerate()
        .filter_map(|(order, entry)| {
            let label = fold(&entry.label);
            let haystack = format!(
                "{label} {} {}",
                fold(&entry.detail),
                entry.commands.join(" ")
            );
            if !words.iter().all(|w| haystack.contains(w)) {
                return None;
            }
            let starts_word = |w: &str| {
                label
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|part| part.starts_with(w))
            };
            let mut score: usize = words
                .iter()
                .map(|w| {
                    if starts_word(w) {
                        2
                    } else {
                        usize::from(label.contains(w))
                    }
                })
                .sum();
            if label.starts_with(&query) {
                score += 3;
            }
            Some((score, order, entry))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, _, entry)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(label: &str, detail: &str, commands: &'static [&'static str]) -> Entry {
        Entry {
            label: label.to_owned(),
            detail: detail.to_owned(),
            shortcut: None,
            commands,
            intents: Vec::new(),
        }
    }

    fn labels(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.label.as_str()).collect()
    }

    #[test]
    fn every_word_must_match_and_word_starts_come_first() {
        let all = || {
            vec![
                entry("Speichern unter…", "Datei", &[]),
                entry("Speichern", "Datei", &[]),
                entry("Übersicht", "Ansicht", &[]),
                entry("Bericht schreiben", "Gehe zu · Lage", &[]),
                entry("Neue Aufgabe", "Lage", &["task.create"]),
            ]
        };
        assert_eq!(labels(&search(all(), "")).len(), 5, "no query: everything");
        assert_eq!(
            labels(&search(all(), "speich")),
            ["Speichern unter…", "Speichern"]
        );
        assert_eq!(labels(&search(all(), "ubersicht")), ["Übersicht"]);
        assert_eq!(labels(&search(all(), "  UEBER ")).len(), 0);
        assert_eq!(
            labels(&search(all(), "lage bericht")),
            ["Bericht schreiben"]
        );
        assert_eq!(labels(&search(all(), "task.create")), ["Neue Aufgabe"]);
        assert_eq!(
            labels(&search(all(), "lage")),
            ["Bericht schreiben", "Neue Aufgabe"]
        );
        assert!(search(all(), "xyz").is_empty());
    }

    #[test]
    fn umlauts_and_sharp_s_can_be_typed_plainly() {
        let all = || {
            vec![
                entry("Änderung prüfen", "", &[]),
                entry("Straßensperrung", "", &[]),
                entry("Lösung", "", &[]),
            ]
        };
        assert_eq!(labels(&search(all(), "anderung")), ["Änderung prüfen"]);
        assert_eq!(labels(&search(all(), "pruf")), ["Änderung prüfen"]);
        assert_eq!(labels(&search(all(), "strasse")), ["Straßensperrung"]);
        assert_eq!(labels(&search(all(), "losung")), ["Lösung"]);
        assert_eq!(labels(&search(all(), "ÄNDERUNG")), ["Änderung prüfen"]);
    }

    #[test]
    fn a_word_start_beats_a_match_inside_a_word() {
        let found = search(
            vec![entry("Ablage", "", &[]), entry("Neue Lage", "", &[])],
            "lage",
        );
        assert_eq!(labels(&found), ["Neue Lage", "Ablage"]);
    }

    #[test]
    fn the_whole_name_at_the_start_wins() {
        let found = search(
            vec![
                entry("Neue Aufgabe speichern", "", &[]),
                entry("Speichern", "", &[]),
            ],
            "speichern",
        );
        assert_eq!(labels(&found), ["Speichern", "Neue Aufgabe speichern"]);
    }
}
