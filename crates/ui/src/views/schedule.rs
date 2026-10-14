//! The schedule of a project (Gantt): one row per task, a time axis in days, bars from start to
//! deadline, milestones as diamonds, arrows for dependencies, the critical path in red.
//! Dragging a bar moves the task; what waits for it moves along (`Intent::Reschedule`). The
//! numbers come from `plnwrk_schedule` (Hamburg working days).

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Pos2, Rect, RichText, Sense, Shape, Stroke, pos2, vec2};
use jiff::civil::Date;
use plnwrk_core::{Project, Task, TaskId, TaskStatus};
use plnwrk_engine::query::TaskIndex;
use plnwrk_schedule::{Analysis, Calendar, Item, Plan, Warning};

use crate::doc::OpenDoc;
use crate::state::State;
use crate::{Intent, dates, text};

/// Width of a day.
const DAY: f32 = 22.0;
/// Height of a row.
const ROW: f32 = 24.0;
/// Width of the column with the titles.
const LABELS: f32 = 220.0;
/// Height of the time axis on top.
const HEADER: f32 = 34.0;
/// At most this many days are drawn (see [`window`] for which).
const MOST_DAYS: i32 = 800;

/// Days from `first` to `date` (negative before).
fn days_from(first: Date, date: Date) -> i32 {
    first.until(date).map_or(0, |span| span.get_days())
}

/// The day `days` days after `date`.
fn plus(date: Date, days: i32) -> Date {
    date.checked_add(jiff::Span::new().days(days))
        .unwrap_or(date)
}

/// The plan of a project and what follows from it, worked out once per change of the data.
struct Worked {
    /// The open file and the length of its history when it was worked out.
    key: (String, usize),
    plan: Plan,
    analysis: Analysis,
}

/// The plan and its analysis for `project`, worked out again only when the data has changed
/// (every change, undo included, adds to the history).
fn worked(ui: &egui::Ui, doc: &OpenDoc, project: &Project) -> Arc<Worked> {
    let id = egui::Id::new(("schedule-plan", project.id));
    let key = (doc.recovery_key().to_owned(), doc.session.log().len());
    if let Some(kept) = ui.data(|d| d.get_temp::<Arc<Worked>>(id))
        && kept.key == key
    {
        return kept;
    }
    let plan = Plan::of(doc.session.workspace(), project.id, Calendar::hamburg());
    let analysis = plan.analyse();
    let fresh = Arc::new(Worked {
        key,
        plan,
        analysis,
    });
    ui.data_mut(|d| d.insert_temp(id, Arc::clone(&fresh)));
    fresh
}

/// The days to draw: the first and how many. Everything, with today, if it fits into
/// [`MOST_DAYS`]; otherwise from the earliest open task on (but never so early that today falls
/// out), so old finished work does not push the current work out of view.
fn window(
    all: &[(Date, Date)],
    open: &[(Date, Date)],
    today: Date,
    due: Option<Date>,
) -> Option<(Date, i32)> {
    let earliest = all.iter().map(|(start, _)| *start).min()?.min(today);
    let latest = all
        .iter()
        .map(|(_, end)| *end)
        .chain([today])
        .chain(due)
        .max()?;
    let first = if days_from(earliest, latest) + 8 <= MOST_DAYS {
        earliest
    } else {
        open.iter()
            .map(|(start, _)| *start)
            .min()
            .unwrap_or(today)
            .max(plus(today, -MOST_DAYS / 2))
    };
    let first = plus(first, -3);
    Some((first, (days_from(first, latest) + 6).clamp(1, MOST_DAYS)))
}

/// The tasks of the project in the order of the list: parents before their subtasks.
fn rows<'a>(index: &TaskIndex<'a>, project: &Project) -> Vec<(&'a Task, usize)> {
    let sorted = |tasks: &[&'a Task]| {
        let mut tasks = tasks.to_vec();
        tasks.sort_by(|a, b| a.rank.cmp(&b.rank));
        tasks
    };
    let mut rows = Vec::new();
    let mut stack: Vec<(&Task, usize)> = sorted(index.tasks_of(project.id))
        .into_iter()
        .filter(|t| t.parent.is_none())
        .rev()
        .map(|t| (t, 0))
        .collect();
    while let Some((task, depth)) = stack.pop() {
        rows.push((task, depth));
        for child in sorted(index.children(task.id)).into_iter().rev() {
            stack.push((child, depth.saturating_add(1)));
        }
    }
    rows
}

/// A dependency arrow from `from` to `to`: out to the right, down, in from the left.
fn arrow(from: Pos2, to: Pos2, stroke: Stroke) -> Vec<Shape> {
    let out = from.x + 6.0;
    let points = if to.x - 6.0 >= out {
        vec![from, pos2(out, from.y), pos2(out, to.y), to]
    } else {
        // Backwards (overlap, or start to start): around between the rows.
        let middle = (from.y + to.y) / 2.0;
        vec![
            from,
            pos2(out, from.y),
            pos2(out, middle),
            pos2(to.x - 6.0, middle),
            pos2(to.x - 6.0, to.y),
            to,
        ]
    };
    let head = vec![
        to,
        pos2(to.x - 5.0, to.y - 3.5),
        pos2(to.x - 5.0, to.y + 3.5),
    ];
    vec![
        Shape::line(points, stroke),
        Shape::convex_polygon(head, stroke.color, Stroke::NONE),
    ]
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &OpenDoc,
    project: &Project,
    state: &State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let calendar = Calendar::hamburg();
    let worked = worked(ui, doc, project);
    let (plan, analysis) = (&worked.plan, &worked.analysis);
    let today = doc.session.clock().today();
    let index = TaskIndex::new(ws);
    let rows = rows(&index, project);
    let spans: BTreeMap<TaskId, (Date, Date)> = plan
        .items()
        .filter_map(|item| item.span().map(|span| (item.id, span)))
        .collect();
    let title = |id: TaskId| ws.task(id).map_or("", |t| t.title.as_str());
    let unscheduled: Vec<&Task> = rows
        .iter()
        .map(|(t, _)| *t)
        .filter(|t| !spans.contains_key(&t.id))
        .collect();

    let all: Vec<(Date, Date)> = spans.values().copied().collect();
    let open: Vec<(Date, Date)> = plan
        .items()
        .filter(|item| !item.done)
        .filter_map(Item::span)
        .collect();
    let Some((first, days)) = window(&all, &open, today, project.due) else {
        ui.weak(text::t().no_scheduled);
        unscheduled_list(ui, &unscheduled, out);
        return;
    };
    ui.weak(text::t().critical_hint);
    ui.add_space(4.0);

    // Opens a week before today (once per project), so the current work is in view.
    let opened = ui.id().with(("schedule-opened", project.id));
    let first_time = !ui.data(|d| d.get_temp::<bool>(opened).unwrap_or(false));
    ui.data_mut(|d| d.insert_temp(opened, true));
    let mut scroll = egui::ScrollArea::both()
        .auto_shrink([false, true])
        .max_height((ui.available_height() - 70.0).max(ROW * 4.0));
    if first_time && (0..days).contains(&days_from(first, today)) {
        let offset = (days_from(first, today) - 7).max(0) as f32 * DAY;
        scroll = scroll.horizontal_scroll_offset(offset);
    }
    scroll.show(ui, |ui| {
        let width = LABELS + days as f32 * DAY;
        let height = HEADER + rows.len() as f32 * ROW;
        let (area, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let painter = ui.painter_at(area);
        let visuals = ui.visuals().clone();
        let x_of = |date: Date| area.left() + LABELS + days_from(first, date) as f32 * DAY;
        let row_y = |i: usize| area.top() + HEADER + i as f32 * ROW;
        // The column with the titles stays at the left edge while scrolling sideways; what
        // lies under it is hidden and cannot be dragged.
        let left = ui.clip_rect().left().max(area.left());
        let chart = Rect::from_min_max(pos2(left + LABELS - 6.0, area.top()), area.max);
        let bar_of = |milestone: bool, (start, end): (Date, Date), y: f32| {
            if milestone {
                let center = pos2(x_of(end) + DAY / 2.0, y + ROW / 2.0);
                Rect::from_center_size(center, vec2(14.0, 14.0))
            } else {
                Rect::from_min_max(
                    pos2(x_of(start) + 1.0, y + 5.0),
                    pos2(x_of(end) + DAY - 1.0, y + ROW - 5.0),
                )
            }
        };

        // Days off, weeks, today, the project's deadline.
        for n in 0..days {
            let day = plus(first, n);
            let x = x_of(day);
            if !calendar.is_working(day) {
                painter.rect_filled(
                    Rect::from_min_max(pos2(x, area.top() + HEADER), pos2(x + DAY, area.bottom())),
                    0.0,
                    visuals.faint_bg_color,
                );
            }
            if day.weekday() == jiff::civil::Weekday::Monday {
                painter.line_segment(
                    [pos2(x, area.top()), pos2(x, area.bottom())],
                    Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
                );
                painter.text(
                    pos2(x + 3.0, area.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    format!(
                        "{} · {}",
                        text::week(day.iso_week_date().week()),
                        dates::format(day)
                    ),
                    egui::FontId::proportional(11.0),
                    visuals.weak_text_color(),
                );
            }
            painter.text(
                pos2(x + DAY / 2.0, area.top() + HEADER - 4.0),
                egui::Align2::CENTER_BOTTOM,
                day.day().to_string(),
                egui::FontId::proportional(10.0),
                visuals.weak_text_color(),
            );
        }
        let marker = |date: Date, color, label: &str| {
            let x = x_of(date) + DAY / 2.0;
            painter.line_segment(
                [pos2(x, area.top() + HEADER), pos2(x, area.bottom())],
                Stroke::new(1.5, color),
            );
            painter.text(
                pos2(x + 2.0, area.top() + HEADER),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(10.0),
                color,
            );
        };
        if (0..days).contains(&days_from(first, today)) {
            marker(today, visuals.selection.stroke.color, text::t().today);
        }
        if let Some(due) = project.due
            && (0..days).contains(&days_from(first, due))
        {
            marker(due, visuals.error_fg_color, text::t().project_due);
        }

        // Rows: titles and bars.
        let mut boxes: BTreeMap<TaskId, Rect> = BTreeMap::new();
        for (i, (task, _)) in rows.iter().enumerate() {
            let y = row_y(i);
            if state.selected == Some(task.id) {
                painter.rect_filled(
                    Rect::from_min_max(pos2(area.left(), y), pos2(area.right(), y + ROW)),
                    0.0,
                    visuals.selection.bg_fill.gamma_multiply(0.25),
                );
            }

            let Some((start, end)) = spans.get(&task.id).copied() else {
                continue;
            };
            let critical = analysis.critical.contains(&task.id);
            let bar = bar_of(task.milestone, (start, end), y);
            boxes.insert(task.id, bar);
            let visible = bar.intersect(chart);
            if visible.width() <= 0.0 || visible.height() <= 0.0 {
                continue;
            }
            let response = ui.interact(
                visible,
                ui.id().with(("bar", task.id)),
                Sense::click_and_drag(),
            );
            // While dragged: shown where it would land, whole days. Kept between frames:
            // when the button is released, egui no longer knows where the drag began.
            let kept = response.id.with("shift");
            if response.dragged()
                && let Some(days) = ui.input(|i| {
                    let origin = i.pointer.press_origin()?;
                    let now = i.pointer.interact_pos()?;
                    Some(((now.x - origin.x) / DAY).round() as i32)
                })
            {
                ui.data_mut(|d| d.insert_temp(kept, days));
            }
            let shift = if response.dragged() || response.drag_stopped() {
                ui.data(|d| d.get_temp::<i32>(kept)).unwrap_or(0)
            } else {
                0
            };
            if response.drag_stopped() {
                ui.data_mut(|d| d.remove::<i32>(kept));
            }
            // Where it would land: the same working days as when it is dropped.
            let shown = if shift == 0 {
                bar
            } else {
                plan.shifted(task.id, plus(start, shift))
                    .and_then(|m| {
                        Item {
                            id: task.id,
                            start: m.start,
                            due: m.due,
                            milestone: task.milestone,
                            done: false,
                        }
                        .span()
                    })
                    .map_or(bar, |span| bar_of(task.milestone, span, y))
            };
            let fill = if task.status == TaskStatus::Done {
                visuals.widgets.inactive.bg_fill
            } else if critical {
                visuals.error_fg_color
            } else {
                visuals.selection.bg_fill
            };
            if task.milestone {
                let c = shown.center();
                let diamond = vec![
                    pos2(c.x, c.y - 7.0),
                    pos2(c.x + 7.0, c.y),
                    pos2(c.x, c.y + 7.0),
                    pos2(c.x - 7.0, c.y),
                ];
                painter.add(Shape::convex_polygon(diamond, fill, Stroke::NONE));
            } else {
                painter.rect_filled(shown, 3.0, fill);
            }
            let when = if start == end {
                dates::format(end)
            } else {
                format!("{} – {}", dates::format(start), dates::format(end))
            };
            let name = format!("{}: {when}", task.title);
            let enabled = ui.is_enabled();
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name)
            });
            let float = analysis.float.get(&task.id).copied();
            let response = response.on_hover_text(match float {
                Some(days) if days <= 0 => format!("{name}\n{}", text::t().critical),
                Some(days) => format!("{name}\n{}", text::float(days)),
                None => name.clone(),
            });
            if response.clicked() {
                out.push(Intent::Select(Some(task.id)));
            }
            if response.drag_stopped()
                && shift != 0
                && let Some(moved) = plan.shifted(task.id, plus(start, shift))
            {
                out.push(Intent::Reschedule {
                    task: task.id,
                    start: moved.start,
                    due: moved.due,
                    due_time: None,
                });
            }
        }

        // Dependencies.
        for link in plan.links() {
            let (Some(from), Some(to)) = (boxes.get(&link.from), boxes.get(&link.to)) else {
                continue;
            };
            let both_critical =
                analysis.critical.contains(&link.from) && analysis.critical.contains(&link.to);
            let color = if both_critical {
                visuals.error_fg_color
            } else {
                visuals.weak_text_color()
            };
            let (start, end) = match link.kind {
                plnwrk_core::DependencyKind::FinishToStart => {
                    (from.right_center(), to.left_center())
                }
                plnwrk_core::DependencyKind::StartToStart => (from.left_center(), to.left_center()),
                plnwrk_core::DependencyKind::FinishToFinish => {
                    (from.right_center(), to.right_center())
                }
            };
            painter.extend(arrow(start, end, Stroke::new(1.2, color)));
        }

        // Titles last, over the bars; a click chooses the task.
        let column = Rect::from_min_max(
            pos2(left, area.top()),
            pos2(left + LABELS - 6.0, area.bottom()),
        );
        painter.rect_filled(column, 0.0, visuals.panel_fill);
        for (i, (task, depth)) in rows.iter().enumerate() {
            let y = row_y(i);
            let cell = Rect::from_min_max(pos2(left, y), pos2(left + LABELS - 6.0, y + ROW));
            if state.selected == Some(task.id) {
                painter.rect_filled(cell, 0.0, visuals.selection.bg_fill.gamma_multiply(0.25));
            }
            let response = ui.interact(cell, ui.id().with(("title", task.id)), Sense::click());
            let enabled = ui.is_enabled();
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &task.title)
            });
            if response.clicked() {
                out.push(Intent::Select(Some(task.id)));
            }
            painter.with_clip_rect(cell).text(
                pos2(left + 4.0 + *depth as f32 * 14.0, y + ROW / 2.0),
                egui::Align2::LEFT_CENTER,
                &task.title,
                egui::FontId::proportional(13.0),
                visuals.text_color(),
            );
        }
    });

    // What cannot hold.
    for warning in &analysis.warnings {
        let (task, line) = match *warning {
            Warning::Violated { from, to } => (to, text::waits_too_short(title(from), title(to))),
            Warning::Milestone { task, earliest } => (
                task,
                text::milestone_unreachable(title(task), &dates::format(earliest)),
            ),
            Warning::AfterProjectDue { task } => (task, text::after_project_due(title(task))),
        };
        if ui
            .link(RichText::new(format!("⚠ {line}")).color(ui.visuals().warn_fg_color))
            .clicked()
        {
            out.push(Intent::Select(Some(task)));
        }
    }
    unscheduled_list(ui, &unscheduled, out);
}

/// The tasks without any date, to give them some.
fn unscheduled_list(ui: &mut egui::Ui, tasks: &[&Task], out: &mut Vec<Intent>) {
    if tasks.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.weak(text::t().unscheduled);
        for task in tasks {
            if ui.link(&task.title).clicked() {
                out.push(Intent::Select(Some(task.id)));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn everything_is_shown_when_it_fits() {
        let all = [(d("2026-10-12"), d("2026-10-16"))];
        let (first, days) = window(&all, &all, d("2026-10-14"), None).unwrap();
        assert_eq!(first, d("2026-10-09"));
        assert_eq!(days, days_from(first, d("2026-10-16")) + 6);
        // Today before a project that starts later: in view too.
        let (first, _) = window(&all, &all, d("2026-09-01"), None).unwrap();
        assert_eq!(first, d("2026-08-29"));
    }

    #[test]
    fn old_finished_work_does_not_push_today_out() {
        let all = [
            (d("2020-01-06"), d("2020-01-10")),
            (d("2026-10-12"), d("2026-10-16")),
        ];
        let open = [(d("2026-10-12"), d("2026-10-16"))];
        let today = d("2026-10-14");
        let (first, days) = window(&all, &open, today, None).unwrap();
        assert_eq!(first, d("2026-10-09"));
        assert!((0..days).contains(&days_from(first, today)));
        // An open task from long ago: today still in view.
        let open = [(d("2020-01-06"), d("2020-01-10"))];
        let (first, days) = window(&all, &open, today, None).unwrap();
        assert!((0..days).contains(&days_from(first, today)));
        assert_eq!(first, plus(today, -MOST_DAYS / 2 - 3));
        assert!(
            days_from(first, d("2026-10-16")) < days,
            "up to the last end"
        );
    }

    #[test]
    fn nothing_scheduled_no_window() {
        assert_eq!(window(&[], &[], d("2026-10-14"), None), None);
    }
}
