//! Modal dialogs: new project or case, from template, passphrase, unsaved changes, delete.

use egui::{Id, Key, Modal};
use plnwrk_core::PersonId;
use plnwrk_engine::query;
use plnwrk_store::SecretString;
use serde_json::json;
use zeroize::Zeroize as _;

use crate::design::tokens::space;
use crate::design::widgets::{self as w, Kind};
use crate::doc::OpenDoc;
use crate::state::{Dialog, Message, State};
use crate::views::date;
use crate::views::project::person_choice;
use crate::{Clear, Intent, Then, text};

pub fn show(ctx: &egui::Context, doc: Option<&OpenDoc>, state: &mut State, out: &mut Vec<Intent>) {
    let serial = state.dialog_serial;
    // Built before the dialog is borrowed: the palette offers what fits the current view.
    let mut offered = matches!(state.dialog, Some(Dialog::Palette { .. }))
        .then(|| crate::palette::entries(doc, state));
    // A refusal while the dialog is open is said in it, not behind it (one message, one
    // place: the message bar waits while a dialog is open). Only one that came up in it.
    let refusal = state.said_in_dialog();
    let Some(dialog) = &mut state.dialog else {
        return;
    };
    let people: Vec<(PersonId, String)> = doc
        .map(|d| {
            d.session
                .workspace()
                .people()
                .map(|p| (p.id, p.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    let today = doc.map_or_else(|| jiff::Zoned::now().date(), |d| d.session.clock().today());
    let mut message = None;
    let response = Modal::new(Id::new("dialog"))
        .frame(w::dialog_frame(ctx))
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            match dialog {
                Dialog::Palette { query, selected } => {
                    palette(
                        ui,
                        offered.take().unwrap_or_default(),
                        query,
                        selected,
                        serial,
                        out,
                    );
                }
                Dialog::NewProject(form) => {
                    new_project(ui, form, &people, today, serial, out);
                }
                Dialog::FromTemplate(form) => {
                    ui.heading(text::t().from_template_title);
                    ui.weak(text::t().from_template_hint);
                    ui.add_space(space::S);
                    let templates: Vec<_> = doc
                        .map(|d| {
                            query::templates(d.session.workspace(), d.session.clock().now_local())
                        })
                        .unwrap_or_default();
                    egui::Grid::new("from-template")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label(text::t().template);
                            let current = form
                                .template
                                .and_then(|id| templates.iter().find(|t| t.id == id))
                                .map_or(text::t().please_choose, |t| t.title.as_str());
                            egui::ComboBox::from_id_salt("template")
                                .selected_text(current)
                                .show_ui(ui, |ui| {
                                    for template in &templates {
                                        ui.selectable_value(
                                            &mut form.template,
                                            Some(template.id),
                                            &template.title,
                                        );
                                    }
                                });
                            ui.end_row();
                            let label = ui.label(text::t().title);
                            ui.text_edit_singleline(&mut form.title)
                                .labelled_by(label.id);
                            ui.end_row();
                            let label = ui.label(text::t().reference);
                            ui.text_edit_singleline(&mut form.reference)
                                .labelled_by(label.id);
                            ui.end_row();
                            let label = ui.label(text::t().start);
                            date::date_input(
                                ui,
                                "template-start",
                                &mut form.start,
                                today,
                                Some(label.id),
                            );
                            ui.end_row();
                            let label = ui.label(text::t().due);
                            // The one rule: the template's kind and the reference typed.
                            let kind = form
                                .template
                                .and_then(|id| templates.iter().find(|t| t.id == id))
                                .and_then(|t| t.kind.clone());
                            let case = plnwrk_core::is_case(kind.as_deref(), Some(&form.reference));
                            date::deadline_input(
                                ui,
                                "template-due",
                                &mut form.due,
                                &mut form.due_time,
                                today,
                                case,
                                Some(label.id),
                            );
                            ui.end_row();
                            ui.label(text::t().owner);
                            person_choice(ui, "template-owner", &mut form.owner, &people);
                            ui.end_row();
                        });
                    if templates.is_empty() {
                        ui.weak(text::t().no_template);
                    }
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().create).clicked() {
                            match form.params() {
                                Ok(params) => out.push(Intent::Run {
                                    command: "project.instantiate",
                                    params,
                                    then: Then::ShowProject,
                                    clear: Some(Clear::Dialog),
                                }),
                                Err(error) => message = Some(error),
                            }
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::Passphrase {
                    source,
                    text: secret,
                } => {
                    ui.heading(text::t().passphrase);
                    ui.label(source.label());
                    ui.label(text::t().passphrase_question);
                    let field = ui.add(egui::TextEdit::singleline(secret).password(true));
                    focus_once(&field, serial);
                    let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.horizontal(|ui| {
                        if (w::button(ui, Kind::Primary, text::t().open).clicked() || entered)
                            && !secret.is_empty()
                        {
                            let secret = SecretString::from(std::mem::take(secret));
                            out.push(Intent::Unlock(source.clone(), secret));
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::Unsaved(action) => {
                    ui.heading(text::t().unsaved_title);
                    ui.label(text::t().unsaved_question);
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().save).clicked() {
                            out.push(Intent::SaveThen(action.clone()));
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                        // Discarding is set apart from saving: no slip of the pointer.
                        ui.add_space(space::XL);
                        if w::button(ui, Kind::Destructive, text::t().discard).clicked() {
                            out.push(Intent::Discard(action.clone()));
                        }
                    });
                }
                Dialog::Protect { first, second } => {
                    ui.heading(text::t().protect.trim_end_matches('…'));
                    ui.label(text::t().protect_hint);
                    ui.add_space(space::S);
                    egui::Grid::new("protect").num_columns(2).show(ui, |ui| {
                        let label = ui.label(text::t().passphrase);
                        let field = ui
                            .add(egui::TextEdit::singleline(first).password(true))
                            .labelled_by(label.id);
                        focus_once(&field, serial);
                        ui.end_row();
                        let label = ui.label(text::t().passphrase_again);
                        ui.add(egui::TextEdit::singleline(second).password(true))
                            .labelled_by(label.id);
                        ui.end_row();
                    });
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().protect_button).clicked() {
                            if first != second {
                                message = Some(text::t().passphrases_differ.to_owned());
                            } else if !plnwrk_store::passphrase_long_enough(first) {
                                message = Some(text::t().passphrase_too_short.to_owned());
                            } else {
                                second.zeroize();
                                let secret = SecretString::from(std::mem::take(first));
                                out.push(Intent::Protect(secret));
                            }
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::Unprotect => {
                    ui.heading(format!("{}?", text::t().unprotect_button));
                    ui.label(text::t().unprotect_question);
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().unprotect_button).clicked() {
                            out.push(Intent::Unprotect);
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::DeleteCopy(path) => {
                    ui.heading(format!("{}?", text::t().delete_copy));
                    ui.label(
                        path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    );
                    ui.label(text::t().delete_copy_question);
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::DeleteCopy(path.clone()));
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::DeleteStoredCopy { key, name } => {
                    ui.heading(format!("{}?", text::t().delete_copy));
                    ui.label(name.as_str());
                    ui.label(text::t().delete_copy_question);
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::DeleteStoredCopy(key.clone()));
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::DeleteAllStoredCopies => {
                    ui.heading(format!("{}?", text::t().delete_all_copies));
                    ui.label(text::t().delete_all_copies_question);
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::DeleteAllStoredCopies);
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::CloseSubtasks { task, params, open } => {
                    ui.heading(text::t().close_subtasks_title);
                    let title = doc
                        .and_then(|d| d.session.workspace().task(*task))
                        .map(|t| t.title.clone())
                        .unwrap_or_default();
                    ui.label(text::quoted(&title));
                    ui.label(text::subtasks_still_open(*open));
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        let mut with = params.clone();
                        if let Some(fields) = with.as_object_mut() {
                            fields.insert("close_subtasks".to_owned(), json!(true));
                        }
                        for (kind, label, params) in [
                            (Kind::Primary, text::t().close_all_subtasks, with),
                            (Kind::Secondary, text::t().only_this_task, params.clone()),
                        ] {
                            if w::button(ui, kind, label).clicked() {
                                out.push(Intent::Run {
                                    command: "task.move",
                                    params,
                                    then: Then::Nothing,
                                    clear: Some(Clear::Dialog),
                                });
                            }
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::ConfirmMilestone {
                    command,
                    params,
                    then,
                    clear,
                    changes,
                } => {
                    ui.heading(text::t().confirm_milestone_title);
                    ui.weak(text::t().confirm_milestone_hint);
                    ui.add_space(space::S);
                    for change in changes.iter() {
                        w::subheading(ui, &format!("♦ {}", change.title));
                        for line in milestone_lines(change) {
                            ui.label(line);
                        }
                        if change.late_predecessors > 0 {
                            w::field_error(ui, &text::late_predecessors(change.late_predecessors));
                        }
                        if change.after_project_due {
                            w::field_error(ui, text::t().after_project_due_then);
                        }
                        ui.add_space(space::S);
                    }
                    let along = if *command == "task.reschedule" {
                        crate::along_of(params)
                    } else {
                        0
                    };
                    if along > 0 {
                        ui.label(text::moves_along(along));
                        ui.add_space(space::S);
                    }
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().change_button).clicked() {
                            out.push(Intent::CloseDialog);
                            out.push(Intent::Confirmed(Box::new(Intent::Run {
                                command,
                                params: params.clone(),
                                then: *then,
                                clear: *clear,
                            })));
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::DeclineMilestone);
                        }
                    });
                }
                Dialog::DeleteTask(task) => {
                    ui.heading(text::t().delete_task);
                    let title = doc
                        .and_then(|d| d.session.workspace().task(*task))
                        .map(|t| t.title.clone())
                        .unwrap_or_default();
                    ui.label(text::quoted(&title));
                    ui.label(text::t().delete_task_question);
                    ui.add_space(space::S);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::Confirmed(Box::new(Intent::Run {
                                command: "task.delete",
                                params: json!({"task": task.to_string()}),
                                then: Then::Nothing,
                                clear: Some(Clear::Dialog),
                            })));
                        }
                        if w::button(ui, Kind::Secondary, text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
            }
            if let Some(refusal) = &refusal {
                ui.add_space(space::S);
                w::field_error(ui, &refusal.text);
            }
        });
    // Escape or a click beside it: for a milestone the same as "Abbrechen" (the typed value
    // goes back to the stored one).
    if response.should_close() && !matches!(dialog, Dialog::Unsaved(_)) {
        match dialog {
            Dialog::ConfirmMilestone { .. } => out.push(Intent::DeclineMilestone),
            // Escape while a person is being added drops that person, not the whole form.
            Dialog::NewProject(form) if form.new_person.is_some() => form.new_person = None,
            _ => out.push(Intent::CloseDialog),
        }
    }
    if let Some(error) = message {
        state.say(Message::error(error));
    }
}

/// How many palette results are shown at most.
const MAX_SHOWN: usize = 100;

/// The command palette: a search field over a list; arrows choose, Enter or a click runs.
fn palette(
    ui: &mut egui::Ui,
    offered: Vec<crate::palette::Entry>,
    query: &mut String,
    selected: &mut usize,
    serial: u64,
    out: &mut Vec<Intent>,
) {
    let (down, up) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
        )
    });
    let label = ui.label(text::t().palette.trim_end_matches('…'));
    let field = ui
        .add(
            egui::TextEdit::singleline(query)
                .hint_text(text::t().palette_hint)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
    focus_once(&field, serial);
    let found = crate::palette::search(offered, query);
    if field.changed() {
        *selected = 0;
    }
    let last = found.len().min(MAX_SHOWN).saturating_sub(1);
    if down {
        *selected = (*selected + 1).min(last);
    }
    if up {
        *selected = selected.saturating_sub(1);
    }
    *selected = (*selected).min(last);
    let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let mut chosen = (entered && !found.is_empty()).then_some(*selected);
    if entered && found.is_empty() {
        field.request_focus();
    }
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            if found.is_empty() {
                ui.weak(text::t().palette_empty);
            }
            // Only the best ones are laid out: thousands of tasks must not slow down typing.
            for (index, entry) in found.iter().enumerate().take(MAX_SHOWN) {
                ui.horizontal(|ui| {
                    let response = ui.selectable_label(index == *selected, &entry.label);
                    if index == *selected && (up || down) {
                        response.scroll_to_me(None);
                    }
                    if response.clicked() {
                        chosen = Some(index);
                    }
                    ui.weak(&entry.detail);
                    if let Some(shortcut) = &entry.shortcut {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(ui.ctx().format_shortcut(shortcut));
                        });
                    }
                });
            }
            if found.len() > MAX_SHOWN {
                ui.weak(text::t().more_results);
            }
        });
    if let Some(entry) = chosen.and_then(|index| found.into_iter().nth(index)) {
        out.push(Intent::CloseDialog);
        out.extend(entry.intents);
    }
}

/// Focuses a field once per opened dialog (`serial` counts opened dialogs).
fn focus_once(response: &egui::Response, serial: u64) {
    let id = Id::new(("dialog-focus", serial));
    let done = response
        .ctx
        .data(|d| d.get_temp::<bool>(id))
        .unwrap_or(false);
    if !done {
        response.request_focus();
        response.ctx.data_mut(|d| d.insert_temp(id, true));
    }
}

/// What changes at a milestone, one line each: "Termin: 24.10.2026 → 18.10.2026".
fn milestone_lines(change: &plnwrk_engine::milestones::MilestoneChange) -> Vec<String> {
    use plnwrk_engine::milestones::Field;
    let t = text::t();
    let day = |d: Option<jiff::civil::Date>| d.map_or_else(|| "–".to_owned(), crate::dates::format);
    let deadline = |d: Option<(jiff::civil::Date, Option<jiff::civil::Time>)>| {
        d.map_or_else(
            || "–".to_owned(),
            |(day, time)| crate::dates::format_deadline(day, time),
        )
    };
    change
        .fields
        .iter()
        .map(|field| match field {
            Field::Title { old, new } => format!("{}: {old} → {new}", t.title),
            Field::Start { old, new } => format!("{}: {} → {}", t.start, day(*old), day(*new)),
            Field::Due { old, new } => {
                format!(
                    "{}: {} → {}",
                    t.milestone_date,
                    deadline(*old),
                    deadline(*new)
                )
            }
            Field::Status { old, new } => format!(
                "{}: {} → {}",
                t.status,
                text::task_status(*old),
                text::task_status(*new)
            ),
            Field::Deleted => t.milestone_deleted.to_owned(),
            Field::Unmarked => t.milestone_unmarked.to_owned(),
            Field::Marked => t.milestone_marked.to_owned(),
        })
        .collect()
}

/// The dialog for a new case or project (Phase 9f): the choice first, each with a sentence
/// and an example; the reference only for a case; mistakes said at their field; "Anlegen"
/// (or Enter) only once a title stands; a person added right in the choice.
fn new_project(
    ui: &mut egui::Ui,
    form: &mut crate::state::ProjectForm,
    people: &[(PersonId, String)],
    today: jiff::civil::Date,
    serial: u64,
    out: &mut Vec<Intent>,
) {
    let t = text::t();
    let [project_kind, case_kind] = t.kinds;
    ui.heading(t.new_project_title);
    ui.add_space(space::S);
    // The choice writes the kind; a refined kind ("SKA") counts as a case (one rule).
    let mut case = plnwrk_core::is_case(Some(&form.kind), None);
    if w::segmented(ui, &mut case, &[(true, case_kind), (false, project_kind)]) {
        form.kind = if case { case_kind } else { project_kind }.to_owned();
    }
    ui.weak(if case {
        t.case_explained
    } else {
        t.project_explained
    });
    ui.add_space(space::S);
    // A date is wrong only once its field is left (not while it is typed).
    let mut wrong_dates = false;
    let mut too_long = false;
    // Enter counts where it was pressed in one of the form's own fields (on a button it is a
    // click of that button – "Abbrechen" must not create).
    let enter = ui.input(|i| i.key_pressed(Key::Enter));
    let mut enter_in_field = false;
    // The person's field takes the cursor once, in the frame after it was chosen.
    let focus_id = Id::new(("new-person-focus", serial));
    let focus_person = ui
        .data_mut(|d| d.remove_temp::<bool>(focus_id))
        .unwrap_or(false);
    let mut chosen_now = false;
    egui::Grid::new("new-project")
        .num_columns(2)
        .show(ui, |ui| {
            let label = ui.label(t.title);
            ui.vertical(|ui| {
                let field = ui
                    .text_edit_singleline(&mut form.title)
                    .labelled_by(label.id);
                focus_once(&field, serial);
                enter_in_field |= field.lost_focus() && enter;
                // Said here, before the engine would refuse it (one message, one place).
                if form.title.trim().chars().count() > plnwrk_core::text::TITLE_MAX {
                    too_long = true;
                    w::field_error(ui, &text::title_too_long(plnwrk_core::text::TITLE_MAX));
                }
            });
            ui.end_row();
            // A project keeps what was typed here, but does not get it.
            if case {
                let label = ui.label(t.reference);
                let field = ui
                    .text_edit_singleline(&mut form.reference)
                    .labelled_by(label.id);
                enter_in_field |= field.lost_focus() && enter;
                ui.end_row();
            }
            let label = ui.label(t.start);
            ui.vertical(|ui| {
                let input =
                    date::date_input(ui, "new-start", &mut form.start, today, Some(label.id));
                enter_in_field |= input.response.lost_focus() && enter;
                if let Err(error) = crate::dates::parse(&form.start) {
                    wrong_dates = true;
                    if !input.response.has_focus() {
                        w::field_error(ui, &error);
                    }
                }
            });
            ui.end_row();
            let label = ui.label(t.due);
            ui.vertical(|ui| {
                let input = date::deadline_input(
                    ui,
                    "new-due",
                    &mut form.due,
                    &mut form.due_time,
                    today,
                    case,
                    Some(label.id),
                );
                enter_in_field |= (input.response.lost_focus() || input.time_done) && enter;
                if let Err(error) = crate::dates::parse_deadline(&form.due, &form.due_time) {
                    wrong_dates = true;
                    if !input.response.has_focus() {
                        w::field_error(ui, &error);
                    }
                }
            });
            ui.end_row();
            let label = ui.label(t.owner);
            ui.vertical(|ui| {
                let current = form
                    .owner
                    .and_then(|id| people.iter().find(|(p, _)| *p == id))
                    .map_or(t.nobody, |(_, name)| name.as_str());
                egui::ComboBox::from_id_salt("new-owner")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut form.owner, None, t.nobody);
                        for (person, name) in people {
                            ui.selectable_value(&mut form.owner, Some(*person), name);
                        }
                        if ui.selectable_label(false, t.add_person).clicked() {
                            form.new_person = Some(String::new());
                            chosen_now = true;
                        }
                    })
                    .response
                    .labelled_by(label.id);
                // A new person, named here: Enter (or the button) adds them and puts them in
                // charge; leaving the field empty drops it, Escape too (see `show`).
                let mut drop_person = false;
                if let Some(name) = &mut form.new_person {
                    ui.horizontal(|ui| {
                        let field = ui.add(
                            egui::TextEdit::singleline(name)
                                .hint_text(t.name)
                                .desired_width(160.0),
                        );
                        let field = field.labelled_by(label.id);
                        // The cursor goes there once, when it is chosen – not again and again.
                        if focus_person || chosen_now {
                            field.request_focus();
                        }
                        let entered = field.lost_focus() && enter;
                        let add = w::button(ui, Kind::Secondary, t.create).clicked();
                        if (entered || add) && !name.trim().is_empty() {
                            out.push(Intent::Run {
                                command: "person.create",
                                params: json!({"name": name.trim()}),
                                then: Then::OwnerOfNew,
                                clear: None,
                            });
                        } else if field.lost_focus()
                            && name.trim().is_empty()
                            && !ui.input(|i| i.key_pressed(Key::Escape))
                        {
                            // Left empty (not by Escape: that is handled in `show`).
                            drop_person = true;
                        }
                    });
                }
                if drop_person {
                    form.new_person = None;
                }
            });
            ui.end_row();
        });
    // Chosen in the list just now: the cursor goes there once more next frame (the list
    // closing may take it back), then never again.
    if chosen_now {
        ui.data_mut(|d| d.insert_temp(focus_id, true));
    }
    ui.add_space(space::S);
    let titled = !form.title.trim().is_empty();
    // A name typed for a new person and not added yet would be lost: added (or emptied) first.
    let person_pending = form
        .new_person
        .as_ref()
        .is_some_and(|name| !name.trim().is_empty());
    let ready = titled && !wrong_dates && !too_long && !person_pending;
    let entered = enter_in_field;
    ui.horizontal(|ui| {
        let create = ui
            .add_enabled_ui(ready, |ui| w::button(ui, Kind::Primary, t.create))
            .inner
            .on_disabled_hover_text(if titled { "" } else { t.title_needed });
        if (create.clicked() || entered) && ready {
            let mut form = form.clone();
            // The reference only for a case: a project with one would count as a case.
            if !case {
                form.reference.clear();
            }
            match form.create_params() {
                Ok(params) => out.push(Intent::Run {
                    command: "project.create",
                    params,
                    then: Then::ShowProject,
                    clear: Some(Clear::Dialog),
                }),
                // Not expected (the dates are checked above), but never swallowed.
                Err(error) => out.push(Intent::Say(Message::error(error))),
            }
        }
        if w::button(ui, Kind::Secondary, t.cancel).clicked() {
            out.push(Intent::CloseDialog);
        }
    });
    if !titled {
        ui.weak(t.title_needed);
    } else if person_pending {
        ui.weak(t.person_pending);
    }
}
