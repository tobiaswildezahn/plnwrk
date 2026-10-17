//! The schedule of a project (Gantt): one row per task, a time axis in days, bars from start to
//! deadline, milestones as diamonds, arrows for dependencies, the critical path in red.
//! Dragging a bar moves the task; what waits for it moves along (`Intent::Reschedule`). The
//! numbers come from `plnwrk_schedule` (Hamburg working days).

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{CornerRadius, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, pos2, vec2};
use jiff::civil::Date;
use plnwrk_core::{Project, Task, TaskId, TaskStatus};
use plnwrk_engine::query::TaskIndex;
use plnwrk_schedule::{Analysis, Calendar, Item, Plan, Warning};

use crate::design::Icon;
use crate::design::tokens::radius;
use crate::doc::OpenDoc;
use crate::state::{ScheduleColumn, ScheduleRequest, State};
use crate::views::detail::{may_follow, may_precede};
use crate::{Intent, dates, text};

/// Width of a day at each zoom level, coarse to fine: quarter, month, week, day (names in
/// `text::t().zoom_levels`).
const ZOOM: [f32; 4] = [3.0, 8.0, 22.0, 48.0];
/// The level a schedule opens with (week).
pub const LEVEL_START: usize = 2;
/// Room before the first and after the last date: the schedule scrolls this far, no further.
const PAD: i32 = 28;
/// Height of a row.
const ROW: f32 = 32.0;
/// Standard width of the column with the titles (the person can drag it between the limits).
const TITLES: f32 = 220.0;
const TITLES_MIN: f32 = 120.0;
const TITLES_MAX: f32 = 480.0;
/// Height of the time axis on top.
pub(crate) const HEADER: f32 = 40.0;
/// At most this many days are drawn (see [`window`] for which).
const MOST_DAYS: i32 = 800;

/// The command for "`to` waits for `from`".
fn link_intent(from: TaskId, to: TaskId) -> Intent {
    Intent::run(
        "dependency.add",
        serde_json::json!({"from": from.to_string(), "to": to.to_string()}),
    )
}

/// Which part of a bar is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grip {
    Move,
    Start,
    End,
}

/// Width of a day at zoom level `level`.
fn day_width_of(level: usize) -> f32 {
    ZOOM.get(level).copied().unwrap_or(22.0)
}

/// Days from `first` to `date` (negative before).
pub(crate) fn days_from(first: Date, date: Date) -> i32 {
    first.until(date).map_or(0, |span| span.get_days())
}

/// The day `days` days after `date` (the calendar's first or last day beyond its ends).
pub(crate) fn plus(date: Date, days: i32) -> Date {
    jiff::Span::new()
        .try_days(days)
        .ok()
        .and_then(|span| date.checked_add(span).ok())
        .unwrap_or(if days >= 0 { Date::MAX } else { Date::MIN })
}

/// The plan of a project and what follows from it, worked out once per change of the data.
pub(crate) struct Worked {
    /// The open file and the length of its history when it was worked out.
    key: (String, usize),
    pub(crate) plan: Plan,
    pub(crate) analysis: Analysis,
}

/// The plan and its analysis for `project`, worked out again only when the data has changed
/// (every change, undo included, adds to the history).
pub(crate) fn worked(ui: &egui::Ui, doc: &OpenDoc, project: &Project) -> Arc<Worked> {
    let id = egui::Id::new(("schedule-plan", project.id));
    let key = (doc.recovery_key().to_owned(), doc.session.log().len());
    if let Some(kept) = ui.data(|d| d.get_temp::<Arc<Worked>>(id))
        && kept.key == key
    {
        return kept;
    }
    let plan = Plan::of(doc.session.workspace(), project.id, Calendar::standard());
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
/// [`MOST_DAYS`]. Otherwise from the earliest open task on, with today in view unless all open
/// work starts more than half a window later; without open work, the last of the work. So
/// old finished work does not push the current work out of view.
fn window(
    all: &[(Date, Date)],
    open: &[(Date, Date)],
    today: Date,
    due: Option<Date>,
) -> Option<(Date, i32)> {
    let earliest = all.iter().map(|(start, _)| *start).min()?;
    let last_work = all.iter().map(|(_, end)| *end).max()?;
    let latest = [last_work, today].into_iter().chain(due).max()?;
    let first = if days_from(earliest.min(today), latest) + 8 <= MOST_DAYS {
        earliest.min(today)
    } else {
        let half = MOST_DAYS / 2;
        match open.iter().map(|(start, _)| *start).min() {
            Some(start) if start > plus(today, half) => start,
            Some(start) => start.min(today).max(plus(today, -half)),
            // The room after the end counts too, so the end stays in view.
            None => plus(last_work, 9 + PAD - MOST_DAYS),
        }
    };
    let first = plus(first, -PAD);
    Some((
        first,
        (days_from(first, latest) + PAD + 1).clamp(1, MOST_DAYS),
    ))
}

/// The tasks of the project in the order of the list: parents before their subtasks.
pub(crate) fn rows<'a>(index: &TaskIndex<'a>, project: &Project) -> Vec<(&'a Task, usize)> {
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
    state: &mut State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    // Where the schedule begins on screen (its toolbar), for the zoom gesture.
    let top = ui.cursor().top();
    // Only to show weekends and holidays; plnwrk counts every day (Calendar::standard).
    let office = Calendar::hamburg();
    let worked = worked(ui, doc, project);
    let (plan, analysis) = (&worked.plan, &worked.analysis);
    let today = doc.session.clock().today();
    let now = doc.session.clock().now_local();
    let index = TaskIndex::new(ws);
    // A task chosen elsewhere is shown: its folded parents unfold.
    let unfold_id = egui::Id::new(("schedule-unfold", project.id));
    if ui.data(|d| d.get_temp::<Option<TaskId>>(unfold_id)) != Some(state.selected) {
        let mut parent = state
            .selected
            .and_then(|id| ws.task(id))
            .and_then(|t| t.parent);
        while let Some(id) = parent {
            state.folded.remove(&id);
            parent = ws.task(id).and_then(|t| t.parent);
        }
    }
    ui.data_mut(|d| d.insert_temp(unfold_id, state.selected));
    let mut rows = rows(&index, project);
    // Folded tasks keep their subtasks out of the schedule.
    let mut folded_at: Option<usize> = None;
    rows.retain(|(task, depth)| {
        if let Some(at) = folded_at {
            if *depth > at {
                return false;
            }
            folded_at = None;
        }
        if state.folded.contains(&task.id) {
            folded_at = Some(*depth);
        }
        true
    });
    // The table left of the chart: titles, then the columns the person did not switch off. It
    // takes at most half the width: the titles are narrowed first down to their least, then
    // the columns go from the right, so the chart keeps its room.
    let room = ui.clip_rect().right().min(ui.max_rect().right()) - ui.cursor().left();
    let title_width = state
        .title_width
        .unwrap_or(TITLES)
        .clamp(TITLES_MIN, TITLES_MAX)
        .min((room / 2.0).max(TITLES_MIN));
    let mut columns: Vec<ScheduleColumn> = ScheduleColumn::ALL
        .into_iter()
        .filter(|c| !state.hidden_columns.contains(c))
        .collect();
    let width_of =
        |columns: &[ScheduleColumn]| title_width + columns.iter().map(|c| c.width()).sum::<f32>();
    while width_of(&columns) > room / 2.0 && columns.pop().is_some() {}
    let table = width_of(&columns);
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

    // Above everything, so a project without dates can be exported too.
    // Zoom: named levels, or Ctrl/⌘ and the mouse wheel (a pinch) anywhere in the schedule;
    // kept per project in the state, so the ribbon shows the level too.
    let mut level = state
        .zoom_levels
        .get(&project.id)
        .copied()
        .unwrap_or(LEVEL_START)
        .min(ZOOM.len() - 1);
    // Today or the whole project, asked for here or in the ribbon.
    let mut jump = None;
    match state.schedule_request {
        Some((asked, ScheduleRequest::Level(n))) if asked == project.id => {
            level = n.min(ZOOM.len() - 1);
        }
        Some((asked, request)) if asked == project.id => jump = Some(request),
        _ => {}
    }
    // Wraps in a narrow window rather than reach past it (and stretch everything below).
    ui.horizontal_wrapped(|ui| {
        use crate::design::widgets::{Kind, button_with, segmented};
        if button_with(ui, Kind::Secondary, Some(Icon::Today), text::t().today).clicked() {
            jump = Some(ScheduleRequest::Today);
        }
        let whole = text::t().whole_project;
        if button_with(ui, Kind::Secondary, Some(Icon::WholeProject), whole).clicked() {
            jump = Some(ScheduleRequest::Whole);
        }
        ui.add_space(8.0);
        // Finest first, as people read it: Tag, Woche, Monat, Quartal.
        let names = text::t().zoom_levels;
        let options: Vec<(usize, &str)> = (0..ZOOM.len())
            .rev()
            .filter_map(|n| names.get(n).map(|name| (n, *name)))
            .collect();
        segmented(ui, &mut level, &options);
        ui.add_space(8.0);
        if crate::design::widgets::button(ui, Kind::Secondary, text::t().export_schedule).clicked()
        {
            out.push(Intent::ExportSchedule(project.id));
        }
    });
    ui.add_space(4.0);

    let all: Vec<(Date, Date)> = spans.values().copied().collect();
    let open: Vec<(Date, Date)> = plan
        .items()
        .filter(|item| !item.done)
        .filter_map(Item::span)
        .collect();
    let Some((first, days)) = window(&all, &open, today, project.due) else {
        // Nothing to draw: say why, and offer the one step that helps – dates for the first
        // task without any, or a first task.
        let action = match unscheduled.first() {
            Some(_) => text::t().give_dates,
            None => text::t().first_task,
        };
        if crate::design::widgets::empty_state(
            ui,
            Icon::Calendar,
            text::t().schedule_empty,
            Some(action),
        ) {
            match unscheduled.first() {
                Some(task) => out.push(Intent::Select(Some(task.id))),
                None => out.extend([Intent::Tab(crate::state::Tab::List), Intent::FocusNewTask]),
            }
        }
        unscheduled_list(ui, &unscheduled, out);
        return;
    };
    // Zoom by pinching or Ctrl/⌘ and the wheel anywhere in the schedule: over the bars, the
    // titles and below the last row. One notch (or a pinch) comes in over several frames: the
    // factor is gathered and one level taken per 25 %.
    let anchor_id = egui::Id::new(("schedule-zoom-anchor", project.id));
    let region = Rect::from_min_max(pos2(ui.max_rect().left(), top), ui.max_rect().max);
    // Through the layer, so a dialog or menu on top still takes the gesture.
    if let Some(pointer) = ui.ctx().pointer_hover_pos()
        && ui
            .ctx()
            .rect_contains_pointer(ui.layer_id(), region.intersect(ui.clip_rect()))
    {
        let gesture_id = egui::Id::new(("schedule-zoom-gesture", project.id));
        let zoom = ui.input(|i| i.zoom_delta());
        if (zoom - 1.0).abs() > f32::EPSILON {
            let mut gathered = ui.data(|d| d.get_temp::<f32>(gesture_id)).unwrap_or(1.0) * zoom;
            let before = level;
            if gathered >= 1.25 {
                level = (level + 1).min(ZOOM.len() - 1);
                gathered = 1.0;
            } else if gathered <= 0.8 {
                level = level.saturating_sub(1);
                gathered = 1.0;
            }
            if level != before {
                // The scroll area starts at the left edge of this region.
                let anchor = pointer.x - region.left();
                ui.data_mut(|d| d.insert_temp(anchor_id, anchor));
            }
            ui.data_mut(|d| d.insert_temp(gesture_id, gathered));
            ui.ctx().request_repaint();
        }
    }
    // The whole project: the finest level at which its dates fit into the visible chart.
    // What can be seen, not what the layout offers (it can reach past the window).
    let right = ui.clip_rect().right().min(ui.max_rect().right());
    let visible = (right - ui.cursor().left() - table - 12.0).max(day_width_of(0));
    let span = (days - 2 * PAD).max(1) as f32;
    if jump == Some(ScheduleRequest::Whole) {
        level = (0..ZOOM.len())
            .rev()
            .find(|n| span * day_width_of(*n) <= visible)
            .unwrap_or(0);
    }
    let day_width = day_width_of(level);

    // Opens a week before today (once per project), so the current work is in view.
    let opened = ui.id().with(("schedule-opened", project.id));
    let first_time = !ui.data(|d| d.get_temp::<bool>(opened).unwrap_or(false));
    ui.data_mut(|d| d.insert_temp(opened, true));
    let scroll_salt = ("schedule-scroll", project.id);
    let scroll_id = ui.make_persistent_id(egui::IdSalt::new(scroll_salt));
    let max_height = (ui.available_height() - 70.0).max(ROW * 4.0);
    // The mouse wheel moves the rows while there are more than fit, otherwise through time
    // (with only one direction left, egui turns the wheel sideways). Shift and the wheel, or a
    // trackpad swiped sideways, always move through time.
    // The milestones of the project, once more in a row of their own under the time axis that
    // stays in view while the rows scroll (plan §7.3).
    let mut pinned: Vec<(&Task, Date)> = ws
        .tasks_of(project.id)
        .filter(|t| t.milestone)
        .filter_map(|t| t.due.map(|due| (t, due)))
        .collect();
    pinned.sort_by_key(|(t, due)| (*due, t.title.clone()));
    let pin = if pinned.is_empty() { 0.0 } else { ROW };
    let rows_fit = HEADER + pin + rows.len() as f32 * ROW <= max_height;
    // Scroll bars always shown (the way sideways must be visible), beside the rows, never over
    // them; dragging the empty area moves the view (with the mouse too).
    let mut scroll = egui::ScrollArea::new([true, !rows_fit])
        .id_salt(scroll_salt)
        .auto_shrink([false, true])
        .max_height(max_height)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .scroll_source(egui::scroll_area::ScrollSource {
            drag: egui::scroll_area::DragScroll::Always,
            ..Default::default()
        });
    // The zoom level last drawn: a change keeps the same dates in view (the scroll offset is
    // in pixels, so it is scaled along).
    let drawn_id = egui::Id::new(("schedule-drawn-zoom", project.id));
    let drawn = ui.data(|d| d.get_temp::<usize>(drawn_id));
    ui.data_mut(|d| d.insert_temp(drawn_id, level));
    // Where the zoom was asked for, from the left edge of the visible chart: the day there
    // stays in place. The buttons keep the middle of the visible chart.
    let anchor = ui
        .data_mut(|d| d.remove_temp::<f32>(anchor_id))
        .unwrap_or(table + visible / 2.0);
    // Today a third into the visible chart; the whole project from its first date.
    let today_offset = |width: f32| {
        let third = (visible / width / 3.0) as i32;
        (days_from(first, today) - third).max(0) as f32 * width
    };
    if jump == Some(ScheduleRequest::Whole) {
        scroll = scroll.horizontal_scroll_offset((PAD - 2).max(0) as f32 * day_width);
    } else if (jump == Some(ScheduleRequest::Today) || first_time)
        && (0..days).contains(&days_from(first, today))
    {
        scroll = scroll.horizontal_scroll_offset(today_offset(day_width));
    } else if let Some(drawn) = drawn
        && drawn != level
        && let (Some(before), Some(state)) = (
            ZOOM.get(drawn),
            egui::scroll_area::State::load(ui.ctx(), scroll_id),
        )
    {
        let from_labels = state.offset.x + anchor - table;
        scroll = scroll
            .horizontal_scroll_offset((table + from_labels * day_width / before - anchor).max(0.0));
    }
    // A task chosen elsewhere (list, details, palette) since the last frame: its bar comes into
    // view.
    let chosen_id = egui::Id::new(("schedule-chosen", project.id));
    let newly_chosen = ui.data(|d| d.get_temp::<Option<TaskId>>(chosen_id)) != Some(state.selected);
    ui.data_mut(|d| d.insert_temp(chosen_id, state.selected));
    // Changes from the table, taken after drawing.
    let mut fold: Option<TaskId> = None;
    let mut widen = 0.0_f32;
    let only_direction = ui.style().always_scroll_the_only_direction;
    ui.style_mut().always_scroll_the_only_direction = true;
    scroll.show(ui, |ui| {
        let width = table + days as f32 * day_width;
        // At least as high as the room: days off and today reach down to the bottom.
        let room = if rows_fit { max_height - 16.0 } else { 0.0 };
        let height = (HEADER + pin + rows.len() as f32 * ROW).max(room);
        let (area, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let painter = ui.painter_at(area);
        let visuals = ui.visuals().clone();
        let palette = crate::design::palette(ui);
        // Dates and markers in the smallest size there is (12.5), never smaller.
        let small = egui::TextStyle::Small.resolve(ui.style());
        let x_of = |date: Date| area.left() + table + days_from(first, date) as f32 * day_width;
        let row_y = |i: usize| area.top() + HEADER + pin + i as f32 * ROW;
        // The milestone row: under the time axis, or at the top of the view once the rows
        // scrolled below it.
        let pin_y = (area.top() + HEADER).max(ui.clip_rect().top());
        // The column with the titles stays at the left edge while scrolling sideways; what
        // lies under it is hidden and cannot be dragged.
        let left = ui.clip_rect().left().max(area.left());
        let chart = Rect::from_min_max(pos2(left + table - 6.0, area.top()), area.max);
        let bar_of = |milestone: bool, (start, end): (Date, Date), y: f32| {
            if milestone {
                let center = pos2(x_of(end) + day_width / 2.0, y + ROW / 2.0);
                Rect::from_center_size(center, vec2(16.0, 16.0))
            } else {
                // At least 4 px, so a short bar can still be seen and grabbed zoomed out.
                let left = x_of(start) + 1.0;
                Rect::from_min_max(
                    pos2(left, y + 8.0),
                    pos2((x_of(end) + day_width - 1.0).max(left + 4.0), y + ROW - 8.0),
                )
            }
        };

        // Days off set apart from working days.
        let days_off = palette.shade;
        // Days off, weeks, today, the project's deadline.
        for n in 0..days {
            let day = plus(first, n);
            let x = x_of(day);
            if day_width >= 5.0 && !office.is_working(day) {
                painter.rect_filled(
                    Rect::from_min_max(
                        pos2(x, area.top() + HEADER),
                        pos2(x + day_width, area.bottom()),
                    ),
                    0.0,
                    days_off,
                );
            }
            // Fine: weeks with their number; coarse: months.
            let week_starts = day.weekday() == jiff::civil::Weekday::Monday;
            if day_width < 10.0 && day.day() == 1 {
                painter.line_segment(
                    [pos2(x, area.top()), pos2(x, area.bottom())],
                    Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
                );
                painter.text(
                    pos2(x + 3.0, area.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    text::month(day),
                    small.clone(),
                    visuals.weak_text_color(),
                );
            }
            // "KW 42 · 12.10.2026" needs about 110 px: a week of at least 16 px days.
            if day_width >= 16.0 && week_starts {
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
                    small.clone(),
                    visuals.weak_text_color(),
                );
            }
            // Two digits in the small size need about 16 px.
            if day_width >= 18.0 {
                painter.text(
                    pos2(x + day_width / 2.0, area.top() + HEADER - 3.0),
                    egui::Align2::CENTER_BOTTOM,
                    day.day().to_string(),
                    small.clone(),
                    visuals.weak_text_color(),
                );
            }
        }
        let marker = |date: Date, color, label: &str| {
            let x = x_of(date) + day_width / 2.0;
            painter.line_segment(
                [pos2(x, area.top() + HEADER), pos2(x, area.bottom())],
                Stroke::new(1.5, color),
            );
            painter.text(
                pos2(x + 2.0, area.top() + HEADER),
                egui::Align2::LEFT_TOP,
                label,
                small.clone(),
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
        // A dependency being drawn from the handle at a bar's end: from which task, from where,
        // and whether it was let go in this frame.
        let mut linking: Option<(TaskId, Pos2, bool)> = None;
        // The label of a bar being dragged: where the pointer is, and what it says.
        let mut preview: Option<(Pos2, String)> = None;
        for (i, (task, _)) in rows.iter().enumerate() {
            let y = row_y(i);
            // The chosen row is framed, not filled: bars keep their contrast (3:1) on it.
            if state.selected == Some(task.id) {
                let line = Stroke::new(1.0, palette.accent);
                for edge in [y + 0.5, y + ROW - 0.5] {
                    painter.hline(area.left()..=area.right(), edge, line);
                }
            }

            let Some((start, end)) = spans.get(&task.id).copied() else {
                continue;
            };
            let critical = analysis.critical.contains(&task.id);
            let bar = bar_of(task.milestone, (start, end), y);
            boxes.insert(task.id, bar);
            // Centred in the chart, right of the table that covers the left part of the view:
            // the table's width is added before the bar, so the middle of both is the middle of
            // the view and the bar lands right of the table.
            // Only when hardly anything of it is in view: a click on a bar (also one longer
            // than the view) must not move the chart.
            // What the milestone row covers does not count as seen.
            let view = ui.clip_rect();
            let open_top = if pinned.is_empty() {
                view.top()
            } else {
                view.top().max(pin_y + pin)
            };
            let view = Rect::from_min_max(pos2(view.left(), open_top), view.max);
            let seen = chart.intersect(view).intersect(bar);
            let hardly =
                seen.width() <= 0.0 || seen.height() <= 0.0 || seen.width() < bar.width().min(24.0);
            if newly_chosen && state.selected == Some(task.id) && hardly {
                let with_table = Rect::from_min_max(pos2(bar.left() - table, bar.top()), bar.max);
                ui.scroll_to_rect(with_table, Some(egui::Align::Center));
            }
            let visible = bar.intersect(chart);
            if visible.width() <= 0.0 || visible.height() <= 0.0 {
                continue;
            }
            // Three handles: the middle moves the task, the ends change start or deadline
            // (not for milestones, and not on bars too short to hit an end apart).
            let ends = !task.milestone && visible.width() >= 18.0;
            let response = ui.interact(
                visible,
                ui.id().with(("bar", task.id)),
                Sense::click_and_drag(),
            );
            let edge = |at: f32, which: &str| {
                let rect =
                    Rect::from_min_max(pos2(at - 4.0, bar.top()), pos2(at + 4.0, bar.bottom()))
                        .intersect(chart);
                ui.interact(rect, ui.id().with((which, task.id)), Sense::DRAG)
            };
            let start_edge = ends.then(|| edge(bar.left(), "bar-start"));
            let end_edge = ends.then(|| edge(bar.right(), "bar-end"));
            let grip = [
                (Grip::Start, start_edge.as_ref()),
                (Grip::End, end_edge.as_ref()),
                (Grip::Move, Some(&response)),
            ];
            // The handle held: which, whether still dragged, whether just let go.
            let held: Option<(Grip, bool, bool)> = grip.iter().find_map(|(g, r)| {
                r.filter(|r| r.dragged() || r.drag_stopped())
                    .map(|r| (*g, r.dragged(), r.drag_stopped()))
            });
            // The pointer shows what a drag would do.
            if start_edge.as_ref().is_some_and(Response::hovered)
                || end_edge.as_ref().is_some_and(Response::hovered)
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            } else if response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            // While dragged: shown where it would land, whole days. Kept between frames:
            // when the button is released, egui no longer knows where the drag began.
            let kept = response.id.with("shift");
            let mut shift = 0;
            if let Some((_, dragged, stopped)) = held {
                if dragged
                    && let Some(days) = ui.input(|i| {
                        let origin = i.pointer.press_origin()?;
                        let now = i.pointer.interact_pos()?;
                        Some(((now.x - origin.x) / day_width).round() as i32)
                    })
                {
                    ui.data_mut(|d| d.insert_temp(kept, days));
                }
                shift = ui.data(|d| d.get_temp::<i32>(kept)).unwrap_or(0);
                if stopped {
                    ui.data_mut(|d| d.remove::<i32>(kept));
                }
            }
            // Where it would land: the dates, and how many tasks that wait for it move along.
            let landing: Option<(Option<Date>, Option<Date>)> = match held {
                _ if shift == 0 => None,
                Some((Grip::Move, ..)) => plan
                    .shifted(task.id, plus(start, shift))
                    .map(|m| (m.start, m.due)),
                // An end changes only its own date and never crosses the other one. Pulled
                // right onto the other date, the end of a missing date stays missing: nothing
                // is made up. (Dragging the deadline end of a task without a deadline further
                // right gives it one: that is what the person did.)
                Some((Grip::Start, ..)) => {
                    let new = plus(start, shift);
                    let clamped = task.due.map_or(new, |due| new.min(due));
                    (task.start.is_some() || Some(clamped) != task.due)
                        .then_some((Some(clamped), task.due))
                }
                Some((Grip::End, ..)) => {
                    let new = plus(end, shift);
                    let clamped = task.start.map_or(new, |start| new.max(start));
                    (task.due.is_some() || Some(clamped) != task.start)
                        .then_some((task.start, Some(clamped)))
                }
                None => None,
            };
            let landed = landing.and_then(|(from, to)| {
                Item {
                    id: task.id,
                    start: from,
                    due: to,
                    milestone: task.milestone,
                    done: false,
                }
                .span()
            });
            let along = landing
                .and_then(|(from, to)| {
                    plnwrk_schedule::Reschedule::of(ws, task.id, from, to, Calendar::standard())
                })
                .map_or(0, |r| r.along());
            let shown = landed.map_or(bar, |span| bar_of(task.milestone, span, y));
            // The one rule of the app (list, details, overview): with a deadline and its time,
            // not in finished, archived projects or templates.
            let overdue = plnwrk_engine::query::task_overdue(Some(project), task, now);
            let show_critical = critical && !state.hide_critical;
            let (fill, darker) = if task.status == TaskStatus::Done {
                (palette.control, palette.control)
            } else if show_critical {
                (palette.danger_fill, palette.danger_hover)
            } else {
                (palette.accent_fill, palette.accent_hover)
            };
            if task.milestone {
                let c = shown.center();
                let diamond = vec![
                    pos2(c.x, c.y - 8.0),
                    pos2(c.x + 8.0, c.y),
                    pos2(c.x, c.y + 8.0),
                    pos2(c.x - 8.0, c.y),
                ];
                let ring = if overdue {
                    Stroke::new(2.0, palette.danger)
                } else {
                    Stroke::NONE
                };
                painter.add(Shape::convex_polygon(diamond, fill, ring));
                // Milestones carry their name beside them.
                painter.text(
                    pos2(shown.right() + 22.0, c.y),
                    egui::Align2::LEFT_CENTER,
                    &task.title,
                    small.clone(),
                    visuals.weak_text_color(),
                );
            } else {
                let corner = CornerRadius::same(radius::MEDIUM);
                painter.rect_filled(shown, corner, fill);
                // Progress: the done part darker.
                if task.status != TaskStatus::Done && task.progress > 0 {
                    let mut done = shown;
                    done.set_width(shown.width() * f32::from(task.progress.min(100)) / 100.0);
                    painter.rect_filled(done, corner, darker);
                }
                if overdue {
                    painter.rect_stroke(
                        shown,
                        corner,
                        Stroke::new(2.0, palette.danger),
                        egui::StrokeKind::Outside,
                    );
                }
            }
            // The warning sits where the link handle appears; while that shows, it gives way.
            let handle_near = response.hovered()
                || ui.rect_contains_pointer(crate::design::icons::square(
                    pos2(shown.right() + 14.0, shown.center().y),
                    18.0,
                ));
            if overdue && !handle_near {
                Icon::Warning.paint(
                    &painter,
                    crate::design::icons::square(
                        pos2(shown.right() + 12.0, shown.center().y),
                        14.0,
                    ),
                    palette.danger,
                );
            }
            // While dragged: the dates at the pointer and what moves along (painted after all
            // rows, over everything).
            if let Some((from, to)) = landed
                && let Some(at) = ui.input(|i| i.pointer.interact_pos())
            {
                let mut lines = vec![text::span_with_weekdays(from, to)];
                if along > 0 {
                    lines.push(text::moves_along(along));
                }
                preview = Some((at, lines.join("\n")));
            }
            let (from, to) = landed.unwrap_or((start, end));
            let when = if from == to {
                dates::format(to)
            } else {
                format!("{} – {}", dates::format(from), dates::format(to))
            };
            // Spoken: dates, and what the colours say (never colour alone).
            let mut name = format!("{}: {when}", task.title);
            if task.status != TaskStatus::Done && task.progress > 0 {
                name.push_str(&format!(", {} %", task.progress.min(100)));
            }
            if overdue {
                name.push_str(&format!(", {}", text::t().overdue_short));
            }
            if show_critical {
                name.push_str(&format!(", {}", text::t().critical_short));
            }
            if along > 0 {
                name.push_str(&format!(", {}", text::moves_along(along)));
            }
            let enabled = ui.is_enabled();
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name)
            });
            for (grip, handle) in [(text::t().start, &start_edge), (text::t().due, &end_edge)] {
                if let Some(handle) = handle {
                    let spoken = format!("{grip} · {}", task.title);
                    handle.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Other, enabled, &spoken)
                    });
                }
            }
            let float = analysis.float.get(&task.id).copied();
            let response = response.on_hover_text(match float {
                Some(days) if days <= 0 => format!("{name}\n{}", text::t().critical),
                Some(days) => format!("{name}\n{}\n{}", text::float(days), text::t().float_hint),
                None => name.clone(),
            });
            if response.clicked() {
                out.push(Intent::Select(Some(task.id)));
            }
            // Right click: what it waits for, and what waits for it.
            response.context_menu(|ui| {
                let precede = may_precede(ws, task.id);
                let follow = may_follow(ws, task.id);
                let others: Vec<&Task> = rows
                    .iter()
                    .map(|(t, _)| *t)
                    .filter(|t| t.id != task.id)
                    .collect();
                ui.menu_button(text::t().add_predecessor_menu, |ui| {
                    for other in others.iter().filter(|o| precede(o.id)) {
                        if ui.button(&other.title).clicked() {
                            out.push(link_intent(other.id, task.id));
                            ui.close();
                        }
                    }
                });
                ui.menu_button(text::t().add_successor_menu, |ui| {
                    for other in others.iter().filter(|o| follow(o.id)) {
                        if ui.button(&other.title).clicked() {
                            out.push(link_intent(task.id, other.id));
                            ui.close();
                        }
                    }
                });
            });
            // The handle at the end: dragged onto another bar, that one waits for this task.
            let spot = pos2(shown.right() + 14.0, shown.center().y);
            let handle = ui.interact(
                crate::design::icons::square(spot, 18.0).intersect(chart),
                ui.id().with(("link", task.id)),
                Sense::DRAG,
            );
            let handle_name = format!("{} · {}", text::t().link_handle, task.title);
            handle.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Other, enabled, &handle_name)
            });
            if response.hovered() || handle.hovered() || handle.dragged() {
                painter.circle(spot, 5.0, palette.surface, Stroke::new(2.0, palette.accent));
            }
            if handle.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if handle.dragged() || handle.drag_stopped() {
                linking = Some((task.id, spot, handle.drag_stopped()));
            }
            if let Some((_, _, true)) = held
                && let Some((from, to)) = landing
            {
                out.push(Intent::Reschedule {
                    task: task.id,
                    start: from,
                    due: to,
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
            // Muted; red on the critical path; the chosen task's links stand out.
            let touches_chosen =
                state.selected == Some(link.from) || state.selected == Some(link.to);
            let (color, width) = if touches_chosen {
                (palette.accent, 2.0)
            } else if both_critical && !state.hide_critical {
                (visuals.error_fg_color, 1.2)
            } else {
                (palette.control, 1.0)
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
            painter.extend(arrow(start, end, Stroke::new(width, color)));
        }
        // The drag preview over everything, kept inside the view (flipped left or up near an
        // edge).
        if let Some((at, label)) = preview {
            let top = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("schedule-preview"),
            ));
            let galley = top.layout_no_wrap(label, small.clone(), visuals.text_color());
            let size = galley.size() + vec2(12.0, 8.0);
            let view = ui.clip_rect();
            let x = if at.x + 14.0 + size.x > view.right() {
                at.x - 14.0 - size.x
            } else {
                at.x + 14.0
            };
            let y = if at.y + 14.0 + size.y > view.bottom() {
                at.y - 14.0 - size.y
            } else {
                at.y + 14.0
            };
            let place = Rect::from_min_size(pos2(x, y), size);
            top.rect(
                place,
                CornerRadius::same(radius::SMALL),
                palette.surface,
                Stroke::new(1.0, palette.line),
                egui::StrokeKind::Inside,
            );
            top.galley(place.min + vec2(6.0, 4.0), galley, visuals.text_color());
        }
        // A link being drawn: a line to the pointer, the bar under it marked; let go on a bar
        // that may wait for it, and it does.
        if let Some((from, spot, dropped)) = linking
            && let Some(at) = ui.input(|i| i.pointer.interact_pos())
        {
            let follows = may_follow(ws, from);
            let target = boxes
                .iter()
                .find(|(id, rect)| **id != from && rect.expand(4.0).contains(at))
                .map(|(id, rect)| (*id, *rect))
                .filter(|(id, _)| follows(*id));
            painter.line_segment([spot, at], Stroke::new(2.0, palette.accent));
            if let Some((_, rect)) = target {
                painter.rect_stroke(
                    rect.expand(3.0),
                    CornerRadius::same(radius::MEDIUM),
                    Stroke::new(2.0, palette.accent),
                    egui::StrokeKind::Outside,
                );
            }
            if dropped && let Some((to, _)) = target {
                out.push(link_intent(from, to));
            }
        }

        // The milestone row over the rows scrolled under it; it takes the pointer there.
        if !pinned.is_empty() {
            let band =
                Rect::from_min_max(pos2(chart.left(), pin_y), pos2(area.right(), pin_y + ROW));
            painter.rect_filled(band, 0.0, visuals.panel_fill);
            painter.hline(
                band.x_range(),
                band.bottom() - 0.5,
                Stroke::new(1.0, palette.line),
            );
            // It takes the pointer: nothing hidden under it can be dragged through it (a drag
            // on it moves the view, as on the empty chart).
            let over = ui.interact(band, ui.id().with("milestone-row"), Sense::click_and_drag());
            if over.dragged() {
                ui.scroll_with_delta(over.drag_delta());
            }
            // Names only where they do not run into the one before (the spoken name and the
            // tooltip always have it).
            let mut name_end = f32::NEG_INFINITY;
            for (task, due) in &pinned {
                let c = pos2(x_of(*due) + day_width / 2.0, pin_y + ROW / 2.0);
                let done = task.status == TaskStatus::Done;
                let overdue = plnwrk_engine::query::task_overdue(Some(project), task, now);
                let fill = if done {
                    palette.control
                } else {
                    palette.accent_fill
                };
                let ring = if overdue {
                    Stroke::new(2.0, palette.danger)
                } else if state.selected == Some(task.id) {
                    Stroke::new(2.0, palette.text)
                } else {
                    Stroke::NONE
                };
                let diamond = vec![
                    pos2(c.x, c.y - 7.0),
                    pos2(c.x + 7.0, c.y),
                    pos2(c.x, c.y + 7.0),
                    pos2(c.x - 7.0, c.y),
                ];
                painter.add(Shape::convex_polygon(diamond, fill, ring));
                if c.x - 8.0 > name_end {
                    let name = painter.text(
                        pos2(c.x + 11.0, c.y),
                        egui::Align2::LEFT_CENTER,
                        &task.title,
                        small.clone(),
                        visuals.weak_text_color(),
                    );
                    name_end = name.right();
                } else {
                    name_end = name_end.max(c.x + 8.0);
                }
                let hit = Rect::from_center_size(c, vec2(18.0, 18.0)).intersect(band);
                if hit.width() <= 0.0 {
                    continue;
                }
                let response = ui.interact(hit, ui.id().with(("pinned", task.id)), Sense::click());
                let mut spoken = format!(
                    "♦ {}: {}",
                    task.title,
                    dates::format_deadline(*due, task.due_time)
                );
                if overdue {
                    spoken.push_str(&format!(", {}", text::t().overdue_short));
                }
                let enabled = ui.is_enabled();
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &spoken)
                });
                if response.on_hover_text(&spoken).clicked() {
                    out.push(Intent::Select(Some(task.id)));
                }
            }
        }

        // The table last, over the bars: titles and columns. A click chooses the task.
        let column = Rect::from_min_max(
            pos2(left, area.top()),
            pos2(left + table - 6.0, area.bottom()),
        );
        painter.rect_filled(column, 0.0, visuals.panel_fill);
        // Headings in the row of the time axis.
        let mut x = left + 4.0 + title_width;
        painter.text(
            pos2(left + 4.0, area.top() + HEADER - 3.0),
            egui::Align2::LEFT_BOTTOM,
            text::t().ribbon.task,
            small.clone(),
            visuals.weak_text_color(),
        );
        for column in &columns {
            painter.text(
                pos2(x, area.top() + HEADER - 3.0),
                egui::Align2::LEFT_BOTTOM,
                column.name(),
                small.clone(),
                visuals.weak_text_color(),
            );
            x += column.width();
        }
        for (i, (task, depth)) in rows.iter().enumerate() {
            let y = row_y(i);
            let row = Rect::from_min_max(pos2(left, y), pos2(left + table - 6.0, y + ROW));
            if state.selected == Some(task.id) {
                painter.rect_filled(row, 0.0, palette.accent_soft);
            }
            let cell = Rect::from_min_max(pos2(left, y), pos2(left + title_width, y + ROW));
            let response = ui.interact(row, ui.id().with(("title", task.id)), Sense::click());
            let enabled = ui.is_enabled();
            // The columns: planned start, deadline, who does it.
            let values: Vec<String> = columns
                .iter()
                .map(|column| {
                    let value = match column {
                        ScheduleColumn::Start => task.start.map(dates::format),
                        ScheduleColumn::Due => task.due.map(dates::format),
                        ScheduleColumn::Assignee => task
                            .assignee
                            .and_then(|id| ws.person(id))
                            .map(|p| p.name.clone()),
                    };
                    value.unwrap_or_else(|| "–".to_owned())
                })
                .collect();
            // Spoken with the row: "Beginn 12.10.2026, Frist 14.10.2026, Zuständig Anna Berg".
            let spoken = columns
                .iter()
                .zip(&values)
                .map(|(c, v)| format!("{} {v}", c.name()))
                .collect::<Vec<_>>()
                .join(", ");
            response.widget_info(|| {
                let mut info =
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &task.title);
                info.current_text_value = Some(spoken.clone());
                info
            });
            if response.clicked() {
                out.push(Intent::Select(Some(task.id)));
            }
            // Fold and unfold the subtasks of a task that has some.
            let indent = left + 4.0 + *depth as f32 * 14.0;
            if !index.children(task.id).is_empty() {
                let folded = state.folded.contains(&task.id);
                let at = crate::design::icons::square(pos2(indent + 8.0, y + ROW / 2.0), 24.0);
                let name = if folded {
                    text::t().unfold_subtasks
                } else {
                    text::t().fold_subtasks
                };
                let toggle = ui.interact(at, ui.id().with(("fold", task.id)), Sense::click());
                toggle.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name)
                });
                let icon = if folded {
                    Icon::ChevronRight
                } else {
                    Icon::ChevronDown
                };
                icon.paint(
                    &painter,
                    crate::design::icons::square(at.center(), 14.0),
                    visuals.weak_text_color(),
                );
                if toggle.on_hover_text(name).clicked() {
                    fold = Some(task.id);
                }
            }
            painter.with_clip_rect(cell.shrink2(vec2(2.0, 0.0))).text(
                pos2(indent + 20.0, y + ROW / 2.0),
                egui::Align2::LEFT_CENTER,
                &task.title,
                egui::TextStyle::Body.resolve(ui.style()),
                visuals.text_color(),
            );
            let mut x = left + 4.0 + title_width;
            for (column, value) in columns.iter().zip(&values) {
                let cell = Rect::from_min_max(pos2(x, y), pos2(x + column.width() - 6.0, y + ROW));
                painter.with_clip_rect(cell).text(
                    pos2(x, y + ROW / 2.0),
                    egui::Align2::LEFT_CENTER,
                    value,
                    small.clone(),
                    visuals.text_color(),
                );
                x += column.width();
            }
        }
        // The milestone row's name, over the titles scrolled under it.
        if !pinned.is_empty() {
            let cell = Rect::from_min_max(pos2(left, pin_y), pos2(left + table - 6.0, pin_y + ROW));
            painter.rect_filled(cell, 0.0, visuals.panel_fill);
            painter.hline(
                cell.x_range(),
                cell.bottom() - 0.5,
                Stroke::new(1.0, palette.line),
            );
            painter.text(
                pos2(left + 4.0, pin_y + ROW / 2.0),
                egui::Align2::LEFT_CENTER,
                format!("♦ {}", text::t().milestones),
                small.clone(),
                visuals.weak_text_color(),
            );
            ui.interact(cell, ui.id().with("milestone-row-title"), Sense::click());
        }
        // The edge of the title column can be dragged to make it wider or narrower.
        let edge = Rect::from_min_max(
            pos2(left + title_width - 3.0, area.top()),
            pos2(
                left + title_width + 3.0,
                area.top() + HEADER + pin + rows.len() as f32 * ROW,
            ),
        );
        let drag = ui.interact(edge, ui.id().with("title-edge"), Sense::DRAG);
        drag.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, text::t().title_width)
        });
        if drag.hovered() || drag.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            painter.vline(
                edge.center().x,
                edge.y_range(),
                Stroke::new(2.0, palette.accent),
            );
        }
        if drag.dragged() {
            widen = drag.drag_delta().x;
        }
    });
    ui.style_mut().always_scroll_the_only_direction = only_direction;
    if let Some(task) = fold
        && !state.folded.remove(&task)
    {
        state.folded.insert(task);
    }
    if widen != 0.0 {
        state.title_width = Some((title_width + widen).clamp(TITLES_MIN, TITLES_MAX));
    }

    state.zoom_levels.insert(project.id, level);

    // The legend: only what is red now, and how to work with the bars.
    let mut legend = Vec::new();
    if !state.hide_critical && !analysis.critical.is_empty() {
        legend.push(text::t().critical_hint);
    }
    let any_overdue = rows
        .iter()
        .any(|(task, _)| plnwrk_engine::query::task_overdue(Some(project), task, now));
    if any_overdue {
        legend.push(text::t().overdue_hint);
    }
    if project
        .due
        .is_some_and(|due| (0..days).contains(&days_from(first, due)))
    {
        legend.push(text::t().due_line_hint);
    }
    legend.push(text::t().drag_hint);
    for line in legend {
        ui.add(egui::Label::new(RichText::new(line).weak()).wrap());
    }

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
    fn everything_is_shown_when_it_fits_with_four_weeks_around() {
        let all = [(d("2026-10-12"), d("2026-10-16"))];
        let (first, days) = window(&all, &all, d("2026-10-14"), None).unwrap();
        assert_eq!(first, d("2026-09-14"), "four weeks before the first date");
        assert_eq!(
            plus(first, days - 1),
            d("2026-11-13"),
            "four weeks after the last"
        );
        // Today before a project that starts later: in view too.
        let (first, _) = window(&all, &all, d("2026-09-01"), None).unwrap();
        assert_eq!(first, plus(d("2026-09-01"), -PAD));
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
        assert_eq!(first, plus(d("2026-10-12"), -PAD));
        assert!((0..days).contains(&days_from(first, today)));
        // An open task from long ago: today still in view.
        let open = [(d("2020-01-06"), d("2020-01-10"))];
        let (first, days) = window(&all, &open, today, None).unwrap();
        assert!((0..days).contains(&days_from(first, today)));
        assert_eq!(first, plus(today, -MOST_DAYS / 2 - PAD));
        assert!(
            days_from(first, d("2026-10-16")) < days,
            "up to the last end"
        );
    }

    #[test]
    fn finished_work_long_ago_is_shown_up_to_its_end() {
        let all = [(d("2020-01-06"), d("2020-03-10"))];
        let (first, days) = window(&all, &[], d("2026-10-14"), None).unwrap();
        assert!((0..days).contains(&days_from(first, d("2020-03-10"))));
        assert!(
            days_from(first, d("2020-03-10")) >= days - PAD - 12,
            "the end, not the start"
        );
    }

    #[test]
    fn open_work_far_ahead_is_shown_from_its_start() {
        let all = [(d("2029-01-08"), d("2029-02-09"))];
        let (first, days) = window(&all, &all, d("2026-10-14"), None).unwrap();
        assert_eq!(first, plus(d("2029-01-08"), -PAD));
        assert!(days > 30);
        // Within half a window: from today, both in view.
        let all = [
            (d("2027-06-07"), d("2027-06-11")),
            (d("2025-01-06"), d("2025-01-10")),
        ];
        let open = [(d("2027-06-07"), d("2027-06-11"))];
        let (first, days) = window(&all, &open, d("2026-10-14"), None).unwrap();
        assert_eq!(first, plus(d("2026-10-14"), -PAD));
        assert!((0..days).contains(&days_from(first, d("2027-06-11"))));
    }

    #[test]
    fn far_dates_do_not_break_the_window() {
        let all = [(Date::MIN, Date::MAX)];
        assert!(window(&all, &all, d("2026-10-14"), Some(Date::MAX)).is_some());
        assert!(window(&all, &[], d("2026-10-14"), None).is_some());
    }

    #[test]
    fn nothing_scheduled_no_window() {
        assert_eq!(window(&[], &[], d("2026-10-14"), None), None);
    }
}
