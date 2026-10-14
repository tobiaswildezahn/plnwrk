//! The detail panel of the selected task: every field, subtasks, comments and history.

use egui::RichText;
use plnwrk_core::{EntityKey, PersonId, Priority, TaskId, TaskStatus};
use plnwrk_engine::query::{self, Action, TaskIndex};
use serde_json::{Value, json};

use crate::doc::OpenDoc;
use crate::state::{Dialog, Message, State, TaskDraft, tags};
use crate::views::project::person_choice;
use crate::views::{commit_field, date, due_label, enter_field};
use crate::{Clear, Intent, Then, dates, text};

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
                        && ui.link(format!("Teil von: {}", parent.title)).clicked()
                    {
                        out.push(Intent::Select(Some(parent.id)));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if super::icon_button(ui, "🗙", text::CLOSE_DETAILS).clicked() {
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
                if commit_field(&title) && draft.title.trim() != task.title {
                    out.push(update(id, "title", json!(draft.title.trim())));
                }
                ui.add_space(4.0);
                let people: Vec<(PersonId, String)> =
                    ws.people().map(|p| (p.id, p.name.clone())).collect();
                egui::Grid::new("task-fields")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label(text::STATUS);
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

                        ui.label(text::PRIORITY);
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

                        ui.label(text::ASSIGNEE);
                        let mut assignee = task.assignee;
                        if person_choice(ui, "task-assignee", &mut assignee, &people) {
                            let mut params = json!({"task": id.to_string()});
                            if let (Some(person), Some(map)) = (assignee, params.as_object_mut()) {
                                map.insert("assignee".to_owned(), json!(person.to_string()));
                            }
                            out.push(Intent::run("task.assign", params));
                        }
                        ui.end_row();

                        let caption = ui.label(text::START);
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
                                    out.push(update(id, "start", dates::param(date)));
                                }
                                Ok(_) => {}
                                Err(message) => state.message = Some(Message::error(message)),
                            }
                        }
                        ui.end_row();

                        let caption = ui.label(text::DUE);
                        let case = project.is_some_and(|p| p.reference.is_some());
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
                                    out.push(update(id, "due", dates::deadline_param(due)));
                                }
                                Ok(_) => {}
                                Err(message) => state.message = Some(Message::error(message)),
                            }
                        }
                        ui.end_row();

                        // Only top-level tasks without subtasks can be milestones.
                        ui.label("♦");
                        let has_children = ws.tasks_of(task.project).any(|t| t.parent == Some(id));
                        let allowed = task.parent.is_none() && !has_children;
                        let mut milestone = task.milestone;
                        let checkbox = ui
                            .add_enabled(
                                allowed || milestone,
                                egui::Checkbox::new(&mut milestone, text::MILESTONE),
                            )
                            .on_hover_text(text::MILESTONE_HINT)
                            .on_disabled_hover_text(text::MILESTONE_HINT);
                        if checkbox.changed() && milestone != task.milestone {
                            out.push(update(id, "milestone", json!(milestone)));
                        }
                        ui.end_row();

                        let caption = ui.label(text::PROGRESS);
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

                        let caption = ui.label(text::TAGS);
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
                if let Some(project) = project
                    && query::task_overdue(Some(project), task, now)
                {
                    ui.label(RichText::new(text::OVERDUE).color(ui.visuals().error_fg_color));
                }

                ui.add_space(6.0);
                let caption = ui.strong(text::DESCRIPTION);
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

                subtasks(ui, doc, id, state, out);
                comments(ui, doc, id, state, out);
                history(ui, doc, id);

                ui.add_space(12.0);
                if ui.button(text::DELETE_TASK).clicked() {
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
    ui.add_space(6.0);
    ui.strong(text::SUBTASKS);
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
    if enter_field(ui, title, text::NEW_SUBTASK_HINT, 260.0) {
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
    ui.strong(text::COMMENTS);
    for comment in all {
        let author = comment
            .author
            .and_then(|a| ws.person(a))
            .map_or("–", |p| p.name.as_str());
        let when = comment
            .created
            .to_zoned(jiff::tz::TimeZone::system())
            .strftime("%d.%m.%Y %H:%M")
            .to_string();
        ui.label(RichText::new(format!("{author} · {when}")).small().weak());
        ui.label(&comment.body);
    }
    ui.add(
        egui::TextEdit::multiline(&mut state.forms.comment)
            .hint_text(text::NEW_COMMENT_HINT)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    if ui.button(text::ADD_COMMENT).clicked() && !state.forms.comment.trim().is_empty() {
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
    egui::CollapsingHeader::new(text::HISTORY)
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
                let when = entry
                    .at
                    .to_zoned(jiff::tz::TimeZone::system())
                    .strftime("%d.%m.%Y %H:%M")
                    .to_string();
                let what = match entry.action {
                    Action::Created => "angelegt".to_owned(),
                    Action::Removed => "gelöscht".to_owned(),
                    Action::Changed => {
                        let fields: Vec<&str> =
                            entry.changes.iter().map(|c| c.field.as_str()).collect();
                        format!("geändert: {}", fields.join(", "))
                    }
                };
                ui.label(RichText::new(format!("{when}{who}")).small().weak());
                ui.label(what);
            }
        });
}
