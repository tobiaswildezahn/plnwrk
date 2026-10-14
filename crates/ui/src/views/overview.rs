//! The overview: one card per project or case, then everything overdue. And the templates.

use egui::RichText;
use plnwrk_core::{Classification, EntityKey};
use plnwrk_engine::query::{self, ProjectCard};

use crate::doc::OpenDoc;
use crate::state::{Dialog, TemplateForm, View};
use crate::{Intent, dates, text};

const CARD_WIDTH: f32 = 270.0;

pub fn show(ui: &mut egui::Ui, doc: &OpenDoc, out: &mut Vec<Intent>) {
    let ws = doc.session.workspace();
    let today = doc.session.clock().today();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(text::OVERVIEW);
        ui.add_space(8.0);
        let cards = query::overview(ws, today);
        if cards.is_empty() {
            ui.label(text::NOTHING_YET);
        }
        ui.horizontal_wrapped(|ui| {
            for card in &cards {
                project_card(ui, card, out, None);
            }
        });
        ui.add_space(16.0);
        ui.heading(text::OVERDUE);
        let overdue = query::overdue(ws, today);
        if overdue.is_empty() {
            ui.weak(text::NOTHING_OVERDUE);
        }
        for item in overdue {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(text::days_left(-item.days_late))
                        .color(ui.visuals().error_fg_color),
                );
                let project = ws
                    .project(item.project)
                    .map(|p| p.title.as_str())
                    .unwrap_or_default();
                let label = if matches!(item.entity, EntityKey::Project(_)) {
                    item.title.clone()
                } else {
                    format!("{} ({project})", item.title)
                };
                if ui.link(label).clicked() {
                    out.push(Intent::Go(View::Project(item.project)));
                    if let EntityKey::Task(task) = item.entity {
                        out.push(Intent::Select(Some(task)));
                    }
                }
            });
        }
    });
}

/// One project card. `action`: an extra button below (for templates).
fn project_card(
    ui: &mut egui::Ui,
    card: &ProjectCard,
    out: &mut Vec<Intent>,
    action: Option<(&str, Intent)>,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(CARD_WIDTH);
        ui.vertical(|ui| {
            let mut line = Vec::new();
            if let Some(kind) = &card.kind {
                line.push(kind.clone());
            }
            if let Some(reference) = &card.reference {
                line.push(reference.clone());
            }
            line.push(text::project_status(card.status).to_owned());
            ui.weak(line.join(" · "));
            if ui
                .link(RichText::new(&card.title).strong().size(16.0))
                .clicked()
            {
                out.push(Intent::Go(View::Project(card.id)));
            }
            if card.classification >= Classification::VsNfd {
                ui.label(
                    RichText::new("VS-NfD")
                        .small()
                        .color(ui.visuals().error_fg_color),
                );
            }
            if let (Some(due), Some(days)) = (card.due, card.days_left) {
                let line = format!(
                    "{} {} · {}",
                    text::DUE,
                    dates::format(due),
                    text::days_left(days)
                );
                if days < 0 {
                    ui.label(RichText::new(line).color(ui.visuals().error_fg_color));
                } else {
                    ui.label(line);
                }
            }
            ui.add(
                egui::ProgressBar::new(f32::from(card.progress.percent) / 100.0).text(format!(
                    "{} % · {} von {} erledigt",
                    card.progress.percent, card.progress.done, card.progress.tasks
                )),
            );
            let mut facts = vec![format!("{} offen", card.open_tasks)];
            if card.overdue_tasks > 0 {
                facts.push(format!("{} überfällig", card.overdue_tasks));
            }
            if let Some(owner) = &card.owner {
                facts.push(owner.name.clone());
            }
            ui.weak(facts.join(" · "));
            if let Some(milestone) = &card.next_milestone {
                ui.weak(format!(
                    "♦ {} {} ({})",
                    dates::format(milestone.date),
                    milestone.title,
                    text::milestone_state(milestone.state)
                ));
            }
            if let Some((label, intent)) = action
                && ui.button(label).clicked()
            {
                out.push(intent);
            }
        });
    });
}

/// The templates, each with a button to create a project or case from it.
pub fn templates(ui: &mut egui::Ui, doc: &OpenDoc, out: &mut Vec<Intent>) {
    let ws = doc.session.workspace();
    let today = doc.session.clock().today();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(text::TEMPLATES);
        ui.weak("Eine Vorlage ist ein Projekt mit festem Ablauf. Daraus entstehen neue Vorgänge.");
        ui.add_space(8.0);
        let cards = query::templates(ws, today);
        if cards.is_empty() {
            ui.label("Noch keine Vorlagen. Lege ein Projekt an und markiere es als Vorlage.");
        }
        ui.horizontal_wrapped(|ui| {
            for card in &cards {
                let form = TemplateForm {
                    template: Some(card.id),
                    start: dates::format(today),
                    ..TemplateForm::default()
                };
                project_card(
                    ui,
                    card,
                    out,
                    Some((
                        text::FROM_TEMPLATE,
                        Intent::Open(Dialog::FromTemplate(form)),
                    )),
                );
            }
        });
    });
}
