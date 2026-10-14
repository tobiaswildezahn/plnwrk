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
    let now = doc.session.clock().now_local();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(text::t().overview);
        ui.add_space(8.0);
        let cards = query::overview(ws, now);
        if cards.is_empty() {
            ui.label(text::t().nothing_yet);
        }
        ui.horizontal_wrapped(|ui| {
            for card in &cards {
                project_card(ui, card, now, out, None);
            }
        });
        ui.add_space(16.0);
        ui.heading(text::t().overdue);
        let overdue = query::overdue(ws, now);
        if overdue.is_empty() {
            ui.weak(text::t().nothing_overdue);
        }
        for item in overdue {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(text::deadline_state(item.due, item.due_time, now).0)
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
    now: jiff::civil::DateTime,
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
            if let Some(due) = card.due {
                let (state, past) = text::deadline_state(due, card.due_time, now);
                let shown = dates::format_deadline(due, card.due_time);
                // Templates, finished projects: the date only, never "overdue".
                let line = if card.deadlines_count {
                    format!("{} {shown} · {state}", text::t().due)
                } else {
                    format!("{} {shown}", text::t().due)
                };
                if past && card.deadlines_count {
                    ui.label(RichText::new(line).color(ui.visuals().error_fg_color));
                } else {
                    ui.label(line);
                }
            }
            ui.add(
                egui::ProgressBar::new(f32::from(card.progress.percent) / 100.0).text(
                    text::progress(
                        card.progress.percent,
                        card.progress.done,
                        card.progress.tasks,
                    ),
                ),
            );
            let mut facts = vec![text::open_tasks(card.open_tasks)];
            if card.overdue_tasks > 0 {
                facts.push(text::overdue_tasks(card.overdue_tasks));
            }
            if let Some(owner) = &card.owner {
                facts.push(owner.name.clone());
            }
            ui.weak(facts.join(" · "));
            if let Some(milestone) = &card.next_milestone {
                let when = milestone
                    .date
                    .map(|day| format!("{} ", dates::format_deadline(day, milestone.time)))
                    .unwrap_or_default();
                ui.weak(format!(
                    "♦ {when}{} ({})",
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
    let now = doc.session.clock().now_local();
    let today = now.date();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(text::t().templates);
        ui.weak(text::t().templates_hint);
        ui.add_space(8.0);
        let cards = query::templates(ws, now);
        if cards.is_empty() {
            ui.label(text::t().no_templates);
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
                    now,
                    out,
                    Some((
                        text::t().from_template,
                        Intent::Open(Dialog::FromTemplate(form)),
                    )),
                );
            }
        });
    });
}
