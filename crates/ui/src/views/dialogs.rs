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
                            ui.add(egui::TextEdit::singleline(&mut form.kind).desired_width(140.0));
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
                        date::date_input(ui, "new-start", &mut form.start, today, Some(label.id));
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
                    if ui.button(text::t().create).clicked() {
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
                    .map(|d| query::templates(d.session.workspace(), d.session.clock().now_local()))
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
                    if ui.button(text::t().create).clicked() {
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
            Dialog::Passphrase { path, text: secret } => {
                ui.heading(text::t().passphrase);
                ui.label(path.display().to_string());
                ui.label(text::t().passphrase_question);
                let field = ui.add(egui::TextEdit::singleline(secret).password(true));
                focus_once(&field, serial);
                let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.horizontal(|ui| {
                    if (ui.button(text::t().open).clicked() || entered) && !secret.is_empty() {
                        let secret = SecretString::from(std::mem::take(secret));
                        out.push(Intent::Unlock(path.clone(), secret));
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
                    if ui.button(text::t().save).clicked() {
                        out.push(Intent::SaveThen(action.clone()));
                    }
                    if ui.button(text::t().discard).clicked() {
                        out.push(Intent::Discard(action.clone()));
                    }
                    if ui.button(text::t().cancel).clicked() {
                        out.push(Intent::CloseDialog);
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
                    if ui.button(text::t().protect_button).clicked() {
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
                    if ui.button(text::t().unprotect_button).clicked() {
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
                    if ui.button(text::t().delete).clicked() {
                        out.push(Intent::DeleteCopy(path.clone()));
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
                    if ui.button(text::t().delete).clicked() {
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
