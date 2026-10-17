//! The milestones of a project (Phase 9d, plan §7.3): a timeline with every milestone as a
//! diamond, today and the project's deadline; below it a card for each, with what is left, a
//! light derived from the plan and what it waits for. Every change of a milestone – drag on the
//! timeline, "Erreicht", "Termin ändern", "Löschen" – is asked first (`PlnwrkApp::run`).

use egui::{Align2, Rect, Sense, Shape, Stroke, pos2, vec2};
use jiff::civil::{Date, DateTime};
use plnwrk_core::{Project, Task, TaskStatus, Workspace};
use plnwrk_engine::query::{self, MilestoneState};
use plnwrk_schedule::Analysis;
use serde_json::json;

use crate::design::palette;
use crate::design::widgets::{self, Kind, Tone};
use crate::doc::OpenDoc;
use crate::state::{Dialog, Message, State};
use crate::views::date;
use crate::views::schedule::{self, Worked, days_from, plus};
use crate::{Clear, Intent, Then, dates, text};

/// A milestone with this many days of slack or fewer before it is at risk.
const AT_RISK_SLACK: i32 = 2;

/// Where a milestone stands, derived from its status, its date and the plan – never set by
/// hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    /// Done.
    Reached,
    /// Its date passed without it being reached.
    Overdue,
    /// Something it waits for is overdue, or the slack up to it is small.
    AtRisk,
    OnPlan,
}

impl Light {
    pub fn name(self) -> &'static str {
        let t = text::t();
        match self {
            Self::Reached => t.light_reached,
            Self::Overdue => t.light_overdue,
            Self::AtRisk => t.light_at_risk,
            Self::OnPlan => t.light_on_plan,
        }
    }

    fn tone(self) -> Tone {
        match self {
            Self::Reached => Tone::Success,
            Self::Overdue => Tone::Danger,
            Self::AtRisk => Tone::Warning,
            Self::OnPlan => Tone::Info,
        }
    }
}

/// The tasks a milestone waits for.
fn waits_for<'a>(ws: &'a Workspace, milestone: &'a Task) -> impl Iterator<Item = &'a Task> + 'a {
    ws.dependencies()
        .filter(move |d| d.to == milestone.id)
        .filter_map(|d| ws.task(d.from))
}

/// The light of a milestone, and why it is at risk. "Überfällig" as everywhere in the app
/// (`milestone_state_in`, `task_overdue`); the slack is the milestone's own from the plan (how
/// much later what it waits for could end), not the slack to the end of the project.
pub fn light(
    ws: &Workspace,
    project: &Project,
    milestone: &Task,
    analysis: &Analysis,
    now: DateTime,
) -> (Light, Option<String>) {
    match query::milestone_state_in(project, milestone, now) {
        MilestoneState::Reached => return (Light::Reached, None),
        MilestoneState::Missed => return (Light::Overdue, None),
        MilestoneState::Open => {}
    }
    // Where deadlines do not count (templates, finished projects), nothing is at risk either.
    if !query::deadlines_count(project) {
        return (Light::OnPlan, None);
    }
    if waits_for(ws, milestone).any(|t| query::task_overdue(Some(project), t, now)) {
        return (Light::AtRisk, Some(text::t().predecessor_late.to_owned()));
    }
    // The milestone's slack from the plan: only what it still waits for counts there.
    match analysis.float.get(&milestone.id) {
        Some(&days) if days <= AT_RISK_SLACK => (Light::AtRisk, Some(text::little_slack(days))),
        _ => (Light::OnPlan, None),
    }
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    project: &Project,
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let worked = schedule::worked(ui, doc, project);
    let mut all: Vec<&Task> = ws.tasks_of(project.id).filter(|t| t.milestone).collect();
    all.sort_by(|a, b| {
        query::deadline_order(a.due, a.due_time)
            .cmp(&query::deadline_order(b.due, b.due_time))
            .then(a.title.cmp(&b.title))
    });
    let lights: Vec<(Light, Option<String>)> = all
        .iter()
        .map(|m| light(ws, project, m, &worked.analysis, now))
        .collect();
    if all.is_empty() {
        ui.weak(text::t().no_milestones);
    } else {
        timeline(ui, project, &all, &lights, &worked, state, out, now.date());
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
            for (milestone, light) in all.iter().zip(&lights) {
                card(ui, doc, project, milestone, light, state, out, now);
            }
        });
    }
    ui.add_space(12.0);
    new_milestone(ui, project, state, out, now.date());
}

/// The timeline: every milestone with a date as a diamond, today, the project's deadline.
/// A click chooses a milestone, a drag gives it another date (asked first).
#[expect(clippy::too_many_arguments, reason = "one view, drawn in one place")]
fn timeline(
    ui: &mut egui::Ui,
    project: &Project,
    all: &[&Task],
    lights: &[(Light, Option<String>)],
    worked: &Worked,
    state: &State,
    out: &mut Vec<Intent>,
    today: Date,
) {
    let dated: Vec<(usize, &Task, Date)> = all
        .iter()
        .enumerate()
        .filter_map(|(n, t)| t.due.map(|due| (n, *t, due)))
        .collect();
    let marks = dated
        .iter()
        .map(|(_, _, d)| *d)
        .chain([today])
        .chain(project.due);
    let (Some(lo), Some(hi)) = (marks.clone().min(), marks.max()) else {
        return;
    };
    if dated.is_empty() {
        return;
    }
    let span = days_from(lo, hi).max(1);
    let pad = (span / 12).max(3);
    let first = plus(lo, -pad);
    let days = span + 2 * pad;
    let p = palette(ui);
    let small = egui::TextStyle::Small.resolve(ui.style());
    let width = ui.available_width().max(200.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 100.0), Sense::hover());
    let inner = rect.shrink2(vec2(12.0, 0.0));
    let day_px = inner.width() / days as f32;
    let x_of = |date: Date| inner.left() + (days_from(first, date) as f32 + 0.5) * day_px;
    // Room above for two rows of names, below for the dates and the marks' names.
    let axis = rect.top() + 52.0;
    let painter = ui.painter_at(rect);
    painter.hline(inner.x_range(), axis, Stroke::new(1.0, p.line));
    // Today and the project's deadline as marks with their name below.
    let mark = |date: Date, color: egui::Color32, name: &str| {
        let x = x_of(date);
        painter.vline(x, axis - 12.0..=axis + 12.0, Stroke::new(2.0, color));
        painter.text(
            pos2(x, axis + 30.0),
            Align2::CENTER_TOP,
            name,
            small.clone(),
            color,
        );
    };
    mark(today, p.accent, text::t().today);
    if let Some(due) = project.due {
        mark(due, p.danger, text::t().project_due_mark);
    }
    // Names above the diamonds, in two rows where they would overlap; dates below (while
    // dragged: the date it would get).
    let mut ends = [f32::NEG_INFINITY; 2];
    for (n, task, due) in dated {
        let (light, _) = lights.get(n).cloned().unwrap_or((Light::OnPlan, None));
        let response = ui.interact(
            Rect::from_center_size(pos2(x_of(due), axis), vec2(22.0, 22.0)),
            ui.id().with(("milestone-diamond", task.id)),
            Sense::click_and_drag(),
        );
        // While dragged: whole days, kept between frames (when the button is released egui
        // no longer knows where the drag began).
        let kept = response.id.with("shift");
        if response.dragged()
            && let Some(shift) = ui.input(|i| {
                let origin = i.pointer.press_origin()?;
                let now = i.pointer.interact_pos()?;
                Some(((now.x - origin.x) / day_px).round() as i32)
            })
        {
            ui.data_mut(|d| d.insert_temp(kept, shift));
        }
        let shift = if response.dragged() || response.drag_stopped() {
            ui.data(|d| d.get_temp::<i32>(kept)).unwrap_or(0)
        } else {
            0
        };
        if response.drag_stopped() {
            ui.data_mut(|d| d.remove::<i32>(kept));
        }
        // Where it would land: the same rule as in the schedule.
        let landing = (shift != 0)
            .then(|| worked.plan.shifted(task.id, plus(due, shift)))
            .flatten();
        let shown = landing.and_then(|m| m.due).unwrap_or(due);
        let c = pos2(x_of(shown), axis);
        let fill = match light {
            Light::Reached => p.success,
            Light::Overdue => p.danger,
            Light::AtRisk => p.warning_mark,
            Light::OnPlan => p.accent,
        };
        let ring = if state.selected == Some(task.id) || response.hovered() {
            Stroke::new(2.0, p.text)
        } else {
            Stroke::NONE
        };
        let diamond = vec![
            pos2(c.x, c.y - 9.0),
            pos2(c.x + 9.0, c.y),
            pos2(c.x, c.y + 9.0),
            pos2(c.x - 9.0, c.y),
        ];
        painter.add(Shape::convex_polygon(diamond, fill, ring));
        let name = painter.layout_no_wrap(task.title.clone(), small.clone(), p.text);
        // Centred over the diamond, but kept inside the timeline.
        let left = (c.x - name.size().x / 2.0)
            .min(rect.right() - name.size().x)
            .max(rect.left());
        // The first row where it fits; where neither does, the one it overlaps least.
        let row = if left > ends[0] + 8.0 {
            0
        } else if left > ends[1] + 8.0 {
            1
        } else {
            usize::from(ends[1] < ends[0])
        };
        if let Some(end) = ends.get_mut(row) {
            *end = left + name.size().x;
        }
        let y = axis - 16.0 - 16.0 * row as f32;
        painter.galley(pos2(left, y - name.size().y), name, p.text);
        painter.text(
            pos2(c.x, axis + 12.0),
            Align2::CENTER_TOP,
            dates::format_short(shown),
            small.clone(),
            p.muted,
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
        // Spoken: name, date and light (never colour alone).
        let spoken = format!(
            "♦ {}: {}, {}",
            task.title,
            dates::format_deadline(shown, task.due_time),
            light.name()
        );
        let enabled = ui.is_enabled();
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &spoken));
        let response = response.on_hover_text(text::t().timeline_hint);
        if response.clicked() {
            out.push(Intent::Select(Some(task.id)));
        }
        if response.drag_stopped()
            && let Some(landed) = landing
        {
            out.push(Intent::Reschedule {
                task: task.id,
                start: landed.start,
                due: landed.due,
                due_time: None,
            });
        }
    }
}

/// One milestone: light, name, date and what is left, what it waits for, who is in charge, and
/// what can be done with it.
#[expect(clippy::too_many_arguments, reason = "one view, drawn in one place")]
fn card(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    project: &Project,
    milestone: &Task,
    (light, why): &(Light, Option<String>),
    state: &mut State,
    out: &mut Vec<Intent>,
    now: DateTime,
) {
    let ws = doc.session.workspace();
    let p = palette(ui);
    widgets::card(ui, |ui| {
        ui.set_width(260.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                widgets::chip(ui, light.tone(), light.name());
            });
            let title = egui::RichText::new(format!("♦ {}", milestone.title)).strong();
            if ui
                .selectable_label(state.selected == Some(milestone.id), title)
                .clicked()
            {
                out.push(Intent::Select(Some(milestone.id)));
            }
            if let Some(due) = milestone.due {
                ui.label(dates::format_deadline(due, milestone.due_time));
            }
            // What is left: as the app says it everywhere; when reached, the day.
            let left = if milestone.status == TaskStatus::Done {
                let day = query::done_at(doc.session.log(), milestone.id)
                    .map(|at| dates::format(at.to_zoned(jiff::tz::TimeZone::system()).date()));
                Some(text::reached_on(day.as_deref()))
            } else if query::deadlines_count(project) {
                milestone
                    .due
                    .map(|due| text::deadline_state(due, milestone.due_time, now).0)
            } else {
                None
            };
            if let Some(left) = left {
                let color = if *light == Light::Overdue {
                    p.danger
                } else {
                    p.muted
                };
                ui.label(egui::RichText::new(left).color(color));
            }
            if let Some(why) = why {
                ui.label(egui::RichText::new(why).color(p.warning));
            }
            let open: Vec<&Task> = waits_for(ws, milestone)
                .filter(|t| t.status != TaskStatus::Done)
                .collect();
            // Said only of a milestone that waits for something at all.
            if open.is_empty() {
                if waits_for(ws, milestone).next().is_some() {
                    ui.weak(text::open_predecessors(0));
                }
            } else {
                egui::CollapsingHeader::new(text::open_predecessors(open.len()))
                    .id_salt(("milestone-waits", milestone.id))
                    .show(ui, |ui| {
                        for task in &open {
                            if ui.link(&task.title).clicked() {
                                out.push(Intent::Select(Some(task.id)));
                            }
                        }
                    });
            }
            let who = milestone
                .assignee
                .and_then(|id| ws.person(id))
                .map_or(text::t().no_assignee, |person| person.name.as_str());
            ui.weak(format!("{}: {who}", text::t().assignee));
            ui.add_space(4.0);
            let editing = state
                .forms
                .milestone_edit
                .as_ref()
                .is_some_and(|(id, ..)| *id == milestone.id);
            if editing {
                edit_date(ui, project, milestone, state, out, now.date());
            } else {
                actions(ui, milestone, state, out);
            }
        });
    });
}

/// "Erreicht" (or "Wieder öffnen"), "Termin ändern", "Löschen" – each asked first.
fn actions(ui: &mut egui::Ui, milestone: &Task, state: &mut State, out: &mut Vec<Intent>) {
    let t = text::t();
    ui.horizontal_wrapped(|ui| {
        let (label, status) = if milestone.status == TaskStatus::Done {
            (t.reopen_button, TaskStatus::Todo)
        } else {
            (t.reach_button, TaskStatus::Done)
        };
        if widgets::button(ui, Kind::Secondary, label).clicked() {
            out.push(Intent::run(
                "task.set_status",
                json!({"task": milestone.id.to_string(), "status": status.name()}),
            ));
        }
        if widgets::button(ui, Kind::Secondary, t.change_date).clicked() {
            state.forms.milestone_edit = Some((
                milestone.id,
                dates::format_opt(milestone.due),
                dates::format_time_opt(milestone.due_time),
            ));
        }
        ui.add_space(8.0);
        if widgets::button(ui, Kind::Destructive, t.delete).clicked() {
            out.push(Intent::Open(Dialog::DeleteTask(milestone.id)));
        }
    });
}

/// The date of a milestone, changed on its card. "Übernehmen" asks first; the form closes
/// either way, so a "no" leaves the stored date standing.
fn edit_date(
    ui: &mut egui::Ui,
    project: &Project,
    milestone: &Task,
    state: &mut State,
    out: &mut Vec<Intent>,
    today: Date,
) {
    let Some((_, day, time)) = state.forms.milestone_edit.as_mut() else {
        return;
    };
    let input = ui
        .horizontal(|ui| {
            let caption = ui.label(text::t().milestone_date);
            date::deadline_input(
                ui,
                ("edit-milestone", milestone.id),
                day,
                time,
                today,
                project.reference.is_some(),
                Some(caption.id),
            )
        })
        .inner;
    // Enter in the field takes it, as "Übernehmen".
    let entered = input.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let (mut apply, mut cancel) = (entered, false);
    ui.horizontal(|ui| {
        apply |= widgets::button(ui, Kind::Primary, text::t().apply).clicked();
        cancel = widgets::button(ui, Kind::Quiet, text::t().cancel).clicked();
    });
    if cancel {
        state.forms.milestone_edit = None;
    } else if apply {
        let (day, time) = (day.clone(), time.clone());
        match dates::parse_deadline(&day, &time) {
            Ok(Some((due, due_time))) => {
                state.forms.milestone_edit = None;
                out.push(Intent::Reschedule {
                    task: milestone.id,
                    start: milestone.start.filter(|s| *s <= due),
                    due: Some(due),
                    due_time: Some(due_time),
                });
            }
            Ok(None) => state.message = Some(Message::error(text::t().date_needed)),
            Err(message) => state.message = Some(Message::error(message)),
        }
    }
}

/// A new milestone: title and date (creating one asks nothing).
fn new_milestone(
    ui: &mut egui::Ui,
    project: &Project,
    state: &mut State,
    out: &mut Vec<Intent>,
    today: Date,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.forms.milestone_title)
                .hint_text(text::t().title)
                .desired_width(240.0),
        );
        date::deadline_input(
            ui,
            ("new-milestone", project.id),
            &mut state.forms.milestone_date,
            &mut state.forms.milestone_time,
            today,
            project.reference.is_some(),
            None,
        );
        if ui.button(text::t().new_milestone).clicked() {
            match dates::parse_deadline(&state.forms.milestone_date, &state.forms.milestone_time) {
                Ok(Some(due)) => out.push(Intent::Run {
                    command: "task.create",
                    params: json!({
                        "project": project.id.to_string(),
                        "title": state.forms.milestone_title.trim(),
                        "milestone": true,
                        "due": dates::deadline_param(Some(due)),
                    }),
                    then: Then::Nothing,
                    clear: Some(Clear::Milestone),
                }),
                Ok(None) => state.message = Some(Message::error(text::t().date_needed)),
                Err(message) => state.message = Some(Message::error(message)),
            }
        }
    });
}
