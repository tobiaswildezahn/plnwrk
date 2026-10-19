//! People: who can own projects, do tasks and write comments; and who is working right now.

use plnwrk_core::PersonId;

use crate::doc::OpenDoc;
use crate::state::State;
use crate::views::project::person_choice;
use crate::{Clear, Intent, Then, text};

pub fn show(ui: &mut egui::Ui, doc: &OpenDoc, state: &mut State, out: &mut Vec<Intent>) {
    let ws = doc.session.workspace();
    ui.heading(text::t().people);
    ui.add_space(4.0);
    let mut people: Vec<_> = ws.people().collect();
    people.sort_by(|a, b| a.name.cmp(&b.name));
    let choices: Vec<(PersonId, String)> = people.iter().map(|p| (p.id, p.name.clone())).collect();
    ui.horizontal(|ui| {
        ui.label(text::t().i_am);
        let mut actor = doc.session.actor();
        if person_choice(ui, "actor", &mut actor, &choices) {
            out.push(Intent::Actor(actor));
        }
    });
    ui.weak(text::t().i_am_hint);
    ui.add_space(8.0);
    // Nobody yet: a symbol, a sentence, the step that helps (Phase 9f).
    if people.is_empty()
        && crate::design::widgets::empty_state(
            ui,
            crate::design::Icon::Person,
            text::t().people_empty,
            Some(text::t().add_person),
        )
    {
        state.focus_new_person = true;
    }
    let me = doc.session.actor();
    egui::Grid::new("people")
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            ui.strong(text::t().name);
            ui.strong(text::t().initials);
            ui.strong(text::t().role);
            ui.strong(text::t().email);
            ui.label("");
            ui.end_row();
            for person in &people {
                if me == Some(person.id) {
                    ui.label(format!("{} {}", person.name, text::t().me_marker));
                } else {
                    ui.label(&person.name);
                }
                ui.label(&person.initials);
                ui.label(person.role.as_deref().unwrap_or_default());
                ui.label(person.email.as_deref().unwrap_or_default());
                // Named for the screen reader: which person it changes.
                let name = format!("{} · {}", text::t().edit_person, person.name);
                let edit = crate::design::widgets::button(
                    ui,
                    crate::design::widgets::Kind::Quiet,
                    text::t().edit,
                );
                edit.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                });
                if edit.clicked() {
                    out.push(Intent::Open(crate::state::Dialog::EditPerson {
                        person: person.id,
                        form: crate::state::PersonForm::of(person),
                    }));
                }
                ui.end_row();
            }
        });
    ui.add_space(8.0);
    let focus = std::mem::take(&mut state.focus_new_person);
    let form = &mut state.forms.person;
    ui.horizontal(|ui| {
        let name = ui.add(
            egui::TextEdit::singleline(&mut form.name)
                .hint_text(text::t().name)
                .desired_width(180.0),
        );
        if focus {
            name.request_focus();
        }
        ui.add(
            egui::TextEdit::singleline(&mut form.initials)
                .hint_text(text::t().initials)
                .desired_width(50.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut form.role)
                .hint_text(text::t().role)
                .desired_width(140.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut form.email)
                .hint_text(text::t().email)
                .desired_width(180.0),
        );
        if let Some(same) = same_person(ws, &form.name, &form.initials, None) {
            crate::design::widgets::field_error(ui, &text::same_person(&same));
        }
        if ui.button(text::t().new_person).clicked() {
            out.push(Intent::Run {
                command: "person.create",
                params: form.params(),
                then: Then::Nothing,
                clear: Some(Clear::Person),
            });
        }
    });
}

/// Someone else (not `except`) with this name or these initials, ignoring case – to warn
/// before the same person is added twice (Phase 10a, shared files).
pub(crate) fn same_person(
    ws: &plnwrk_core::Workspace,
    name: &str,
    initials: &str,
    except: Option<PersonId>,
) -> Option<String> {
    let (name, initials) = (name.trim().to_lowercase(), initials.trim().to_lowercase());
    ws.people()
        .filter(|p| Some(p.id) != except)
        .find(|p| {
            (!name.is_empty() && p.name.to_lowercase() == name)
                || (!initials.is_empty() && p.initials.to_lowercase() == initials)
        })
        .map(|p| p.name.clone())
}
