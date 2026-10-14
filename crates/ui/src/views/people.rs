//! People: who can own projects, do tasks and write comments; and who is working right now.

use plnwrk_core::PersonId;

use crate::doc::OpenDoc;
use crate::state::State;
use crate::views::project::person_choice;
use crate::{Clear, Intent, Then, text};

pub fn show(ui: &mut egui::Ui, doc: &OpenDoc, state: &mut State, out: &mut Vec<Intent>) {
    let ws = doc.session.workspace();
    ui.heading(text::PEOPLE);
    ui.add_space(4.0);
    let mut people: Vec<_> = ws.people().collect();
    people.sort_by(|a, b| a.name.cmp(&b.name));
    let choices: Vec<(PersonId, String)> = people.iter().map(|p| (p.id, p.name.clone())).collect();
    ui.horizontal(|ui| {
        ui.label(text::I_AM);
        let mut actor = doc.session.actor();
        if person_choice(ui, "actor", &mut actor, &choices) {
            out.push(Intent::Actor(actor));
        }
    });
    ui.weak("Änderungen und Kommentare werden mit dieser Person vermerkt.");
    ui.add_space(8.0);
    egui::Grid::new("people")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            ui.strong(text::NAME);
            ui.strong(text::INITIALS);
            ui.strong(text::ROLE);
            ui.strong(text::EMAIL);
            ui.end_row();
            for person in &people {
                ui.label(&person.name);
                ui.label(&person.initials);
                ui.label(person.role.as_deref().unwrap_or_default());
                ui.label(person.email.as_deref().unwrap_or_default());
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    let form = &mut state.forms.person;
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut form.name)
                .hint_text(text::NAME)
                .desired_width(180.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut form.initials)
                .hint_text(text::INITIALS)
                .desired_width(50.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut form.role)
                .hint_text(text::ROLE)
                .desired_width(140.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut form.email)
                .hint_text(text::EMAIL)
                .desired_width(180.0),
        );
        if ui.button(text::NEW_PERSON).clicked() {
            out.push(Intent::Run {
                command: "person.create",
                params: form.params(),
                then: Then::Nothing,
                clear: Some(Clear::Person),
            });
        }
    });
}
