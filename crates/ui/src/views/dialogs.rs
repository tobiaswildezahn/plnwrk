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
                    ui.heading(text::t().new_project_title);
                    ui.weak(text::t().new_project_hint);
                    ui.add_space(6.0);
                    egui::Grid::new("new-project")
                        .num_columns(2)
                        .show(ui, |ui| {
                            let label = ui.label(text::t().title);
                            let field = ui
                                .text_edit_singleline(&mut form.title)
                                .labelled_by(label.id);
                            focus_once(&field, serial);
                            ui.end_row();
                            ui.label(text::t().kind);
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut form.kind).desired_width(140.0),
                                );
                                for kind in text::t().kinds {
                                    if ui.small_button(kind).clicked() {
                                        form.kind = kind.to_owned();
                                    }
                                }
                            });
                            ui.end_row();
                            let label = ui.label(text::t().reference);
                            ui.text_edit_singleline(&mut form.reference)
                                .labelled_by(label.id);
                            ui.end_row();
                            let label = ui.label(text::t().start);
                            date::date_input(
                                ui,
                                "new-start",
                                &mut form.start,
                                today,
                                Some(label.id),
                            );
                            ui.end_row();
                            let label = ui.label(text::t().due);
                            let case = !form.reference.trim().is_empty();
                            date::deadline_input(
                                ui,
                                "new-due",
                                &mut form.due,
                                &mut form.due_time,
                                today,
                                case,
                                Some(label.id),
                            );
                            ui.end_row();
                            ui.label(text::t().owner);
                            person_choice(ui, "new-owner", &mut form.owner, &people);
                            ui.end_row();
                        });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().create).clicked() {
                            match form.create_params() {
                                Ok(params) => out.push(Intent::Run {
                                    command: "project.create",
                                    params,
                                    then: Then::ShowProject,
                                    clear: Some(Clear::Dialog),
                                }),
                                Err(error) => message = Some(error),
                            }
                        }
                        if ui.button(text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::FromTemplate(form) => {
                    ui.heading(text::t().from_template_title);
                    ui.weak(text::t().from_template_hint);
                    ui.add_space(6.0);
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
                            let case = !form.reference.trim().is_empty();
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
                    ui.add_space(6.0);
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
                        if ui.button(text::t().cancel).clicked() {
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
                        if ui.button(text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::Unsaved(action) => {
                    ui.heading(text::t().unsaved_title);
                    ui.label(text::t().unsaved_question);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().save).clicked() {
                            out.push(Intent::SaveThen(action.clone()));
                        }
                        if ui.button(text::t().cancel).clicked() {
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
                    ui.add_space(6.0);
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
                    ui.add_space(6.0);
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
                        if ui.button(text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::Unprotect => {
                    ui.heading(format!("{}?", text::t().unprotect_button));
                    ui.label(text::t().unprotect_question);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Primary, text::t().unprotect_button).clicked() {
                            out.push(Intent::Unprotect);
                        }
                        if ui.button(text::t().cancel).clicked() {
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
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::DeleteCopy(path.clone()));
                        }
                        if ui.button(text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::DeleteStoredCopy { key, name } => {
                    ui.heading(format!("{}?", text::t().delete_copy));
                    ui.label(name.as_str());
                    ui.label(text::t().delete_copy_question);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::DeleteStoredCopy(key.clone()));
                        }
                        if ui.button(text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
                Dialog::DeleteAllStoredCopies => {
                    ui.heading(format!("{}?", text::t().delete_all_copies));
                    ui.label(text::t().delete_all_copies_question);
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::DeleteAllStoredCopies);
                        }
                        if ui.button(text::t().cancel).clicked() {
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
                    ui.add_space(6.0);
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
                        if ui.button(text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
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
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if w::button(ui, Kind::Destructive, text::t().delete).clicked() {
                            out.push(Intent::Run {
                                command: "task.delete",
                                params: json!({"task": task.to_string()}),
                                then: Then::Nothing,
                                clear: Some(Clear::Dialog),
                            });
                        }
                        if ui.button(text::t().cancel).clicked() {
                            out.push(Intent::CloseDialog);
                        }
                    });
                }
            }
        });
    if response.should_close() && !matches!(dialog, Dialog::Unsaved(_)) {
        out.push(Intent::CloseDialog);
    }
    if let Some(error) = message {
        state.message = Some(Message::error(error));
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
