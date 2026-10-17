//! The detail panel of the selected task: every field, subtasks, comments and history.

use std::collections::{BTreeMap, BTreeSet};

use egui::RichText;
use plnwrk_core::{EntityKey, PersonId, Priority, TaskId, TaskStatus, Workspace};
use plnwrk_engine::query::{self, Action, TaskIndex};
use serde_json::{Value, json};

use crate::design::Icon;
use crate::doc::OpenDoc;
use crate::state::{Dialog, Message, State, TaskDraft, tags};
use crate::views::project::person_choice;
use crate::views::{commit_field, date, due_label, enter_field};
use crate::{Clear, Intent, Then, dates, text};

/// A group of the panel: a heading that folds it away, open at first.
fn group(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    ui.add_space(4.0);
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .id_salt(salt)
        .default_open(true)
        .show(ui, add);
}

/// A `task.update` with one field.
fn update(task: TaskId, field: &str, value: Value) -> Intent {
    let mut params = serde_json::Map::new();
    params.insert("task".to_owned(), json!(task.to_string()));
    params.insert(field.to_owned(), value);
    Intent::run("task.update", Value::Object(params))
}

pub fn panel(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    id: TaskId,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let Some(task) = ws.task(id) else { return };
    let mut draft = match state.forms.task.take() {
        Some(mut draft) if draft.source.id == id => {
            if &draft.source != task {
                draft.rebase(task);
            }
            draft
        }
        _ => TaskDraft::of(task),
    };
    let now = doc.session.clock().now_local();
    let today = now.date();
    let project = ws.project(task.project);
    egui::Panel::right("detail")
        .default_size(360.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(parent) = task.parent.and_then(|p| ws.task(p))
                        && ui
                            .link(format!("{} {}", text::t().part_of, parent.title))
                            .clicked()
                    {
                        out.push(Intent::Select(Some(parent.id)));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if super::icon_button(ui, Icon::Close, text::t().close_details).clicked() {
                            out.push(Intent::Select(None));
                        }
                    });
                });
                let title = ui.add(
                    egui::TextEdit::singleline(&mut draft.title)
                        .id_salt(("task-title", id))
                        .font(egui::TextStyle::Heading)
                        .desired_width(f32::INFINITY),
                );
                if std::mem::take(&mut state.focus_details) {
                    title.request_focus();
                }
                if commit_field(&title) && draft.title.trim() != task.title {
                    out.push(update(id, "title", json!(draft.title.trim())));
                }
                ui.add_space(4.0);
                let people: Vec<(PersonId, String)> =
                    ws.people().map(|p| (p.id, p.name.clone())).collect();
                // In groups (Phase 9f), each folds away; what is typed stays in the draft
                // while folded and is taken on saving or closing like any typed text.
                let t = text::t();
                group(ui, ("assignment", id), t.group_assignment, |ui| {
                    egui::Grid::new(("task-assignment", id))
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label(text::t().status);
                            let mut status = task.status;
                            egui::ComboBox::from_id_salt("task-status")
                                .selected_text(text::task_status(status))
                                .show_ui(ui, |ui| {
                                    for s in TaskStatus::ALL {
                                        ui.selectable_value(&mut status, s, text::task_status(s));
                                    }
                                });
                            if status != task.status {
                                out.push(Intent::run(
                                    "task.set_status",
                                    json!({"task": id.to_string(), "status": status.name()}),
                                ));
                            }
                            ui.end_row();
                            ui.label(text::t().priority);
                            let mut priority = task.priority;
                            egui::ComboBox::from_id_salt("task-priority")
                                .selected_text(text::priority(priority))
                                .show_ui(ui, |ui| {
                                    for p in Priority::ALL {
                                        ui.selectable_value(&mut priority, p, text::priority(p));
                                    }
                                });
                            if priority != task.priority {
                                out.push(update(id, "priority", json!(priority.name())));
                            }
                            ui.end_row();
                            ui.label(text::t().assignee);
                            let mut assignee = task.assignee;
                            if person_choice(ui, "task-assignee", &mut assignee, &people) {
                                let mut params = json!({"task": id.to_string()});
                                if let (Some(person), Some(map)) =
                                    (assignee, params.as_object_mut())
                                {
                                    map.insert("assignee".to_owned(), json!(person.to_string()));
                                }
                                out.push(Intent::run("task.assign", params));
                            }
                            ui.end_row();
                            let caption = ui.label(text::t().tags);
                            let response = ui
                                .add(
                                    egui::TextEdit::singleline(&mut draft.tags)
                                        .id_salt(("task-tags", id))
                                        .hint_text("a, b, c"),
                                )
                                .labelled_by(caption.id);
                            if commit_field(&response) && tags(&draft.tags) != task.tags {
                                out.push(update(id, "tags", json!(tags(&draft.tags))));
                            }
                            ui.end_row();
                        });
                });
                group(ui, ("dates", id), t.group_dates, |ui| {
                    egui::Grid::new(("task-dates", id))
                        .num_columns(2)
                        .show(ui, |ui| {
                            // A date that cannot be read is said under its field.
                            let (start_field, due_field) = (start_said(id), due_said(id));
                            let caption = ui.label(text::t().start);
                            let start = date::date_input(
                                ui,
                                ("task-start", id),
                                &mut draft.start,
                                today,
                                Some(caption.id),
                            );
                            if start.commit() {
                                match dates::parse(&draft.start) {
                                    Ok(date) if date != task.start => {
                                        state.unsay_at(start_field);
                                        out.push(Intent::Reschedule {
                                            task: id,
                                            start: date,
                                            due: task.due,
                                            due_time: None,
                                        });
                                    }
                                    // Readable again: what it said goes (only then).
                                    Ok(_) => state.unsay_at(start_field),
                                    Err(message) => {
                                        state.say_at(start_field, Message::error(message));
                                    }
                                }
                            }
                            ui.end_row();
                            super::field_said_in_grid(ui, state, start_field);
                            let caption = ui.label(text::t().due);
                            let case = project.is_some_and(plnwrk_core::Project::is_case);
                            let due = date::deadline_input(
                                ui,
                                ("task-due", id),
                                &mut draft.due,
                                &mut draft.due_time,
                                today,
                                case,
                                Some(caption.id),
                            );
                            if due.commit() {
                                match dates::parse_deadline(&draft.due, &draft.due_time) {
                                    Ok(due) if due != task.due.map(|day| (day, task.due_time)) => {
                                        state.unsay_at(due_field);
                                        out.push(Intent::Reschedule {
                                            task: id,
                                            start: task.start,
                                            due: due.map(|(day, _)| day),
                                            due_time: Some(due.and_then(|(_, time)| time)),
                                        });
                                    }
                                    Ok(_) => state.unsay_at(due_field),
                                    Err(message) => {
                                        state.say_at(due_field, Message::error(message));
                                    }
                                }
                            }
                            ui.end_row();
                            super::field_said_in_grid(ui, state, due_field);
                            // Only top-level tasks without subtasks can be milestones.
                            ui.label("♦");
                            let has_children =
                                ws.tasks_of(task.project).any(|t| t.parent == Some(id));
                            let allowed = task.parent.is_none() && !has_children;
                            let mut milestone = task.milestone;
                            let checkbox = ui
                                .add_enabled(
                                    allowed || milestone,
                                    egui::Checkbox::new(&mut milestone, text::t().milestone),
                                )
                                .on_hover_text(text::t().milestone_hint)
                                .on_disabled_hover_text(text::t().milestone_hint);
                            if checkbox.changed() && milestone != task.milestone {
                                out.push(update(id, "milestone", json!(milestone)));
                            }
                            ui.end_row();
                            let caption = ui.label(text::t().progress);
                            let slider = ui
                                .add(egui::Slider::new(&mut draft.progress, 0..=100).suffix(" %"))
                                .labelled_by(caption.id);
                            if (slider.drag_stopped() || (slider.changed() && !slider.dragged()))
                                && draft.progress != task.progress
                            {
                                out.push(Intent::run(
                                    "task.set_progress",
                                    json!({"task": id.to_string(), "progress": draft.progress}),
                                ));
                            }
                            ui.end_row();
                        });
                    if let Some(project) = project
                        && query::task_overdue(Some(project), task, now)
                    {
                        ui.label(
                            RichText::new(text::t().overdue).color(ui.visuals().error_fg_color),
                        );
                    }

                    // A date that does not fit what it waits for (or what waits for it):
                    // said here, in the schedule's words.
                    if let Some(project) = project {
                        let worked = super::schedule::worked(ui, doc, project);
                        for warning in worked
                            .analysis
                            .warnings
                            .iter()
                            .filter(|w| super::schedule::warning_concerns(w, id))
                        {
                            let (_, said) = super::schedule::warning_said(ws, warning);
                            crate::design::widgets::field_error(ui, &said);
                        }
                    }
                });
                group(ui, ("subtasks", id), t.subtasks, |ui| {
                    subtasks(ui, doc, id, state, out);
                });
                group(ui, ("dependencies", id), t.waits_for, |ui| {
                    dependencies(ui, doc, id, out);
                });
                group(ui, ("notes", id), t.group_notes, |ui| {
                    let caption = ui.strong(text::t().description);
                    let response = ui
                        .add(
                            egui::TextEdit::multiline(&mut draft.description)
                                .id_salt(("task-description", id))
                                .desired_rows(4)
                                .desired_width(f32::INFINITY),
                        )
                        .labelled_by(caption.id);
                    if commit_field(&response) && draft.description != task.description {
                        out.push(update(id, "description", json!(draft.description)));
                    }

                    comments(ui, doc, id, state, out);
                    history(ui, doc, id);
                });

                // Deleting: red, apart at the bottom.
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                if crate::design::widgets::button(
                    ui,
                    crate::design::widgets::Kind::Destructive,
                    text::t().delete_task,
                )
                .clicked()
                {
                    out.push(Intent::Open(Dialog::DeleteTask(id)));
                }
            });
        });
    state.forms.task = Some(draft);
}

fn subtasks(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    id: TaskId,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let Some(task) = ws.task(id) else { return };
    let index = TaskIndex::new(ws);
    let mut children = index.children(id).to_vec();
    children.sort_by(|a, b| a.rank.cmp(&b.rank));
    for child in children {
        ui.horizontal(|ui| {
            let mut done = child.status == TaskStatus::Done;
            if ui.checkbox(&mut done, "").changed() {
                let status = if done {
                    TaskStatus::Done
                } else {
                    TaskStatus::Todo
                };
                out.push(Intent::run(
                    "task.set_status",
                    json!({"task": child.id.to_string(), "status": status.name()}),
                ));
            }
            if ui.link(&child.title).clicked() {
                out.push(Intent::Select(Some(child.id)));
            }
            due_label(
                ui,
                child.due,
                child.due_time,
                query::task_overdue(ws.project(child.project), child, now),
            );
        });
    }
    let title = &mut state.forms.detail_subtask;
    if enter_field(ui, title, text::t().new_subtask_hint, 260.0, false) {
        out.push(Intent::Run {
            command: "task.create",
            params: json!({
                "project": task.project.to_string(),
                "parent": id.to_string(),
                "title": title.trim(),
            }),
            then: Then::Nothing,
            clear: Some(Clear::DetailSubtask),
        });
    }
}

/// What the task waits for (editable) and what waits for it (to go there).
fn dependencies(ui: &mut egui::Ui, doc: &OpenDoc, id: TaskId, out: &mut Vec<Intent>) {
    let ws = doc.session.workspace();
    let Some(task) = ws.task(id) else { return };
    let incoming: Vec<_> = ws.dependencies().filter(|d| d.to == id).collect();
    for dependency in &incoming {
        let Some(from) = ws.task(dependency.from) else {
            continue;
        };
        ui.horizontal(|ui| {
            if ui.link(&from.title).clicked() {
                out.push(Intent::Select(Some(from.id)));
            }
            if super::icon_button(ui, Icon::Close, text::t().remove_dependency).clicked() {
                out.push(Intent::run(
                    "dependency.remove",
                    json!({"dependency": dependency.id.to_string()}),
                ));
            }
        });
    }
    // Every other task is offered; what it already waits for, and what would make a circle
    // (it waits for this one, directly or not), greyed with the reason (Phase 9f) – not
    // missing without a word.
    let why_not = precede_refusal(ws, id);
    let mut candidates: Vec<_> = ws.tasks_of(task.project).filter(|t| t.id != id).collect();
    candidates.sort_by(|a, b| a.title.cmp(&b.title));
    if !candidates.is_empty() {
        egui::ComboBox::from_id_salt(("predecessor", id))
            .selected_text(text::t().add_predecessor)
            .show_ui(ui, |ui| {
                for candidate in candidates {
                    let refusal = why_not(candidate.id);
                    let choice = ui
                        .add_enabled(
                            refusal.is_none(),
                            egui::Button::selectable(false, &candidate.title),
                        )
                        .on_disabled_hover_text(refusal.unwrap_or_default());
                    if choice.clicked() {
                        out.push(Intent::run(
                            "dependency.add",
                            json!({"from": candidate.id.to_string(), "to": id.to_string()}),
                        ));
                    }
                }
            });
    }
    let outgoing: Vec<_> = ws
        .dependencies()
        .filter(|d| d.from == id)
        .filter_map(|d| ws.task(d.to))
        .collect();
    if !outgoing.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.weak(text::t().followed_by);
            for next in outgoing {
                if ui.link(&next.title).clicked() {
                    out.push(Intent::Select(Some(next.id)));
                }
            }
        });
    }
}

/// What `task` may wait for: not itself, not what it already waits for, and not what waits
/// for it (directly or not) – that would make a circle. Worked out once per task, then asked
/// for each candidate. Shared by the detail panel and the schedule.
pub(crate) fn may_precede(ws: &Workspace, task: TaskId) -> impl Fn(TaskId) -> bool {
    let refusal = precede_refusal(ws, task);
    move |candidate| refusal(candidate).is_none()
}

/// Why `task` may not wait for a candidate (the rule of [`may_precede`], with its reason):
/// itself, already waited for, or a circle.
pub(crate) fn precede_refusal(
    ws: &Workspace,
    task: TaskId,
) -> impl Fn(TaskId) -> Option<&'static str> {
    let after = reach(ws, task, |d| (d.from, d.to));
    let direct: BTreeSet<TaskId> = ws
        .dependencies()
        .filter(|d| d.to == task)
        .map(|d| d.from)
        .collect();
    move |candidate| {
        if candidate == task {
            Some(text::t().not_itself)
        } else if direct.contains(&candidate) {
            Some(text::t().waits_already)
        } else if after.contains(&candidate) {
            Some(text::t().would_circle)
        } else {
            None
        }
    }
}

/// What may wait for `task`: not itself, not what already waits for it, and not what it waits
/// for (directly or not) – that would make a circle.
pub(crate) fn may_follow(ws: &Workspace, task: TaskId) -> impl Fn(TaskId) -> bool {
    let before = reach(ws, task, |d| (d.to, d.from));
    let direct: BTreeSet<TaskId> = ws
        .dependencies()
        .filter(|d| d.from == task)
        .map(|d| d.to)
        .collect();
    move |candidate| {
        candidate != task && !direct.contains(&candidate) && !before.contains(&candidate)
    }
}

/// Every task reached from `id` along the dependencies, read as `step` (from → to, or back).
fn reach(
    ws: &Workspace,
    id: TaskId,
    step: impl Fn(&plnwrk_core::Dependency) -> (TaskId, TaskId),
) -> BTreeSet<TaskId> {
    let mut next: BTreeMap<TaskId, Vec<TaskId>> = BTreeMap::new();
    for dependency in ws.dependencies() {
        let (from, to) = step(dependency);
        next.entry(from).or_default().push(to);
    }
    let mut seen = BTreeSet::new();
    let mut queue = vec![id];
    while let Some(task) = queue.pop() {
        for after in next.get(&task).into_iter().flatten() {
            if seen.insert(*after) {
                queue.push(*after);
            }
        }
    }
    seen
}

fn comments(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    id: TaskId,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let mut all: Vec<_> = ws.comments().filter(|c| c.task == id).collect();
    all.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.cmp(&b.id)));
    ui.add_space(6.0);
    ui.strong(text::t().comments);
    for comment in all {
        let author = comment
            .author
            .and_then(|a| ws.person(a))
            .map_or("–", |p| p.name.as_str());
        let when = crate::dates::format_moment(comment.created);
        ui.label(RichText::new(format!("{author} · {when}")).small().weak());
        ui.label(&comment.body);
    }
    ui.add(
        egui::TextEdit::multiline(&mut state.forms.comment)
            .hint_text(text::t().new_comment_hint)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    if ui.button(text::t().add_comment).clicked() && !state.forms.comment.trim().is_empty() {
        out.push(Intent::Run {
            command: "comment.add",
            params: json!({"task": id.to_string(), "body": state.forms.comment.trim()}),
            then: Then::Nothing,
            clear: Some(Clear::Comment),
        });
    }
}

fn history(ui: &mut egui::Ui, doc: &OpenDoc, id: TaskId) {
    let ws = doc.session.workspace();
    ui.add_space(6.0);
    egui::CollapsingHeader::new(text::t().history)
        .id_salt(("history", id))
        .show(ui, |ui| {
            for entry in query::history(doc.session.log(), EntityKey::Task(id))
                .iter()
                .rev()
            {
                let who = entry
                    .actor
                    .and_then(|a| ws.person(a))
                    .map_or_else(String::new, |p| format!(" · {}", p.name));
                let when = crate::dates::format_moment(entry.at);
                let what = match entry.action {
                    Action::Created => text::t().created.to_owned(),
                    Action::Removed => text::t().removed.to_owned(),
                    Action::Changed => {
                        let fields: Vec<String> = entry
                            .changes
                            .iter()
                            .filter_map(|c| text::field(&c.field))
                            .collect();
                        format!("{} {}", text::t().changed, fields.join(", "))
                    }
                };
                let by = if entry.origin == plnwrk_engine::Origin::Assistant {
                    format!(" · {}", text::t().by_ai)
                } else {
                    String::new()
                };
                ui.label(RichText::new(format!("{when}{who}{by}")).small().weak());
                ui.label(what);
            }
        });
}

/// Where a task's start says what cannot be read (its own id, see `State::say_at`).
pub(crate) fn start_said(task: TaskId) -> egui::Id {
    egui::Id::new(("task-start-said", task))
}

/// Where a task's deadline says what cannot be read.
pub(crate) fn due_said(task: TaskId) -> egui::Id {
    egui::Id::new(("task-due-said", task))
}
