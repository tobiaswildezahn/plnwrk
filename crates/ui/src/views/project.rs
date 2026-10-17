//! One project or case: its head (fields, progress) and the tabs list, board and milestones.

use egui::RichText;
use plnwrk_core::{Classification, Project, ProjectId, ProjectStatus, Task, TaskStatus};
use plnwrk_engine::query::{self, TaskIndex};
use serde_json::json;

use crate::design::Icon;
use crate::doc::OpenDoc;
use crate::state::{ProjectForm, State, Tab};
use crate::views::{board, date, due_label, enter_field, icon_button, status_tag};
use crate::{Clear, Intent, Then, dates, text};

pub fn show(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    id: ProjectId,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let Some(project) = ws.project(id) else {
        return;
    };
    head(ui, doc, project, state, out);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.tab, Tab::List, text::t().list);
        ui.selectable_value(&mut state.tab, Tab::Board, text::t().board);
        ui.selectable_value(&mut state.tab, Tab::Milestones, text::t().milestones);
        ui.selectable_value(&mut state.tab, Tab::Schedule, text::t().schedule_tab);
    });
    ui.separator();
    match state.tab {
        Tab::List => list(ui, doc, project, state, out),
        Tab::Board => board::show(ui, doc, project, state, out),
        Tab::Milestones => milestones(ui, doc, project, state, out),
        Tab::Schedule => super::schedule::show(ui, doc, project, state, out),
    }
}

fn head(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    project: &Project,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let today = now.date();
    let mut error = None;
    let mut cancel = false;
    // An open form follows changes of the stored project (undo, archive) before every frame:
    // `PlnwrkApp::sync_project_form`.
    if let Some((editing, form)) = &mut state.forms.project
        && editing.id == project.id
    {
        let people: Vec<_> = ws.people().map(|p| (p.id, p.name.clone())).collect();
        egui::Grid::new("project-form")
            .num_columns(2)
            .show(ui, |ui| {
                let label = ui.label(text::t().title);
                ui.text_edit_singleline(&mut form.title)
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(text::t().kind);
                ui.add(egui::TextEdit::singleline(&mut form.kind).hint_text(text::t().kind_hint))
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(text::t().reference);
                ui.text_edit_singleline(&mut form.reference)
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(text::t().start);
                date::date_input(ui, "project-start", &mut form.start, today, Some(label.id));
                ui.end_row();
                let label = ui.label(text::t().due);
                let case = !form.reference.trim().is_empty();
                date::deadline_input(
                    ui,
                    "project-due",
                    &mut form.due,
                    &mut form.due_time,
                    today,
                    case,
                    Some(label.id),
                );
                ui.end_row();
                ui.label(text::t().owner);
                person_choice(ui, "project-owner", &mut form.owner, &people);
                ui.end_row();
                ui.label(text::t().status);
                egui::ComboBox::from_id_salt("project-status")
                    .selected_text(text::project_status(form.status))
                    .show_ui(ui, |ui| {
                        for status in ProjectStatus::ALL {
                            ui.selectable_value(
                                &mut form.status,
                                status,
                                text::project_status(status),
                            );
                        }
                    });
                ui.end_row();
                ui.label(text::t().template);
                ui.checkbox(&mut form.template, text::t().use_as_template);
                ui.end_row();
            });
        ui.horizontal(|ui| {
            if crate::design::widgets::button(
                ui,
                crate::design::widgets::Kind::Primary,
                text::t().apply,
            )
            .clicked()
            {
                match form.update_params(project.id) {
                    Ok(params) => out.push(Intent::Run {
                        command: "project.update",
                        params,
                        then: Then::Nothing,
                        clear: Some(Clear::ProjectEdit),
                    }),
                    Err(message) => error = Some(message),
                }
            }
            cancel = crate::design::widgets::button(
                ui,
                crate::design::widgets::Kind::Secondary,
                text::t().cancel,
            )
            .clicked();
        });
        if let Some(message) = error {
            state.message = Some(crate::state::Message::error(message));
        }
        if cancel {
            state.forms.project = None;
        }
        return;
    }

    let mut line = Vec::new();
    if project.template {
        line.push(text::t().template.to_owned());
    }
    if let Some(kind) = &project.kind {
        line.push(kind.clone());
    }
    if let Some(reference) = &project.reference {
        line.push(format!("{} {reference}", text::t().reference));
    }
    line.push(text::project_status(project.status).to_owned());
    ui.horizontal(|ui| {
        ui.heading(&project.title);
        if project.classification >= Classification::VsNfd {
            ui.label(
                RichText::new("VS-NfD")
                    .strong()
                    .color(ui.visuals().error_fg_color),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if project.status != ProjectStatus::Archived && ui.button(text::t().archive).clicked() {
                out.push(Intent::run(
                    "project.archive",
                    json!({"project": project.id.to_string()}),
                ));
            }
            if ui.button(text::t().edit).clicked() {
                state.forms.project = Some((project.clone(), ProjectForm::of(project)));
            }
        });
    });
    ui.weak(line.join(" · "));
    ui.horizontal(|ui| {
        if let Some(due) = project.due {
            let (state, past) = text::deadline_state(due, project.due_time, now);
            let text = format!(
                "{} {} · {state}",
                text::t().due,
                dates::format_deadline(due, project.due_time),
            );
            if past && query::deadlines_count(project) {
                ui.label(RichText::new(text).color(ui.visuals().error_fg_color));
            } else {
                ui.label(text);
            }
        }
        if let Some(owner) = project.owner.and_then(|id| ws.person(id)) {
            ui.label(format!("{}: {}", text::t().owner, owner.name));
        }
    });
    if let Some(progress) = query::progress(ws, project.id) {
        crate::design::widgets::progress(
            ui,
            f32::from(progress.percent) / 100.0,
            &text::progress(progress.percent, progress.done, progress.tasks),
        );
    }
}

/// The title as shown in lists: milestones carry a ♦.
fn title_of(task: &Task) -> String {
    if task.milestone {
        format!("♦ {}", task.title)
    } else {
        task.title.clone()
    }
}

/// Tasks in manual order.
fn sorted<'a>(tasks: &[&'a Task]) -> Vec<&'a Task> {
    let mut tasks = tasks.to_vec();
    tasks.sort_by(|a, b| a.rank.cmp(&b.rank));
    tasks
}

/// A choice of a person (or nobody).
pub fn person_choice(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut Option<plnwrk_core::PersonId>,
    people: &[(plnwrk_core::PersonId, String)],
) -> bool {
    let current = value
        .and_then(|id| people.iter().find(|(p, _)| *p == id))
        .map_or(text::t().nobody, |(_, name)| name.as_str());
    let before = *value;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, text::t().nobody);
            for (person, name) in people {
                ui.selectable_value(value, Some(*person), name);
            }
        });
    *value != before
}

/// The task tree, depth first in manual order, with quick entry for tasks and subtasks.
fn list(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    project: &Project,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let index = TaskIndex::new(ws);
    let roots: Vec<&Task> = sorted(index.tasks_of(project.id))
        .into_iter()
        .filter(|t| t.parent.is_none())
        .collect();
    egui::ScrollArea::vertical().show(ui, |ui| {
        if roots.is_empty() {
            ui.weak(text::t().no_tasks);
        }
        let mut stack: Vec<(&Task, usize)> = roots.iter().rev().map(|t| (*t, 0)).collect();
        while let Some((task, depth)) = stack.pop() {
            let overdue = query::task_overdue(Some(project), task, now);
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 20.0);
                let mut done = task.status == TaskStatus::Done;
                if ui
                    .checkbox(&mut done, "")
                    .on_hover_text(text::task_status(TaskStatus::Done))
                    .changed()
                {
                    let status = if done {
                        TaskStatus::Done
                    } else {
                        TaskStatus::Todo
                    };
                    out.push(Intent::run(
                        "task.set_status",
                        json!({"task": task.id.to_string(), "status": status.name()}),
                    ));
                }
                let mut cancel = false;
                if let Some((editing, text)) = &mut state.forms.inline
                    && *editing == task.id
                {
                    // Renaming right in the list: Enter or leaving takes it, Escape drops it.
                    let field = ui.add(
                        egui::TextEdit::singleline(text)
                            .id_salt(("inline", task.id))
                            .desired_width(320.0),
                    );
                    if !field.has_focus() && !field.lost_focus() {
                        field.request_focus();
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel = true;
                    } else if field.lost_focus() {
                        // Unchanged or emptied: like Escape.
                        if text.trim() == task.title || text.trim().is_empty() {
                            cancel = true;
                        } else {
                            out.push(Intent::Run {
                                command: "task.update",
                                params: json!({"task": task.id.to_string(), "title": text.trim()}),
                                then: Then::Nothing,
                                clear: Some(Clear::Inline),
                            });
                        }
                    }
                } else {
                    let title = if done {
                        RichText::new(title_of(task)).strikethrough().weak()
                    } else {
                        RichText::new(title_of(task))
                    };
                    let label = ui
                        .selectable_label(state.selected == Some(task.id), title)
                        .on_hover_text(text::t().rename_hint);
                    if label.double_clicked() {
                        state.forms.inline = Some((task.id, task.title.clone()));
                    } else if label.clicked() {
                        out.push(Intent::Select(Some(task.id)));
                    }
                }
                if cancel {
                    state.forms.inline = None;
                }
                if let Some(person) = task.assignee.and_then(|id| ws.person(id)) {
                    ui.label(RichText::new(&person.initials).small().weak())
                        .on_hover_text(&person.name);
                }
                due_label(ui, task.due, task.due_time, overdue);
                status_tag(ui, task.status);
                if icon_button(ui, Icon::Plus, text::t().add_subtask).clicked() {
                    state.forms.new_subtask = Some((task.id, String::new()));
                }
            });
            let mut cancel = false;
            if let Some((parent, title)) = &mut state.forms.new_subtask
                && *parent == task.id
            {
                ui.horizontal(|ui| {
                    ui.add_space((depth + 1) as f32 * 20.0 + 24.0);
                    if enter_field(ui, title, text::t().new_subtask_hint, 320.0, false) {
                        out.push(Intent::Run {
                            command: "task.create",
                            params: json!({
                                "project": project.id.to_string(),
                                "parent": task.id.to_string(),
                                "title": title.trim(),
                            }),
                            then: Then::Nothing,
                            clear: Some(Clear::NewSubtask),
                        });
                    }
                    cancel = icon_button(ui, Icon::Close, text::t().cancel).clicked();
                });
            }
            if cancel {
                state.forms.new_subtask = None;
            }
            for child in sorted(index.children(task.id)).into_iter().rev() {
                stack.push((child, depth + 1));
            }
        }
        ui.add_space(6.0);
        let focus = std::mem::take(&mut state.focus_new_task);
        if enter_field(
            ui,
            &mut state.forms.new_task,
            text::t().new_task_hint,
            360.0,
            focus,
        ) {
            out.push(Intent::Run {
                command: "task.create",
                params: json!({
                    "project": project.id.to_string(),
                    "title": state.forms.new_task.trim(),
                }),
                then: Then::Nothing,
                clear: Some(Clear::NewTask),
            });
        }
    });
}

fn milestones(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    project: &Project,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let mut all: Vec<&Task> = ws.tasks_of(project.id).filter(|t| t.milestone).collect();
    all.sort_by(|a, b| {
        plnwrk_engine::query::deadline_order(a.due, a.due_time)
            .cmp(&plnwrk_engine::query::deadline_order(b.due, b.due_time))
            .then(a.title.cmp(&b.title))
    });
    if all.is_empty() {
        ui.weak(text::t().no_milestones);
    }
    egui::Grid::new("milestones")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            for milestone in all {
                let state_now = query::milestone_state_in(project, milestone, now);
                let mut reached = milestone.status == TaskStatus::Done;
                if ui.checkbox(&mut reached, text::t().reached).changed() {
                    let status = if reached {
                        TaskStatus::Done
                    } else {
                        TaskStatus::Todo
                    };
                    out.push(Intent::run(
                        "task.set_status",
                        json!({"task": milestone.id.to_string(), "status": status.name()}),
                    ));
                }
                ui.label(
                    milestone
                        .due
                        .map(|day| dates::format_deadline(day, milestone.due_time))
                        .unwrap_or_default(),
                );
                let label = RichText::new(format!(
                    "♦ {} ({})",
                    milestone.title,
                    text::milestone_state(state_now)
                ));
                let label = if state_now == query::MilestoneState::Missed {
                    label.color(ui.visuals().error_fg_color)
                } else {
                    label
                };
                if ui
                    .selectable_label(state.selected == Some(milestone.id), label)
                    .clicked()
                {
                    out.push(Intent::Select(Some(milestone.id)));
                }
                if icon_button(ui, Icon::Delete, text::t().delete_milestone).clicked() {
                    out.push(Intent::Open(crate::state::Dialog::DeleteTask(milestone.id)));
                }
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.forms.milestone_title)
                .hint_text(text::t().title)
                .desired_width(240.0),
        );
        date::deadline_input(
            ui,
            ("new-milestone", project.id),
            &mut state.forms.milestone_date,
            &mut state.forms.milestone_time,
            now.date(),
            project.reference.is_some(),
            None,
        );
        if ui.button(text::t().new_milestone).clicked() {
            match dates::parse_deadline(&state.forms.milestone_date, &state.forms.milestone_time) {
                Ok(Some(due)) => out.push(Intent::Run {
                    command: "task.create",
                    params: json!({
                        "project": project.id.to_string(),
                        "title": state.forms.milestone_title.trim(),
                        "milestone": true,
                        "due": dates::deadline_param(Some(due)),
                    }),
                    then: Then::Nothing,
                    clear: Some(Clear::Milestone),
                }),
                Ok(None) => {
                    state.message = Some(crate::state::Message::error(text::t().date_needed));
                }
                Err(message) => state.message = Some(crate::state::Message::error(message)),
            }
        }
    });
}
