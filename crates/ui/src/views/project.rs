//! One project or case: its head (fields, progress) and the tabs list, board and milestones.

use egui::RichText;
use plnwrk_core::{
    Classification, MilestoneStatus, Project, ProjectId, ProjectStatus, Task, TaskStatus,
};
use plnwrk_engine::query::{self, TaskIndex};
use serde_json::json;

use crate::doc::OpenDoc;
use crate::state::{ProjectForm, State, Tab};
use crate::views::{board, due_label, enter_field, status_tag};
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
        ui.selectable_value(&mut state.tab, Tab::List, text::LIST);
        ui.selectable_value(&mut state.tab, Tab::Board, text::BOARD);
        ui.selectable_value(&mut state.tab, Tab::Milestones, text::MILESTONES);
    });
    ui.separator();
    match state.tab {
        Tab::List => list(ui, doc, project, state, out),
        Tab::Board => board::show(ui, doc, project, state, out),
        Tab::Milestones => milestones(ui, doc, project, state, out),
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
    let today = doc.session.clock().today();
    let mut error = None;
    let mut cancel = false;
    if let Some((editing, form)) = &mut state.forms.project
        && *editing == project.id
    {
        let people: Vec<_> = ws.people().map(|p| (p.id, p.name.clone())).collect();
        egui::Grid::new("project-form")
            .num_columns(2)
            .show(ui, |ui| {
                let label = ui.label(text::TITLE);
                ui.text_edit_singleline(&mut form.title)
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(text::KIND);
                ui.add(
                    egui::TextEdit::singleline(&mut form.kind)
                        .hint_text("Projekt, SKA, Beschaffung …"),
                )
                .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(text::REFERENCE);
                ui.text_edit_singleline(&mut form.reference)
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(text::START);
                ui.add(egui::TextEdit::singleline(&mut form.start).hint_text("TT.MM.JJJJ"))
                    .labelled_by(label.id);
                ui.end_row();
                let label = ui.label(text::DUE);
                ui.add(egui::TextEdit::singleline(&mut form.due).hint_text("TT.MM.JJJJ"))
                    .labelled_by(label.id);
                ui.end_row();
                ui.label(text::OWNER);
                person_choice(ui, "project-owner", &mut form.owner, &people);
                ui.end_row();
                ui.label(text::STATUS);
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
                ui.label(text::TEMPLATE);
                ui.checkbox(&mut form.template, "Als Vorlage verwenden");
                ui.end_row();
            });
        ui.horizontal(|ui| {
            if ui.button(text::APPLY).clicked() {
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
            cancel = ui.button(text::CANCEL).clicked();
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
        line.push(text::TEMPLATE.to_owned());
    }
    if let Some(kind) = &project.kind {
        line.push(kind.clone());
    }
    if let Some(reference) = &project.reference {
        line.push(format!("{} {reference}", text::REFERENCE));
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
            if project.status != ProjectStatus::Archived && ui.button(text::ARCHIVE).clicked() {
                out.push(Intent::run(
                    "project.archive",
                    json!({"project": project.id.to_string()}),
                ));
            }
            if ui.button(text::EDIT).clicked() {
                state.forms.project = Some((project.id, ProjectForm::of(project)));
            }
        });
    });
    ui.weak(line.join(" · "));
    ui.horizontal(|ui| {
        if let Some(due) = project.due {
            let days = (due - today).get_days();
            let text = format!(
                "{} {} · {}",
                text::DUE,
                dates::format(due),
                text::days_left(i64::from(days))
            );
            if days < 0 && query::deadlines_count(project) {
                ui.label(RichText::new(text).color(ui.visuals().error_fg_color));
            } else {
                ui.label(text);
            }
        }
        if let Some(owner) = project.owner.and_then(|id| ws.person(id)) {
            ui.label(format!("{}: {}", text::OWNER, owner.name));
        }
    });
    if let Some(progress) = query::progress(ws, project.id) {
        ui.add(
            egui::ProgressBar::new(f32::from(progress.percent) / 100.0)
                .desired_width(320.0)
                .text(format!(
                    "{} % · {} von {} erledigt",
                    progress.percent, progress.done, progress.tasks
                )),
        );
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
        .map_or(text::NOBODY, |(_, name)| name.as_str());
    let before = *value;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, text::NOBODY);
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
    let today = doc.session.clock().today();
    let index = TaskIndex::new(ws);
    let roots: Vec<&Task> = sorted(index.tasks_of(project.id))
        .into_iter()
        .filter(|t| t.parent.is_none())
        .collect();
    egui::ScrollArea::vertical().show(ui, |ui| {
        if roots.is_empty() {
            ui.weak(text::NO_TASKS);
        }
        let mut stack: Vec<(&Task, usize)> = roots.iter().rev().map(|t| (*t, 0)).collect();
        while let Some((task, depth)) = stack.pop() {
            let overdue = query::task_overdue(Some(project), task, today);
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
                let title = if done {
                    RichText::new(&task.title).strikethrough().weak()
                } else {
                    RichText::new(&task.title)
                };
                if ui
                    .selectable_label(state.selected == Some(task.id), title)
                    .clicked()
                {
                    out.push(Intent::Select(Some(task.id)));
                }
                if let Some(person) = task.assignee.and_then(|id| ws.person(id)) {
                    ui.label(RichText::new(&person.initials).small().weak())
                        .on_hover_text(&person.name);
                }
                due_label(ui, task.due, overdue);
                status_tag(ui, task.status);
                if ui
                    .small_button("+")
                    .on_hover_text(text::ADD_SUBTASK)
                    .clicked()
                {
                    state.forms.new_subtask = Some((task.id, String::new()));
                }
            });
            let mut cancel = false;
            if let Some((parent, title)) = &mut state.forms.new_subtask
                && *parent == task.id
            {
                ui.horizontal(|ui| {
                    ui.add_space((depth + 1) as f32 * 20.0 + 24.0);
                    if enter_field(ui, title, text::NEW_SUBTASK_HINT, 320.0) {
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
                    cancel = ui.small_button("✕").on_hover_text(text::CANCEL).clicked();
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
        if enter_field(ui, &mut state.forms.new_task, text::NEW_TASK_HINT, 360.0) {
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
    let today = doc.session.clock().today();
    let mut all: Vec<_> = ws
        .milestones()
        .filter(|m| m.project == project.id)
        .collect();
    all.sort_by(|a, b| a.date.cmp(&b.date).then(a.title.cmp(&b.title)));
    if all.is_empty() {
        ui.weak(text::NO_MILESTONES);
    }
    egui::Grid::new("milestones")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            for milestone in all {
                let state_now = query::milestone_state_in(project, milestone, today);
                let mut reached = milestone.status == MilestoneStatus::Reached;
                if ui.checkbox(&mut reached, text::REACHED).changed() {
                    let status = if reached {
                        MilestoneStatus::Reached
                    } else {
                        MilestoneStatus::Open
                    };
                    out.push(Intent::run(
                        "milestone.update",
                        json!({"milestone": milestone.id.to_string(), "status": status.name()}),
                    ));
                }
                ui.label(dates::format(milestone.date));
                let label = RichText::new(format!(
                    "◆ {} ({})",
                    milestone.title,
                    text::milestone_state(state_now)
                ));
                if state_now == query::MilestoneState::Missed {
                    ui.label(label.color(ui.visuals().error_fg_color));
                } else {
                    ui.label(label);
                }
                if ui.small_button("🗑").on_hover_text(text::DELETE).clicked() {
                    out.push(Intent::run(
                        "milestone.delete",
                        json!({"milestone": milestone.id.to_string()}),
                    ));
                }
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.forms.milestone_title)
                .hint_text(text::TITLE)
                .desired_width(240.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut state.forms.milestone_date)
                .hint_text(format!("{} TT.MM.JJJJ", text::DATE))
                .desired_width(130.0),
        );
        if ui.button(text::NEW_MILESTONE).clicked() {
            match dates::parse(&state.forms.milestone_date) {
                Ok(Some(date)) => out.push(Intent::Run {
                    command: "milestone.create",
                    params: json!({
                        "project": project.id.to_string(),
                        "title": state.forms.milestone_title.trim(),
                        "date": date.to_string(),
                    }),
                    then: Then::Nothing,
                    clear: Some(Clear::Milestone),
                }),
                Ok(None) => {
                    state.message = Some(crate::state::Message::error("Bitte ein Datum angeben."));
                }
                Err(message) => state.message = Some(crate::state::Message::error(message)),
            }
        }
    });
}
