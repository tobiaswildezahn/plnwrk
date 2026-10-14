//! Modal dialogs: new project or case, from template, passphrase, unsaved changes, delete.

use egui::{Id, Key, Modal};
use plnwrk_core::PersonId;
use plnwrk_engine::query;
use plnwrk_store::SecretString;
use serde_json::json;

use crate::doc::OpenDoc;
use crate::state::{Dialog, Message, State};
use crate::views::project::person_choice;
use crate::{Clear, Intent, Then, text};

pub fn show(ctx: &egui::Context, doc: Option<&OpenDoc>, state: &mut State, out: &mut Vec<Intent>) {
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
                    ui.text_edit_singleline(&mut form.title)
                        .labelled_by(label.id)
                        .request_focus_once();
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
                    ui.add(egui::TextEdit::singleline(&mut form.start).hint_text("TT.MM.JJJJ"))
                        .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label(text::DUE);
                    ui.add(egui::TextEdit::singleline(&mut form.due).hint_text("TT.MM.JJJJ"))
                        .labelled_by(label.id);
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
                    .map(|d| query::templates(d.session.workspace(), d.session.clock().today()))
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
                    ui.add(egui::TextEdit::singleline(&mut form.start).hint_text("TT.MM.JJJJ"))
                        .labelled_by(label.id);
                    ui.end_row();
                    let label = ui.label(text::DUE);
                    ui.add(egui::TextEdit::singleline(&mut form.due).hint_text("TT.MM.JJJJ"))
                        .labelled_by(label.id);
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
                field.request_focus_once();
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

/// Focus a field once, when the dialog opens.
trait FocusOnce {
    fn request_focus_once(&self);
}

impl FocusOnce for egui::Response {
    fn request_focus_once(&self) {
        let id = self.id.with("focused-once");
        let done = self.ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        if !done {
            self.request_focus();
            self.ctx.data_mut(|d| d.insert_temp(id, true));
        }
    }
}
