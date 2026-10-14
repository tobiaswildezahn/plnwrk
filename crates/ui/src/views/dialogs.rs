//! Modal dialogs: new project or case, from template, passphrase, unsaved changes, delete.

use egui::{Id, Key, Modal};
use plnwrk_core::PersonId;
use plnwrk_engine::query;
use plnwrk_store::SecretString;
use serde_json::json;
use zeroize::Zeroize as _;

use crate::doc::OpenDoc;
use crate::state::{Dialog, Message, State};
use crate::views::date;
use crate::views::project::person_choice;
use crate::{Clear, Intent, Then, text};

pub fn show(ctx: &egui::Context, doc: Option<&OpenDoc>, state: &mut State, out: &mut Vec<Intent>) {
    let serial = state.dialog_serial;
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
    let response = Modal::new(Id::new("dialog")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        match dialog {
            Dialog::NewProject(form) => {
                ui.heading("Neues Projekt oder neuer Vorgang");
                ui.weak("Ein Vorgang ist ein Projekt mit Aktenzeichen und Frist.");
                ui.add_space(6.0);
                egui::Grid::new("new-project").num_columns(2).show(ui, |ui| {
                    let label = ui.label(text::TITLE);
                    let field = ui.text_edit_singleline(&mut form.title).labelled_by(label.id);
                    focus_once(&field, serial);
                    ui.end_row();
                    ui.label(text::KIND);
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut form.kind).desired_width(140.0));
                        for kind in ["Projekt", "Vorgang"] {
                            if ui.small_button(kind).clicked() {
                                form.kind = kind.to_owned();
                            }
                        }
                    });
                    ui.end_row();
                    let label = ui.label(text::REFERENCE);
                    ui.text_edit_singleline(&mut form.reference)
                        .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label(text::START);
                    date::date_input(ui, "new-start", &mut form.start, today, Some(label.id));
                    ui.end_row();
                    let label = ui.label(text::DUE);
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
                    ui.label(text::OWNER);
                    person_choice(ui, "new-owner", &mut form.owner, &people);
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(text::CREATE).clicked() {
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
                    if ui.button(text::CANCEL).clicked() {
                        out.push(Intent::CloseDialog);
                    }
                });
            }
            Dialog::FromTemplate(form) => {
                ui.heading("Aus Vorlage anlegen");
                ui.weak("Aufgaben und Meilensteine der Vorlage werden übernommen und auf den Beginn verschoben.");
                ui.add_space(6.0);
                let templates: Vec<_> = doc
                    .map(|d| query::templates(d.session.workspace(), d.session.clock().now_local()))
                    .unwrap_or_default();
                egui::Grid::new("from-template").num_columns(2).show(ui, |ui| {
                    ui.label(text::TEMPLATE);
                    let current = form
                        .template
                        .and_then(|id| templates.iter().find(|t| t.id == id))
                        .map_or("Bitte wählen", |t| t.title.as_str());
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
                    let label = ui.label(text::TITLE);
                    ui.text_edit_singleline(&mut form.title)
                        .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label(text::REFERENCE);
                    ui.text_edit_singleline(&mut form.reference)
                        .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label(text::START);
                    date::date_input(ui, "template-start", &mut form.start, today, Some(label.id));
                    ui.end_row();
                    let label = ui.label(text::DUE);
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
                    ui.label(text::OWNER);
                    person_choice(ui, "template-owner", &mut form.owner, &people);
                    ui.end_row();
                });
                if templates.is_empty() {
                    ui.weak("Es gibt noch keine Vorlage.");
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(text::CREATE).clicked() {
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
                    if ui.button(text::CANCEL).clicked() {
                        out.push(Intent::CloseDialog);
                    }
                });
            }
            Dialog::Passphrase { path, text: secret } => {
                ui.heading(text::PASSPHRASE);
                ui.label(path.display().to_string());
                ui.label(text::PASSPHRASE_QUESTION);
                let field = ui.add(egui::TextEdit::singleline(secret).password(true));
                focus_once(&field, serial);
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.horizontal(|ui| {
                    if (ui.button("Öffnen").clicked() || entered) && !secret.is_empty() {
                        let secret = SecretString::from(std::mem::take(secret));
                        out.push(Intent::Unlock(path.clone(), secret));
                    }
                    if ui.button(text::CANCEL).clicked() {
                        out.push(Intent::CloseDialog);
                    }
                });
            }
            Dialog::Unsaved(action) => {
                ui.heading(text::UNSAVED_TITLE);
                ui.label(text::UNSAVED_QUESTION);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(text::SAVE).clicked() {
                        out.push(Intent::SaveThen(action.clone()));
                    }
                    if ui.button(text::DISCARD).clicked() {
                        out.push(Intent::Discard(action.clone()));
                    }
                    if ui.button(text::CANCEL).clicked() {
                        out.push(Intent::CloseDialog);
                    }
                });
            }
            Dialog::Protect { first, second } => {
                ui.heading(text::PROTECT.trim_end_matches('…'));
                ui.label(text::PROTECT_HINT);
                ui.add_space(6.0);
                egui::Grid::new("protect").num_columns(2).show(ui, |ui| {
                    let label = ui.label(text::PASSPHRASE);
                    let field = ui
                        .add(egui::TextEdit::singleline(first).password(true))
                        .labelled_by(label.id);
                    focus_once(&field, serial);
                    ui.end_row();
                    let label = ui.label(text::PASSPHRASE_AGAIN);
                    ui.add(egui::TextEdit::singleline(second).password(true))
                        .labelled_by(label.id);
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(text::PROTECT_BUTTON).clicked() {
                        if first != second {
                            message = Some(text::PASSPHRASES_DIFFER.to_owned());
                        } else if !plnwrk_store::passphrase_long_enough(first) {
                            message = Some(text::PASSPHRASE_TOO_SHORT.to_owned());
                        } else {
                            second.zeroize();
                            let secret = SecretString::from(std::mem::take(first));
                            out.push(Intent::Protect(secret));
                        }
                    }
                    if ui.button(text::CANCEL).clicked() {
                        out.push(Intent::CloseDialog);
                    }
                });
            }
            Dialog::Unprotect => {
                ui.heading(format!("{}?", text::UNPROTECT_BUTTON));
                ui.label(text::UNPROTECT_QUESTION);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(text::UNPROTECT_BUTTON).clicked() {
                        out.push(Intent::Unprotect);
                    }
                    if ui.button(text::CANCEL).clicked() {
                        out.push(Intent::CloseDialog);
                    }
                });
            }
            Dialog::DeleteCopy(path) => {
                ui.heading(format!("{}?", text::DELETE_COPY));
                ui.label(
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                );
                ui.label(text::DELETE_COPY_QUESTION);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(text::DELETE).clicked() {
                        out.push(Intent::DeleteCopy(path.clone()));
                    }
                    if ui.button(text::CANCEL).clicked() {
                        out.push(Intent::CloseDialog);
                    }
                });
            }
            Dialog::DeleteTask(task) => {
                ui.heading(text::DELETE_TASK);
                let title = doc
                    .and_then(|d| d.session.workspace().task(*task))
                    .map(|t| t.title.clone())
                    .unwrap_or_default();
                ui.label(format!("„{title}“"));
                ui.label(text::DELETE_TASK_QUESTION);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(text::DELETE).clicked() {
                        out.push(Intent::Run {
                            command: "task.delete",
                            params: json!({"task": task.to_string()}),
                            then: Then::Nothing,
                            clear: Some(Clear::Dialog),
                        });
                    }
                    if ui.button(text::CANCEL).clicked() {
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
