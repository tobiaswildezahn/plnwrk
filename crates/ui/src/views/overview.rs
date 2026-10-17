//! The overview: one card per project or case, then everything overdue. And the templates.

use egui::RichText;
use plnwrk_core::{Classification, EntityKey};
use plnwrk_engine::query::{self, ProjectCard};

use crate::doc::OpenDoc;
use crate::state::{Dialog, TemplateForm, View};
use crate::{Intent, dates, design, text};

/// Wider than before (Phase 9f): room for the light and the facts on one line each.
const CARD_WIDTH: f32 = 320.0;

pub fn show(ui: &mut egui::Ui, doc: &OpenDoc, out: &mut Vec<Intent>) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(text::t().overview);
        ui.add_space(8.0);
        // First what needs doing now: everything overdue, then what is due today – each
        // ticked off right here (a milestone asks first, open subtasks too).
        design::widgets::subheading(ui, text::t().today_and_overdue);
        let overdue = query::overdue(ws, now);
        let today = query::due_today(ws, now);
        if overdue.is_empty() && today.is_empty() {
            ui.weak(text::t().nothing_due);
        }
        for (item, late) in overdue
            .iter()
            .map(|i| (i, true))
            .chain(today.iter().map(|i| (i, false)))
        {
            due_row(ui, ws, item, late, now, out);
        }
        ui.add_space(16.0);
        design::widgets::subheading(ui, text::t().projects);
        let cards = query::overview(ws, now);
        // Nothing yet: a symbol, a sentence, the one step that helps (Phase 9f).
        if cards.is_empty()
            && design::widgets::empty_state(
                ui,
                crate::design::Icon::Case,
                text::t().nothing_yet,
                Some(text::t().first_case),
            )
        {
            out.push(Intent::Open(Dialog::NewProject(
                crate::state::ProjectForm {
                    kind: text::t().kinds[1].to_owned(),
                    ..crate::state::ProjectForm::default()
                },
            )));
        }
        // In rows of as many as fit (egui does not wrap framed cards by itself).
        for row in cards.chunks(per_row(ui)) {
            ui.horizontal_top(|ui| {
                for card in row {
                    let light = ws.project(card.id).map(|project| {
                        let worked = super::schedule::worked(ui, doc, project);
                        super::milestones::project_light(ws, project, &worked.analysis, now)
                    });
                    project_card(ui, card, light, now, out, None);
                }
            });
            ui.add_space(8.0);
        }
    });
}

/// One line of "Heute und überfällig": a tick (tasks and milestones), how the deadline stands,
/// and the title, which opens it.
fn due_row(
    ui: &mut egui::Ui,
    ws: &plnwrk_core::Workspace,
    item: &query::OverdueItem,
    late: bool,
    now: jiff::civil::DateTime,
    out: &mut Vec<Intent>,
) {
    ui.horizontal(|ui| {
        if let EntityKey::Task(task) = item.entity {
            let mut done = false;
            let name = text::mark_done(&item.title);
            let tick = ui
                .add(egui::Checkbox::without_text(&mut done))
                .on_hover_text(&name);
            tick.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, false, &name)
            });
            if tick.clicked() {
                let params = serde_json::json!({"task": task.to_string(), "status": "done"});
                let open = super::board::open_below(ws, task);
                out.push(if open > 0 {
                    Intent::Open(Dialog::CloseSubtasks { task, params, open })
                } else {
                    Intent::run("task.move", params)
                });
            }
        }
        let tone = if late {
            design::widgets::Tone::Danger
        } else {
            design::widgets::Tone::Warning
        };
        design::widgets::chip(
            ui,
            tone,
            &text::deadline_state(item.due, item.due_time, now).0,
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

/// How many cards fit side by side (at least one).
fn per_row(ui: &egui::Ui) -> usize {
    // The card's frame adds 2 × 12 inside and its line; then the gap.
    let card = CARD_WIDTH + 24.0 + 2.0 + ui.spacing().item_spacing.x;
    ((ui.available_width() / card).floor() as usize).max(1)
}

/// One project card. `action`: an extra button below (for templates).
fn project_card(
    ui: &mut egui::Ui,
    card: &ProjectCard,
    light: Option<(super::milestones::Light, Option<String>)>,
    now: jiff::civil::DateTime,
    out: &mut Vec<Intent>,
    action: Option<(&str, Intent)>,
) {
    design::widgets::card(ui, |ui| {
        ui.set_width(CARD_WIDTH);
        ui.vertical(|ui| {
            let mut line = Vec::new();
            if let Some(kind) = &card.kind {
                line.push(text::kind_shown(kind));
            }
            if let Some(reference) = &card.reference {
                line.push(reference.clone());
            }
            line.push(text::project_status(card.status).to_owned());
            ui.horizontal(|ui| {
                // The light as on the milestones, with its reason on hover and read out.
                if let Some((light, why)) = &light {
                    let chip = design::widgets::chip(ui, light.tone(), light.name());
                    if let Some(why) = why {
                        chip.on_hover_text(why);
                    }
                }
                ui.weak(line.join(" · "));
            });
            // The reason, where the facts below do not say it already (overdue work does).
            if let Some((_, Some(why))) = &light
                && card.overdue_tasks == 0
            {
                ui.label(RichText::new(why).color(design::palette(ui).warning));
            }
            if ui
                .link(RichText::new(&card.title).text_style(design::subheading()))
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
            design::widgets::progress(
                ui,
                f32::from(card.progress.percent) / 100.0,
                &text::progress(
                    card.progress.percent,
                    card.progress.done,
                    card.progress.tasks,
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
            // How a template comes about is the step to take (no button can do it here).
            design::widgets::empty_state(
                ui,
                crate::design::Icon::Board,
                text::t().no_templates,
                None,
            );
        }
        for row in cards.chunks(per_row(ui)) {
            ui.horizontal_top(|ui| {
                for card in row {
                    let form = TemplateForm {
                        template: Some(card.id),
                        start: dates::format(today),
                        ..TemplateForm::default()
                    };
                    project_card(
                        ui,
                        card,
                        None,
                        now,
                        out,
                        Some((
                            text::t().from_template,
                            Intent::Open(Dialog::FromTemplate(form)),
                        )),
                    );
                }
            });
            ui.add_space(8.0);
        }
    });
}
