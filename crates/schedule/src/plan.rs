//! The schedule of one project: its tasks with their dates, the dependencies between them, and
//! what follows from them – moving successors along, the critical path, warnings.
//!
//! Dates are whole days; a task lasts from its start to its deadline, both included, counted
//! in working days. A task with only a deadline (or only a start) lasts one day. A milestone is
//! a point in time on its deadline and never moves by itself; neither does a task that is done
//! (decision 4.12). Tasks without any date are not scheduled; a chain of dependencies runs
//! through them as if they took no time.
//!
//! A date on a day off counts as the working day it stands for: an end on a Sunday is the
//! Friday before, a start on a Saturday the Monday after.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use jiff::civil::Date;
use plnwrk_core::{DependencyKind, ProjectId, TaskId, TaskStatus, Workspace};

use crate::Calendar;

/// A task as the schedule sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: TaskId,
    pub start: Option<Date>,
    pub due: Option<Date>,
    pub milestone: bool,
    /// Done: it stays where it is, is never critical and is not warned about.
    pub done: bool,
}

impl Item {
    /// First and last day, if scheduled.
    pub fn span(&self) -> Option<(Date, Date)> {
        match (self.start, self.due) {
            _ if self.milestone => self.due.map(|due| (due, due)),
            (Some(start), Some(due)) => Some((start, due.max(start))),
            (Some(day), None) | (None, Some(day)) => Some((day, day)),
            (None, None) => None,
        }
    }

    /// Whether it stays where it is when what it waits for moves.
    fn fixed(&self) -> bool {
        self.milestone || self.done
    }
}

/// One dependency: `to` waits for `from`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub from: TaskId,
    pub to: TaskId,
    pub kind: DependencyKind,
    /// Extra working days in between (negative: overlap).
    pub lag: i32,
}

/// New dates for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub task: TaskId,
    pub start: Option<Date>,
    pub due: Option<Date>,
}

/// Something in the plan that cannot hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Warning {
    /// `to` starts (or ends) earlier than its dependency on `from` allows.
    Violated { from: TaskId, to: TaskId },
    /// A milestone comes before what it waits for: possible at the earliest on `earliest`.
    Milestone { task: TaskId, earliest: Date },
    /// A task ends after the deadline of its project.
    AfterProjectDue { task: TaskId },
}

/// The critical path and the warnings of a plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Analysis {
    /// Working days each scheduled task may slip without moving the end of the plan (negative:
    /// the plan cannot hold as it is). For a milestone: how much later what it waits for could
    /// end. Done tasks and milestones that wait for nothing have none.
    pub float: BTreeMap<TaskId, i32>,
    /// Tasks on the critical path: no float.
    pub critical: BTreeSet<TaskId>,
    /// What cannot hold.
    pub warnings: Vec<Warning>,
}

/// The schedule of a project.
#[derive(Debug, Clone)]
pub struct Plan {
    calendar: Calendar,
    items: BTreeMap<TaskId, Item>,
    links: Vec<Link>,
    /// Tasks in an order in which every task comes after what it waits for.
    order: Vec<TaskId>,
    project_due: Option<Date>,
}

impl Plan {
    /// The plan of `project` in `workspace`.
    pub fn of(workspace: &Workspace, project: ProjectId, calendar: Calendar) -> Self {
        let items: Vec<Item> = workspace
            .tasks()
            .filter(|t| t.project == project)
            .map(|t| Item {
                id: t.id,
                start: t.start,
                due: t.due,
                milestone: t.milestone,
                done: t.status == TaskStatus::Done,
            })
            .collect();
        let links = workspace
            .dependencies()
            .map(|d| Link {
                from: d.from,
                to: d.to,
                kind: d.kind,
                lag: d.lag_days,
            })
            .collect();
        let due = workspace.project(project).and_then(|p| p.due);
        Self::new(calendar, items, links, due)
    }

    /// A plan of these tasks and links (links to tasks not among them are left out).
    pub fn new(
        calendar: Calendar,
        items: Vec<Item>,
        links: Vec<Link>,
        project_due: Option<Date>,
    ) -> Self {
        let items: BTreeMap<TaskId, Item> = items.into_iter().map(|i| (i.id, i)).collect();
        let links: Vec<Link> = links
            .into_iter()
            .filter(|l| l.from != l.to && items.contains_key(&l.from) && items.contains_key(&l.to))
            .collect();
        let order = topological(&items, &links);
        Self {
            calendar,
            items,
            links,
            order,
            project_due,
        }
    }

    /// The tasks of the plan.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.values()
    }

    /// The dependencies of the plan.
    pub fn links(&self) -> &[Link] {
        &self.links
    }

    /// The calendar it counts with.
    pub fn calendar(&self) -> Calendar {
        self.calendar
    }

    /// The working days `item` lies on, if it has dates: from the first working day at or
    /// after its start to the last at or before its end. A task lying wholly on days off counts
    /// as the working day before its end – or, with only a start, the one after its start. A
    /// milestone keeps its date.
    fn placed(&self, item: &Item) -> Option<(Date, Date)> {
        let (start, end) = item.span()?;
        if item.milestone {
            return Some((start, end));
        }
        let cal = &self.calendar;
        let (first, last) = (cal.next_working(start), cal.previous_working(end));
        Some(if first <= last {
            (first, last)
        } else if item.due.is_none() {
            (first, first)
        } else {
            (last, last)
        })
    }

    /// Length of `item` on `span` in working days: none for a milestone and for a task without
    /// dates (the chain runs through it), otherwise at least one.
    fn duration(&self, item: &Item, (start, end): (Date, Date)) -> i32 {
        if item.milestone || item.span().is_none() {
            0
        } else {
            self.calendar.count(start, end).max(1)
        }
    }

    /// The first working day of `item` lasting `length` working days and ending on `end`. A
    /// milestone starts where it ends; a task without dates (length 0) the working day after.
    fn start_of(&self, item: &Item, end: Date, length: i32) -> Date {
        if item.milestone {
            end
        } else {
            self.calendar.after_end(end, 1 - length)
        }
    }

    /// The last working day of `item` lasting `length` working days and starting on `start`
    /// (the working day before, for a task without dates).
    fn end_of(&self, item: &Item, start: Date, length: i32) -> Date {
        if item.milestone {
            start
        } else {
            self.calendar.from_start(start, length - 1)
        }
    }

    /// The span `item` would have starting on `start` (moved to a working day), keeping its
    /// length in working days.
    fn span_from(&self, item: &Item, span: (Date, Date), start: Date) -> (Date, Date) {
        let start = self.calendar.next_working(start);
        (start, self.end_of(item, start, self.duration(item, span)))
    }

    /// The dates a task would get when it is moved to start on `start` (for dragging): the
    /// same length in working days, starting on the working day at or after `start`.
    pub fn shifted(&self, task: TaskId, start: Date) -> Option<Move> {
        let item = self.items.get(&task)?;
        let span = item.span()?;
        if item.milestone {
            let due = self.calendar.next_working(start);
            return Some(Move {
                task,
                // A start after the new deadline would not be allowed.
                start: item.start.filter(|s| *s <= due),
                due: Some(due),
            });
        }
        let (start, end) = self.span_from(item, span, start);
        Some(as_move(item, start, end))
    }

    /// The earliest start `item` (lasting `length` working days) may have by `link`, given the
    /// span of the task it waits for.
    fn requires(
        &self,
        link: &Link,
        item: &Item,
        spans: &BTreeMap<TaskId, (Date, Date)>,
        length: i32,
    ) -> Option<Date> {
        let cal = &self.calendar;
        let (from_start, from_end) = *spans.get(&link.from)?;
        Some(match link.kind {
            // A milestone may fall on the day its predecessor ends.
            DependencyKind::FinishToStart if item.milestone => cal.after_end(from_end, link.lag),
            DependencyKind::FinishToStart => cal.after_end(from_end, link.lag.saturating_add(1)),
            DependencyKind::StartToStart => cal.from_start(from_start, link.lag),
            DependencyKind::FinishToFinish => {
                self.start_of(item, cal.after_end(from_end, link.lag), length)
            }
        })
    }

    /// The earliest start `item` may have, given the spans of the tasks it waits for.
    fn required(
        &self,
        item: &Item,
        spans: &BTreeMap<TaskId, (Date, Date)>,
        length: i32,
    ) -> Option<Date> {
        self.links
            .iter()
            .filter(|l| l.to == item.id)
            .filter_map(|l| self.requires(l, item, spans, length))
            .max()
    }

    /// Where a task without dates lies: from the earliest start its dependencies allow to the
    /// working day before (it takes no time).
    fn passing(&self, item: &Item, spans: &BTreeMap<TaskId, (Date, Date)>) -> Option<(Date, Date)> {
        let start = self.required(item, spans, 0)?;
        Some((start, self.end_of(item, start, 0)))
    }

    /// Whether `item` on `span` starts before `required` allows. A milestone is compared by
    /// its date, a task by the working day it really starts on.
    fn too_early(&self, item: &Item, span: (Date, Date), required: Date) -> bool {
        if item.milestone {
            required > span.0
        } else {
            required > self.calendar.next_working(span.0)
        }
    }

    /// The spans of all tasks with `changes` applied, tasks without dates running through as
    /// if they took no time.
    fn spans(&self, changes: &BTreeMap<TaskId, Move>) -> BTreeMap<TaskId, (Date, Date)> {
        let mut spans = BTreeMap::new();
        for id in &self.order {
            let Some(item) = self.items.get(id) else {
                continue;
            };
            let item = changes.get(id).map_or_else(
                || item.clone(),
                |m| Item {
                    start: m.start,
                    due: m.due,
                    ..item.clone()
                },
            );
            if let Some(span) = self.placed(&item).or_else(|| self.passing(&item, &spans)) {
                spans.insert(*id, span);
            }
        }
        spans
    }

    /// Gives `moved` new dates and moves every task that waits for it (directly or not) as far
    /// later as its dependencies require, keeping its length. Nothing moves earlier, and
    /// milestones and done tasks do not move (see [`Plan::analyse`] for the warning). Returns
    /// every change, the moved task first.
    pub fn push(&self, moved: Move) -> Vec<Move> {
        let mut changes: BTreeMap<TaskId, Move> = BTreeMap::new();
        if !self.items.contains_key(&moved.task) {
            return Vec::new();
        }
        changes.insert(moved.task, moved);
        let after = self.reachable(moved.task);
        let mut spans: BTreeMap<TaskId, (Date, Date)> = BTreeMap::new();
        for id in &self.order {
            let Some(item) = self.items.get(id) else {
                continue;
            };
            let current = changes.get(id).map_or_else(
                || item.clone(),
                |m| Item {
                    start: m.start,
                    due: m.due,
                    ..item.clone()
                },
            );
            let span = match self.placed(&current) {
                Some(span) => {
                    let length = self.duration(&current, span);
                    match self.required(&current, &spans, length) {
                        Some(required)
                            if after.contains(id)
                                && *id != moved.task
                                && !current.fixed()
                                && self.too_early(&current, span, required) =>
                        {
                            let (start, end) = self.span_from(&current, span, required);
                            changes.insert(*id, as_move(&current, start, end));
                            (start, end)
                        }
                        _ => span,
                    }
                }
                None => match self.passing(&current, &spans) {
                    Some(span) => span,
                    None => continue,
                },
            };
            spans.insert(*id, span);
        }
        let mut moves: Vec<Move> = changes
            .remove(&moved.task)
            .into_iter()
            .chain(changes.into_values())
            .collect();
        moves.retain(|m| {
            self.items
                .get(&m.task)
                .is_some_and(|i| i.start != m.start || i.due != m.due)
        });
        moves
    }

    /// Every task that waits for `task`, directly or through others.
    fn reachable(&self, task: TaskId) -> BTreeSet<TaskId> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([task]);
        while let Some(next) = queue.pop_front() {
            for link in self.links.iter().filter(|l| l.from == next) {
                if seen.insert(link.to) {
                    queue.push_back(link.to);
                }
            }
        }
        seen
    }

    /// The critical path and what cannot hold, for the dates as they are.
    pub fn analyse(&self) -> Analysis {
        let cal = &self.calendar;
        let spans = self.spans(&BTreeMap::new());
        let mut analysis = Analysis::default();
        // Warnings: dependencies not kept, milestones that cannot hold (once each, with the
        // latest date), ends after the project's deadline. None about done tasks.
        for id in &self.order {
            let (Some(item), Some(span)) = (self.items.get(id), spans.get(id)) else {
                continue;
            };
            if item.done || item.span().is_none() {
                continue;
            }
            let length = self.duration(item, *span);
            if item.milestone {
                if let Some(earliest) = self.required(item, &spans, length)
                    && self.too_early(item, *span, earliest)
                {
                    analysis.warnings.push(Warning::Milestone {
                        task: item.id,
                        earliest,
                    });
                }
                continue;
            }
            for link in self.links.iter().filter(|l| l.to == *id) {
                if self
                    .requires(link, item, &spans, length)
                    .is_some_and(|required| self.too_early(item, *span, required))
                {
                    analysis.warnings.push(Warning::Violated {
                        from: link.from,
                        to: link.to,
                    });
                }
            }
        }
        if let Some(due) = self.project_due {
            for item in self.items.values().filter(|i| !i.done) {
                if self.placed(item).is_some_and(|(_, end)| end > due) {
                    analysis
                        .warnings
                        .push(Warning::AfterProjectDue { task: item.id });
                }
            }
        }
        // Backward pass: the latest end each task may have without moving the end of the plan
        // (or a milestone). The end of the plan is that of the open work with dates: a task
        // done early keeps its deadline, a task without dates is not to be seen. Done tasks
        // with dates stay where they are and limit nothing before them; a task without dates
        // limits only through what comes after it.
        let ends = |open: bool| {
            self.items
                .values()
                .filter(|i| !(open && i.done))
                .filter_map(|i| self.placed(i).map(|(_, end)| end))
                .max()
        };
        let Some(finish) = ends(true).or_else(|| ends(false)) else {
            return analysis;
        };
        let mut latest_end: BTreeMap<TaskId, Date> = BTreeMap::new();
        for id in self.order.iter().rev() {
            let (Some(item), Some(span)) = (self.items.get(id), spans.get(id)) else {
                continue;
            };
            let dated = item.span().is_some();
            if item.done && dated {
                continue;
            }
            let length = self.duration(item, *span);
            let mut end = match (item.milestone, dated) {
                (true, _) => Some(span.1),
                (false, true) => Some(finish),
                (false, false) => None,
            };
            for link in self.links.iter().filter(|l| l.from == *id) {
                let (Some(next), Some(next_span), Some(next_end)) = (
                    self.items.get(&link.to),
                    spans.get(&link.to),
                    latest_end.get(&link.to),
                ) else {
                    continue;
                };
                let next_length = self.duration(next, *next_span);
                let next_start = cal.previous_working(self.start_of(next, *next_end, next_length));
                let limit = match link.kind {
                    DependencyKind::FinishToStart if next.milestone => {
                        cal.after_end(next_start, link.lag.saturating_neg())
                    }
                    DependencyKind::FinishToStart => {
                        cal.after_end(next_start, link.lag.saturating_add(1).saturating_neg())
                    }
                    DependencyKind::StartToStart => self.end_of(
                        item,
                        cal.after_end(next_start, link.lag.saturating_neg()),
                        length,
                    ),
                    DependencyKind::FinishToFinish => {
                        cal.after_end(*next_end, link.lag.saturating_neg())
                    }
                };
                end = Some(end.map_or(limit, |end| end.min(limit)));
            }
            let Some(end) = end else {
                continue;
            };
            latest_end.insert(*id, end);
            if !dated {
                continue;
            }
            let float = if item.milestone {
                // A milestone does not move: its float is how much later what it waits for
                // could end. Waiting for nothing, it has none to lose.
                self.required(item, &spans, length)
                    .map(|earliest| cal.between(earliest, cal.previous_working(span.1)))
            } else {
                Some(cal.between(span.0, self.start_of(item, end, length)))
            };
            if let Some(float) = float {
                analysis.float.insert(*id, float);
                if float <= 0 {
                    analysis.critical.insert(*id);
                }
            }
        }
        analysis
    }
}

/// A move of `item` to `start..=end`, keeping which dates it has.
fn as_move(item: &Item, start: Date, end: Date) -> Move {
    match (item.start, item.due) {
        (Some(_), Some(_)) => Move {
            task: item.id,
            start: Some(start),
            due: Some(end),
        },
        (Some(_), None) => Move {
            task: item.id,
            start: Some(start),
            due: None,
        },
        _ => Move {
            task: item.id,
            start: item.start,
            due: Some(end),
        },
    }
}

/// The tasks in an order in which every task comes after what it waits for. Tasks in a circle
/// (which the data rules forbid) come last, in id order, so nothing loops.
fn topological(items: &BTreeMap<TaskId, Item>, links: &[Link]) -> Vec<TaskId> {
    let mut waiting: BTreeMap<TaskId, usize> = items.keys().map(|id| (*id, 0)).collect();
    for link in links {
        if let Some(n) = waiting.get_mut(&link.to) {
            *n = n.saturating_add(1);
        }
    }
    let mut ready: VecDeque<TaskId> = waiting
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::with_capacity(items.len());
    let mut placed = BTreeSet::new();
    while let Some(id) = ready.pop_front() {
        if !placed.insert(id) {
            continue;
        }
        order.push(id);
        for link in links.iter().filter(|l| l.from == id) {
            if let Some(n) = waiting.get_mut(&link.to) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    ready.push_back(link.to);
                }
            }
        }
    }
    order.extend(items.keys().filter(|id| !placed.contains(id)));
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn d(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn id(n: u128) -> TaskId {
        ulid::Ulid(n).into()
    }

    fn task(n: u128, start: &str, due: &str) -> Item {
        Item {
            id: id(n),
            start: Some(d(start)),
            due: Some(d(due)),
            milestone: false,
            done: false,
        }
    }

    fn milestone(n: u128, due: &str) -> Item {
        Item {
            id: id(n),
            start: None,
            due: Some(d(due)),
            milestone: true,
            done: false,
        }
    }

    fn open(n: u128) -> Item {
        Item {
            id: id(n),
            start: None,
            due: None,
            milestone: false,
            done: false,
        }
    }

    fn fs(from: u128, to: u128) -> Link {
        Link {
            from: id(from),
            to: id(to),
            kind: DependencyKind::FinishToStart,
            lag: 0,
        }
    }

    fn plan(items: Vec<Item>, links: Vec<Link>) -> Plan {
        Plan::new(Calendar::hamburg(), items, links, None)
    }

    fn moved(n: u128, start: &str, due: &str) -> Move {
        Move {
            task: id(n),
            start: Some(d(start)),
            due: Some(d(due)),
        }
    }

    // October 2026: Mon 12, Tue 13, Wed 14, Thu 15, Fri 16, Mon 19, Tue 20.

    #[test]
    fn successors_move_along_keeping_their_length() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-15", "2026-10-16"),
                task(3, "2026-10-19", "2026-10-19"),
            ],
            vec![fs(1, 2), fs(2, 3)],
        );
        let moves = p.push(moved(1, "2026-10-13", "2026-10-15"));
        assert_eq!(
            moves,
            vec![
                moved(1, "2026-10-13", "2026-10-15"),
                moved(2, "2026-10-16", "2026-10-19"),
                moved(3, "2026-10-20", "2026-10-20"),
            ]
        );
    }

    #[test]
    fn nothing_is_pulled_earlier() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-19", "2026-10-20"),
            ],
            vec![fs(1, 2)],
        );
        assert_eq!(
            p.push(moved(1, "2026-10-12", "2026-10-13")),
            vec![moved(1, "2026-10-12", "2026-10-13")]
        );
        // Room enough: the successor stays.
        assert_eq!(p.push(moved(1, "2026-10-13", "2026-10-15")).len(), 1);
    }

    #[test]
    fn a_milestone_stays_and_is_warned_about() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                milestone(2, "2026-10-16"),
            ],
            vec![fs(1, 2)],
        );
        let moves = p.push(moved(1, "2026-10-15", "2026-10-19"));
        assert_eq!(moves, vec![moved(1, "2026-10-15", "2026-10-19")]);
        let after = plan(
            vec![
                task(1, "2026-10-15", "2026-10-19"),
                milestone(2, "2026-10-16"),
            ],
            vec![fs(1, 2)],
        );
        assert_eq!(
            after.analyse().warnings,
            vec![Warning::Milestone {
                task: id(2),
                earliest: d("2026-10-19")
            }],
            "a milestone may fall on the day its predecessor ends"
        );
    }

    #[test]
    fn a_chain_runs_through_tasks_without_dates() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                open(2),
                task(3, "2026-10-15", "2026-10-16"),
            ],
            vec![fs(1, 2), fs(2, 3)],
        );
        let moves = p.push(moved(1, "2026-10-14", "2026-10-16"));
        assert_eq!(
            moves,
            vec![
                moved(1, "2026-10-14", "2026-10-16"),
                moved(3, "2026-10-19", "2026-10-20"),
            ]
        );
    }

    #[test]
    fn start_to_start_and_finish_to_finish_with_lag() {
        let ss = Link {
            kind: DependencyKind::StartToStart,
            lag: 1,
            ..fs(1, 2)
        };
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-13", "2026-10-13"),
            ],
            vec![ss],
        );
        // Start moves to Wednesday: the other may start Thursday at the earliest.
        assert_eq!(
            p.push(moved(1, "2026-10-14", "2026-10-16")),
            vec![
                moved(1, "2026-10-14", "2026-10-16"),
                moved(2, "2026-10-15", "2026-10-15"),
            ]
        );
        let ff = Link {
            kind: DependencyKind::FinishToFinish,
            lag: 0,
            ..fs(1, 2)
        };
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-13", "2026-10-14"),
            ],
            vec![ff],
        );
        // The first now ends Monday: the second ends Monday too, still two days long.
        assert_eq!(
            p.push(moved(1, "2026-10-15", "2026-10-19")),
            vec![
                moved(1, "2026-10-15", "2026-10-19"),
                moved(2, "2026-10-16", "2026-10-19"),
            ]
        );
    }

    #[test]
    fn a_drag_onto_a_weekend_starts_on_monday_with_the_same_length() {
        let p = plan(vec![task(1, "2026-10-12", "2026-10-14")], vec![]);
        assert_eq!(
            p.shifted(id(1), d("2026-10-17")),
            Some(moved(1, "2026-10-19", "2026-10-21"))
        );
        // Over a holiday: Wed 30 Oct, Sat 31 is Reformation Day anyway; Christmas instead.
        assert_eq!(
            p.shifted(id(1), d("2026-12-24")),
            Some(moved(1, "2026-12-24", "2026-12-29"))
        );
    }

    #[test]
    fn circles_in_old_data_do_not_loop() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-15", "2026-10-16"),
            ],
            vec![fs(1, 2), fs(2, 1)],
        );
        let _ = p.push(moved(1, "2026-10-19", "2026-10-21"));
        let _ = p.analyse();
    }

    #[test]
    fn the_critical_path_has_no_float() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-15", "2026-10-16"),
                task(3, "2026-10-12", "2026-10-12"),
            ],
            vec![fs(1, 2)],
        );
        let a = p.analyse();
        assert_eq!(a.critical, BTreeSet::from([id(1), id(2)]));
        assert_eq!(a.float.get(&id(3)), Some(&4));
        assert!(a.warnings.is_empty());
        // A milestone fixes its own end: what leads to it is critical even if it ends early.
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                milestone(2, "2026-10-14"),
                task(3, "2026-10-12", "2026-10-20"),
            ],
            vec![fs(1, 2)],
        );
        assert!(p.analyse().critical.contains(&id(1)));
    }

    #[test]
    fn broken_dependencies_and_late_tasks_are_warned_about() {
        let p = Plan::new(
            Calendar::hamburg(),
            vec![
                task(1, "2026-10-12", "2026-10-16"),
                task(2, "2026-10-14", "2026-10-20"),
            ],
            vec![fs(1, 2)],
            Some(d("2026-10-19")),
        );
        let warnings = p.analyse().warnings;
        assert!(warnings.contains(&Warning::Violated {
            from: id(1),
            to: id(2)
        }));
        assert!(warnings.contains(&Warning::AfterProjectDue { task: id(2) }));
        assert!(p.analyse().float.get(&id(2)).is_some_and(|f| *f <= 0));
    }

    fn done(n: u128, start: &str, due: &str) -> Item {
        Item {
            done: true,
            ..task(n, start, due)
        }
    }

    #[test]
    fn an_end_on_a_weekend_is_the_friday_before() {
        // Ends Sunday 18th: the successor may start Monday 19th.
        let p = plan(
            vec![
                task(1, "2026-10-14", "2026-10-18"),
                task(2, "2026-10-19", "2026-10-20"),
            ],
            vec![fs(1, 2)],
        );
        assert!(p.analyse().warnings.is_empty());
        assert_eq!(p.push(moved(1, "2026-10-15", "2026-10-18")).len(), 1);
        // Starts Saturday 17th, really Monday 19th: a start-to-start successor on Monday keeps
        // its dependency.
        let ss = Link {
            kind: DependencyKind::StartToStart,
            lag: 0,
            ..fs(1, 2)
        };
        let p = plan(
            vec![
                task(1, "2026-10-17", "2026-10-20"),
                task(2, "2026-10-19", "2026-10-19"),
            ],
            vec![ss],
        );
        assert!(p.analyse().warnings.is_empty());
        // …but one starting Friday 16th starts too early.
        let p = plan(
            vec![
                task(1, "2026-10-17", "2026-10-20"),
                task(2, "2026-10-16", "2026-10-16"),
            ],
            vec![ss],
        );
        assert_eq!(
            p.analyse().warnings,
            vec![Warning::Violated {
                from: id(1),
                to: id(2)
            }]
        );
        // A successor starting Saturday 17th after a Friday end really starts Monday: fine,
        // and it is not moved.
        let p = plan(
            vec![
                task(1, "2026-10-14", "2026-10-16"),
                task(2, "2026-10-17", "2026-10-20"),
            ],
            vec![fs(1, 2)],
        );
        assert!(p.analyse().warnings.is_empty());
        assert_eq!(p.push(moved(1, "2026-10-13", "2026-10-16")).len(), 1);
        // Finish-to-finish ending on a holiday (Sat 3 Oct 2026 is one anyway; Christmas Day):
        // the successor may end on Thursday 24th.
        let ff = Link {
            kind: DependencyKind::FinishToFinish,
            lag: 0,
            ..fs(1, 2)
        };
        let p = plan(
            vec![
                task(1, "2026-12-21", "2026-12-25"),
                task(2, "2026-12-23", "2026-12-24"),
            ],
            vec![ff],
        );
        assert!(p.analyse().warnings.is_empty());
        // Critical with the weekend end: nothing in between, no float.
        let p = plan(
            vec![
                task(1, "2026-10-14", "2026-10-18"),
                task(2, "2026-10-19", "2026-10-20"),
            ],
            vec![fs(1, 2)],
        );
        assert_eq!(p.analyse().float.get(&id(1)), Some(&0));
    }

    #[test]
    fn a_milestone_is_critical_only_through_what_it_waits_for() {
        let p = plan(
            vec![
                milestone(1, "2026-10-14"),
                task(2, "2026-10-12", "2026-10-20"),
            ],
            vec![],
        );
        let a = p.analyse();
        assert!(!a.critical.contains(&id(1)), "waits for nothing");
        assert_eq!(a.float.get(&id(1)), None);
        // Waiting for a task that ends two working days before.
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-13"),
                milestone(2, "2026-10-15"),
                task(3, "2026-10-12", "2026-10-20"),
            ],
            vec![fs(1, 2)],
        );
        let a = p.analyse();
        assert_eq!(a.float.get(&id(2)), Some(&2));
        assert!(!a.critical.contains(&id(2)));
        // On the day its predecessor ends: critical. On a Saturday after a Friday end: too.
        for due in ["2026-10-13", "2026-10-17"] {
            let end = if due == "2026-10-13" {
                "2026-10-13"
            } else {
                "2026-10-16"
            };
            let p = plan(
                vec![task(1, "2026-10-12", end), milestone(2, due)],
                vec![fs(1, 2)],
            );
            let a = p.analyse();
            assert_eq!(a.float.get(&id(2)), Some(&0), "{due}");
            assert!(a.critical.contains(&id(2)), "{due}");
            assert!(a.warnings.is_empty(), "{due}");
        }
    }

    #[test]
    fn done_tasks_stay_and_still_hold_back_what_waits_for_them() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                done(2, "2026-10-15", "2026-10-15"),
                task(3, "2026-10-16", "2026-10-16"),
            ],
            vec![fs(1, 2), fs(2, 3)],
        );
        // Moving the first: the done one stays, and so does the one after it.
        assert_eq!(
            p.push(moved(1, "2026-10-14", "2026-10-16")),
            vec![moved(1, "2026-10-14", "2026-10-16")]
        );
        let after = plan(
            vec![
                task(1, "2026-10-14", "2026-10-16"),
                done(2, "2026-10-15", "2026-10-15"),
                task(3, "2026-10-16", "2026-10-16"),
            ],
            vec![fs(1, 2), fs(2, 3)],
        );
        let a = after.analyse();
        assert!(a.warnings.is_empty(), "no warning about a done task");
        assert!(!a.critical.contains(&id(2)));
        assert_eq!(a.float.get(&id(2)), None);
        // Its dates still count for the task after it.
        assert_eq!(
            p.push(moved(2, "2026-10-16", "2026-10-16")),
            vec![
                moved(2, "2026-10-16", "2026-10-16"),
                moved(3, "2026-10-19", "2026-10-19"),
            ]
        );
        // Done, late for the project: no warning either.
        let p = Plan::new(
            Calendar::hamburg(),
            vec![done(1, "2026-10-12", "2026-10-20")],
            vec![],
            Some(d("2026-10-16")),
        );
        assert!(p.analyse().warnings.is_empty());
    }

    #[test]
    fn a_task_without_dates_takes_no_time_backwards_either() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                open(2),
                task(3, "2026-10-15", "2026-10-15"),
            ],
            vec![fs(1, 2), fs(2, 3)],
        );
        let a = p.analyse();
        assert!(a.warnings.is_empty());
        assert_eq!(a.float.get(&id(1)), Some(&0));
        assert!(a.critical.contains(&id(1)));
    }

    #[test]
    fn a_milestone_is_warned_about_once_with_the_latest_date() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-15"),
                task(2, "2026-10-12", "2026-10-19"),
                milestone(3, "2026-10-14"),
            ],
            vec![fs(1, 3), fs(2, 3)],
        );
        assert_eq!(
            p.analyse().warnings,
            vec![Warning::Milestone {
                task: id(3),
                earliest: d("2026-10-19")
            }]
        );
    }

    #[test]
    fn a_dragged_milestone_drops_a_start_after_its_new_date() {
        let p = plan(
            vec![Item {
                start: Some(d("2026-10-14")),
                ..milestone(1, "2026-10-16")
            }],
            vec![],
        );
        assert_eq!(
            p.shifted(id(1), d("2026-10-13")),
            Some(Move {
                task: id(1),
                start: None,
                due: Some(d("2026-10-13"))
            })
        );
        assert_eq!(
            p.shifted(id(1), d("2026-10-19")).and_then(|m| m.start),
            Some(d("2026-10-14"))
        );
    }

    #[test]
    fn a_task_done_early_does_not_hide_the_critical_path() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-15", "2026-10-16"),
                done(3, "2026-10-12", "2026-11-30"),
            ],
            vec![fs(1, 2)],
        );
        let a = p.analyse();
        assert_eq!(a.critical, BTreeSet::from([id(1), id(2)]));
        // A task without dates (with a lag) does not set the end either.
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                task(2, "2026-10-15", "2026-10-16"),
                open(3),
            ],
            vec![
                fs(1, 2),
                Link {
                    lag: 10,
                    ..fs(2, 3)
                },
            ],
        );
        assert_eq!(p.analyse().critical, BTreeSet::from([id(1), id(2)]));
    }

    #[test]
    fn a_done_task_without_dates_still_carries_the_chain_backwards() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-14"),
                Item {
                    done: true,
                    ..open(2)
                },
                task(3, "2026-10-15", "2026-10-16"),
            ],
            vec![fs(1, 2), fs(2, 3)],
        );
        assert_eq!(p.analyse().float.get(&id(1)), Some(&0));
        assert_eq!(p.push(moved(1, "2026-10-13", "2026-10-15")).len(), 2);
    }

    #[test]
    fn a_task_with_only_a_start_on_a_day_off_starts_the_working_day_after() {
        let p = plan(
            vec![
                task(1, "2026-10-12", "2026-10-16"),
                Item {
                    due: None,
                    ..task(2, "2026-10-17", "2026-10-17")
                },
            ],
            vec![fs(1, 2)],
        );
        assert!(p.analyse().warnings.is_empty());
        assert_eq!(p.push(moved(1, "2026-10-12", "2026-10-16")), vec![]);
    }

    #[test]
    fn a_task_wholly_on_days_off_is_the_working_day_before() {
        // Only a deadline, on Saturday 17th: it stands for Friday 16th.
        let saturday = Item {
            start: None,
            ..task(2, "2026-10-17", "2026-10-17")
        };
        let p = plan(
            vec![task(1, "2026-10-12", "2026-10-16"), saturday.clone()],
            vec![fs(1, 2)],
        );
        assert_eq!(
            p.analyse().warnings,
            vec![Warning::Violated {
                from: id(1),
                to: id(2)
            }],
            "Friday is the predecessor's last day"
        );
        // The latest task, alone: no float lost, nothing negative.
        let p = plan(vec![saturday.clone()], vec![]);
        assert_eq!(p.analyse().float.get(&id(2)), Some(&0));
        // Before a plan ending Wednesday 21st: three working days to spare (Mon, Tue, Wed).
        let p = plan(
            vec![saturday.clone(), task(3, "2026-10-19", "2026-10-21")],
            vec![],
        );
        assert_eq!(p.analyse().float.get(&id(2)), Some(&3));
        // Pushed, it stays one working day long.
        let p = plan(
            vec![task(1, "2026-10-12", "2026-10-14"), saturday],
            vec![fs(1, 2)],
        );
        assert_eq!(
            p.push(moved(1, "2026-10-14", "2026-10-16")),
            vec![
                moved(1, "2026-10-14", "2026-10-16"),
                Move {
                    task: id(2),
                    start: None,
                    due: Some(d("2026-10-19"))
                },
            ]
        );
    }

    /// A random plan: tasks 0..n on working days of October/November 2026, dependencies only
    /// forward (no circles), some milestones and some tasks without dates.
    fn random_plan() -> impl Strategy<Value = (Vec<Item>, Vec<Link>)> {
        (2usize..12).prop_flat_map(|n| {
            let items = prop::collection::vec((0u8..40, 0u8..6, 0u8..10), n);
            let links = prop::collection::vec((0usize..n, 0usize..n, 0u8..3, -2i32..3), 0..n * 2);
            (items, links).prop_map(|(items, links)| {
                let base = d("2026-10-01");
                let items: Vec<Item> = items
                    .into_iter()
                    .enumerate()
                    .map(|(i, (offset, length, kind))| {
                        // Calendar days: starts and ends on weekends and holidays too.
                        let start = base + jiff::Span::new().days(i64::from(offset));
                        let end = start + jiff::Span::new().days(i64::from(length));
                        let n = i as u128 + 1;
                        match kind {
                            0 => open(n),
                            3 => Item {
                                done: true,
                                ..open(n)
                            },
                            4 => Item {
                                id: id(n),
                                start: Some(start),
                                due: None,
                                milestone: false,
                                done: false,
                            },
                            5 => Item {
                                id: id(n),
                                start: None,
                                due: Some(end),
                                milestone: false,
                                done: false,
                            },
                            1 => Item {
                                id: id(n),
                                start: None,
                                due: Some(end),
                                milestone: true,
                                done: false,
                            },
                            _ => Item {
                                id: id(n),
                                start: Some(start),
                                due: Some(end),
                                milestone: false,
                                done: kind == 2,
                            },
                        }
                    })
                    .collect();
                let links = links
                    .into_iter()
                    .filter(|(a, b, _, _)| a < b)
                    .map(|(a, b, kind, lag)| Link {
                        from: id(a as u128 + 1),
                        to: id(b as u128 + 1),
                        kind: [
                            DependencyKind::FinishToStart,
                            DependencyKind::StartToStart,
                            DependencyKind::FinishToFinish,
                        ][usize::from(kind)],
                        lag,
                    })
                    .collect();
                (items, links)
            })
        })
    }

    /// The end of the open work of `p` and whether a milestone cannot hold.
    fn outcome(p: &Plan) -> (Option<Date>, bool) {
        let end = p
            .items()
            .filter(|i| !i.done)
            .filter_map(|i| p.placed(i).map(|(_, end)| end))
            .max();
        let late = p
            .analyse()
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::Milestone { .. }));
        (end, late)
    }

    proptest! {
        #[test]
        fn slipping_by_the_float_is_harmless_and_one_day_more_is_not((items, links) in random_plan(), pick in 0usize..12, every in any::<bool>()) {
            let cal = if every { Calendar::every_day() } else { Calendar::hamburg() };
            let p = Plan::new(cal, items.clone(), links.clone(), None);
            let a = p.analyse();
            // Float only means something for a plan that holds.
            if !a.warnings.is_empty() {
                return Ok(());
            }
            let Some(target) = items
                .iter()
                .filter(|i| !i.milestone && !i.done && i.span().is_some())
                .nth(pick % items.len().max(1))
            else {
                return Ok(());
            };
            let Some(&float) = a.float.get(&target.id) else {
                return Ok(());
            };

            let start = p.placed(target).unwrap().0;
            let before = outcome(&p);
            let slipped = |days: i32| {
                let first = p.shifted(target.id, cal.from_start(start, days)).unwrap();
                let moves = p.push(first);
                let after: Vec<Item> = items
                    .iter()
                    .map(|i| match moves.iter().find(|m| m.task == i.id) {
                        Some(m) => Item { start: m.start, due: m.due, ..i.clone() },
                        None => i.clone(),
                    })
                    .collect();
                outcome(&Plan::new(cal, after, links.clone(), None))
            };
            prop_assert_eq!(slipped(float), before, "float {} of {:?} is too much", float, target.id);
            prop_assert_ne!(slipped(float + 1), before, "float {} of {:?} is too little", float, target.id);
        }

        #[test]
        fn pushing_keeps_every_dependency_and_every_length((items, links) in random_plan(), pick in 0usize..12, later in 0i32..15, every in any::<bool>()) {
            let cal = if every { Calendar::every_day() } else { Calendar::hamburg() };
            let p = Plan::new(cal, items.clone(), links.clone(), None);
            let Some(target) = items.iter().filter(|i| i.span().is_some() && !i.milestone).nth(pick % items.len().max(1)) else {
                return Ok(());
            };
            let (start, _) = target.span().unwrap();
            let Some(first) = p.shifted(target.id, start + jiff::Span::new().days(i64::from(later))) else {
                return Ok(());
            };
            let moves = p.push(first);
            let after: Vec<Item> = items
                .iter()
                .map(|i| match moves.iter().find(|m| m.task == i.id) {
                    Some(m) => Item { start: m.start, due: m.due, ..i.clone() },
                    None => i.clone(),
                })
                .collect();
            let before: BTreeMap<TaskId, &Item> = items.iter().map(|i| (i.id, i)).collect();
            for (old, new) in items.iter().zip(&after) {
                if let (Some(a), Some(b)) = (old.span(), new.span()) {
                    // Nothing earlier, the same length.
                    prop_assert!(b.0 >= a.0, "{:?} moved earlier", new.id);
                    if old.id != target.id && (old.milestone || old.done) {
                        prop_assert_eq!(a, b, "a milestone or done task does not move");
                    } else if !old.milestone {
                        let length = |i: &Item| p.duration(i, p.placed(i).unwrap());
                        prop_assert_eq!(length(old), length(new), "{:?} changed its length", new.id);
                    }
                }
            }
            // Every task that waits for the moved one (directly or not) keeps all its
            // dependencies; milestones stay and are warned about instead.
            let reach = p.reachable(target.id);
            for warning in Plan::new(cal, after.clone(), links.clone(), None).analyse().warnings {
                if let Warning::Violated { to, .. } = warning {
                    prop_assert!(!reach.contains(&to), "{:?} still waits too short after the push", to);
                }
            }
            // Tasks that do not wait for it stay where they were.
            for (old, new) in items.iter().zip(&after) {
                if old.id != target.id && !reach.contains(&old.id) {
                    prop_assert_eq!(old, new);
                }
            }
            let _ = before;
        }
    }
}
