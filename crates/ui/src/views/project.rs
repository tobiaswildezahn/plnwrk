//! One project or case: its head (fields, progress) and the tabs list, board and milestones.

use egui::RichText;
use plnwrk_core::{Classification, Project, ProjectId, ProjectStatus, Task, TaskId, TaskStatus};
use plnwrk_engine::query::{self, TaskIndex};
use serde_json::json;

use crate::design::Icon;
use crate::doc::OpenDoc;
use crate::state::{ProjectForm, State, Tab};
use crate::views::{board, date, due_label, enter_field, icon_button};
use crate::{Clear, Intent, Then, dates, text};

/// The list's columns (Phase 9f): deadline, who, status; room for the "+"; the least the
/// title keeps before columns give way; how long a new task is lit up.
const LIST_DUE: f32 = 150.0;
const LIST_WHO: f32 = 140.0;
const LIST_STATUS: f32 = 100.0;
const LIST_PLUS: f32 = 28.0;
const LIST_TITLE_MIN: f32 = 200.0;
const FRESH_SECONDS: f64 = 1.5;
/// Room the project's buttons (Bearbeiten, Archivieren) need beside its title.
const HEAD_BUTTONS: f32 = 240.0;

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
        Tab::Milestones => super::milestones::show(ui, doc, project, state, out),
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
                let case = form.is_case();
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
        line.push(text::kind_shown(kind));
    }
    if let Some(reference) = &project.reference {
        line.push(format!("{} {reference}", text::t().reference));
    }
    line.push(text::project_status(project.status).to_owned());
    // The title and its buttons in one line where they fit; otherwise the buttons go below
    // the title (with the details open the middle is narrow – they must not lie over it).
    let title_width = ui
        .painter()
        .layout_no_wrap(
            project.title.clone(),
            egui::TextStyle::Heading.resolve(ui.style()),
            egui::Color32::PLACEHOLDER,
        )
        .size()
        .x;
    let one_line = ui.available_width() >= title_width + HEAD_BUTTONS;
    let mut buttons = |ui: &mut egui::Ui| {
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
    };
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new(&project.title).heading()).wrap());
        if project.classification >= Classification::VsNfd {
            ui.label(
                RichText::new("VS-NfD")
                    .strong()
                    .color(ui.visuals().error_fg_color),
            );
        }
        if one_line {
            buttons(ui);
        }
    });
    if !one_line {
        ui.horizontal(|ui| buttons(ui));
    }
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
    // The columns right of the title (Phase 9f), and how many fit: from the right they give
    // way before the title gets narrower than its least.
    let room = ui.available_width();
    let all_columns: [(&str, f32); 3] = [
        (text::t().due, LIST_DUE),
        (text::t().assignee, LIST_WHO),
        (text::t().status, LIST_STATUS),
    ];
    let mut shown = all_columns.len();
    while shown > 0
        && room - all_columns.iter().take(shown).map(|(_, w)| w).sum::<f32>() - LIST_PLUS
            < LIST_TITLE_MIN
    {
        shown -= 1;
    }
    let columns = all_columns.get(..shown).unwrap_or_default();
    // The gaps between the parts of a row count too (tick, title, columns, "+").
    let gaps = ui.spacing().item_spacing.x * (columns.len() as f32 + 2.0);
    let title_width = room - columns.iter().map(|(_, w)| w).sum::<f32>() - LIST_PLUS - gaps - 8.0;
    // A task made a moment ago is lit up for a second and a half (one repaint ends it; no
    // animation that keeps the app busy).
    let now_s = ui.input(|i| i.time);
    let fresh_id = egui::Id::new("list-fresh");
    let fresh = state.fresh.filter(|task| {
        let since = match ui.data(|d| d.get_temp::<(TaskId, f64)>(fresh_id)) {
            Some((seen, at)) if seen == *task => at,
            _ => {
                ui.data_mut(|d| d.insert_temp(fresh_id, (*task, now_s)));
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(FRESH_SECONDS));
                now_s
            }
        };
        now_s - since < FRESH_SECONDS
    });
    if state.fresh.is_some() && fresh.is_none() {
        state.fresh = None;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        if roots.is_empty() {
            // Nothing yet: the step that helps is the field below.
            if crate::design::widgets::empty_state(
                ui,
                Icon::Task,
                text::t().no_tasks,
                Some(text::t().first_task),
            ) {
                state.focus_new_task = true;
            }
        } else {
            // The headings of the columns, painted (as in the schedule's table): the fields of
            // the details carry these names for the screen reader.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(room, 18.0), egui::Sense::hover());
            let small = egui::TextStyle::Small.resolve(ui.style());
            let weak = ui.visuals().weak_text_color();
            let mut x = rect.left() + 24.0;
            for (said, width) in std::iter::once((text::t().ribbon.task, title_width - 24.0))
                .chain(columns.iter().copied())
            {
                ui.painter().text(
                    egui::pos2(x, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    said,
                    small.clone(),
                    weak,
                );
                x += width + ui.spacing().item_spacing.x;
            }
            ui.separator();
        }
        let mut stack: Vec<(&Task, usize)> = roots.iter().rev().map(|t| (*t, 0)).collect();
        while let Some((task, depth)) = stack.pop() {
            let overdue = query::task_overdue(Some(project), task, now);
            ui.horizontal(|ui| {
                let top = ui.cursor().top();
                let height = ui.spacing().interact_size.y;
                let left = ui.max_rect().left();
                // Lit up just after it was made.
                if fresh == Some(task.id) {
                    let row = egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(room, height));
                    let p = crate::design::palette(ui);
                    ui.painter()
                        .rect_filled(row, egui::CornerRadius::same(4), p.accent_soft);
                }
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
                let indent = depth as f32 * 20.0;
                let title_room = (title_width - 24.0).max(LIST_TITLE_MIN);
                let mut cancel = false;
                ui.allocate_ui_with_layout(
                    egui::vec2(title_room, height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(title_room);
                        ui.add_space(indent);
                        if let Some((editing, text)) = &mut state.forms.inline
                            && *editing == task.id
                        {
                            // Renaming right in the list: Enter or leaving takes it, Escape
                            // drops it.
                            let field = ui.add(
                                egui::TextEdit::singleline(text)
                                    .id_salt(("inline", task.id))
                                    .desired_width((title_room - indent - 16.0).max(120.0)),
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
                    },
                );
                if cancel {
                    state.forms.inline = None;
                }
                for (said, width) in columns {
                    ui.allocate_ui_with_layout(
                        egui::vec2(*width, height),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(*width);
                            if *said == text::t().due {
                                due_label(ui, task.due, task.due_time, overdue);
                            } else if *said == text::t().assignee {
                                if let Some(person) = task.assignee.and_then(|id| ws.person(id)) {
                                    ui.label(RichText::new(&person.name).small());
                                }
                            } else if task.status != TaskStatus::Todo {
                                ui.label(
                                    RichText::new(text::task_status(task.status))
                                        .small()
                                        .weak(),
                                );
                            }
                        },
                    );
                }
                // "+ Unteraufgabe" only where the pointer is, at the chosen task, or while it
                // has the keyboard focus – so the list stays quiet (Phase 9f).
                // The whole line of the row, however wide the window.
                let row_rect = egui::Rect::from_min_max(
                    egui::pos2(left, top),
                    egui::pos2(f32::INFINITY, top + height),
                );
                let shows = ui.rect_contains_pointer(row_rect)
                    || state.selected == Some(task.id)
                    || ui.memory(|m| {
                        m.focused()
                            .is_some_and(|f| ui.data(|d| d.get_temp::<TaskId>(f.with("plus-of"))) == Some(task.id))
                    });
                if shows {
                    let plus = icon_button(ui, Icon::Plus, text::t().add_subtask);
                    ui.data_mut(|d| d.insert_temp(plus.id.with("plus-of"), task.id));
                    if plus.clicked() {
                        state.forms.new_subtask = Some((task.id, String::new()));
                    }
                } else {
                    ui.add_space(LIST_PLUS);
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
