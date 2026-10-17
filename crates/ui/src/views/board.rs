//! The Kanban board (Phase 9e, plan §7.2): one column per status, filling the window (at least
//! [`COLUMN_MIN`] wide, otherwise it scrolls sideways); cards are moved by drag and drop, a line
//! shows where one lands. Cards are the top-level tasks; their subtasks are listed in the card,
//! each with its own status (decision 2026-10-15), fold away, and go wherever the card goes.
//! Above: a filter by person, overdue and words in the title.

use egui::{Id, Sense, Stroke, vec2};
use jiff::civil::DateTime;
use plnwrk_core::{Priority, Project, Task, TaskId, TaskStatus, Workspace};
use plnwrk_engine::query::{self, TaskCard};
use serde_json::{Map, Value, json};

use crate::design::widgets::{self, Tone};
use crate::design::{Icon, palette};
use crate::doc::OpenDoc;
use crate::state::{BoardFilter, Dialog, State};
use crate::{Clear, Intent, Then, dates, text};

/// The narrowest a column gets; below, the board scrolls sideways.
const COLUMN_MIN: f32 = 220.0;
/// A deadline this many days ahead or fewer is "soon".
const SOON_DAYS: i64 = 3;
/// Parameters of `task.move` that put `task` (now in status `from`) at position `index` among
/// the other cards of the column `status` (`column`: its cards in order, possibly with the
/// task itself). `None` if the card would stay where it is.
pub fn move_params(
    task: TaskId,
    from: TaskStatus,
    status: TaskStatus,
    column: &[TaskId],
    index: usize,
) -> Option<Value> {
    let others: Vec<TaskId> = column.iter().copied().filter(|id| *id != task).collect();
    let index = index.min(others.len());
    let after = index.checked_sub(1).and_then(|i| others.get(i)).copied();
    let before = others.get(index).copied();
    if from == status
        && let Some(position) = column.iter().position(|id| *id == task)
    {
        let now_after = position.checked_sub(1).and_then(|i| column.get(i)).copied();
        let now_before = position.checked_add(1).and_then(|i| column.get(i)).copied();
        if (now_after, now_before) == (after, before) {
            return None;
        }
    }
    let mut map = Map::new();
    map.insert("task".to_owned(), json!(task.to_string()));
    if from != status {
        map.insert("status".to_owned(), json!(status.name()));
    }
    if let Some(after) = after {
        map.insert("after".to_owned(), json!(after.to_string()));
    }
    if let Some(before) = before {
        map.insert("before".to_owned(), json!(before.to_string()));
    }
    Some(Value::Object(map))
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
    let Some(board) = query::board(ws, project.id, now) else {
        return;
    };
    // Each project keeps its own filter.
    filters(
        ui,
        ws,
        project,
        state.board_filters.entry(project.id).or_default(),
    );
    ui.add_space(6.0);
    let counts = query::deadlines_count(project);
    // The card being dragged, if any, for the line where it would land and the hint.
    let dragged = egui::DragAndDrop::payload::<TaskId>(ui.ctx()).map(|id| *id);
    let gap = 8.0;
    let columns = board.columns.len().max(1) as f32;
    let width = ((ui.available_width() - gap * (columns - 1.0) - 16.0) / columns).max(COLUMN_MIN);
    let mut fold = None;
    egui::ScrollArea::both()
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for column in &board.columns {
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        column_ui(
                            ui,
                            ws,
                            project,
                            column,
                            (width, now, counts, dragged),
                            state,
                            &mut fold,
                            out,
                        );
                    });
                }
            });
        });
    if let Some(card) = fold
        && !state.board_folded.remove(&card)
    {
        state.board_folded.insert(card);
    }
}

/// The filter row: person, only overdue, words in the title.
fn filters(ui: &mut egui::Ui, ws: &Workspace, project: &Project, filter: &mut BoardFilter) {
    let t = text::t();
    ui.horizontal_wrapped(|ui| {
        // A person filtered for who is gone from the file: the filter goes with them.
        if let Some(Some(id)) = filter.assignee
            && ws.person(id).is_none()
        {
            filter.assignee = None;
        }
        // The people in charge of something here, and the one chosen (even with nothing left
        // to do: the choice must show what is filtered).
        let people: Vec<(plnwrk_core::PersonId, String)> = {
            let mut ids: Vec<plnwrk_core::PersonId> = ws
                .tasks_of(project.id)
                .filter_map(|t| t.assignee)
                .chain(filter.assignee.flatten())
                .collect();
            ids.sort();
            ids.dedup();
            ids.into_iter()
                .filter_map(|id| ws.person(id).map(|p| (id, p.name.clone())))
                .collect()
        };
        let chosen = match filter.assignee {
            None => t.board_filter_everyone.to_owned(),
            Some(None) => t.no_assignee.to_owned(),
            Some(Some(id)) => people
                .iter()
                .find(|(p, _)| *p == id)
                .map_or_else(|| t.board_filter_everyone.to_owned(), |(_, n)| n.clone()),
        };
        let caption = ui.label(t.assignee);
        egui::ComboBox::from_id_salt(("board-person", project.id))
            .selected_text(chosen)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut filter.assignee, None, t.board_filter_everyone);
                ui.selectable_value(&mut filter.assignee, Some(None), t.no_assignee);
                for (id, name) in &people {
                    ui.selectable_value(&mut filter.assignee, Some(Some(*id)), name);
                }
            })
            .response
            .labelled_by(caption.id);
        ui.add_space(8.0);
        ui.checkbox(&mut filter.overdue, t.board_filter_overdue);
        ui.add_space(8.0);
        let caption = ui.label(t.board_search_label);
        ui.add(
            egui::TextEdit::singleline(&mut filter.search)
                .hint_text(t.board_search)
                .desired_width(200.0),
        )
        .labelled_by(caption.id);
        if filter.active() {
            ui.add_space(8.0);
            if widgets::button(ui, widgets::Kind::Quiet, t.board_filter_reset).clicked() {
                *filter = BoardFilter::default();
            }
        }
    });
}

/// One column: heading with "+", a new card being typed, the cards, the line where a dragged
/// card would land, and the hint about open work when dragging to "Erledigt".
#[expect(clippy::too_many_arguments, reason = "one view, drawn in one place")]
fn column_ui(
    ui: &mut egui::Ui,
    ws: &Workspace,
    project: &Project,
    column: &query::Column,
    (width, now, counts, dragged): (f32, DateTime, bool, Option<TaskId>),
    state: &mut State,
    fold: &mut Option<TaskId>,
    out: &mut Vec<Intent>,
) {
    let filter = state
        .board_filters
        .get(&project.id)
        .cloned()
        .unwrap_or_default();
    let shown: Vec<&TaskCard> = column.tasks.iter().filter(|c| filter.keeps(c)).collect();
    let heading = ui
        .horizontal(|ui| {
            let heading = ui.strong(text::column_heading(
                column.status,
                shown.len(),
                column.tasks.len(),
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let name = text::new_in_column(column.status);
                if widgets::icon_button(ui, Icon::Plus, &name).clicked() {
                    state.forms.board_new = Some((project.id, column.status, String::new()));
                    state.focus_board_new = true;
                }
            });
            heading
        })
        .inner;
    // A new card typed here (in this project and column): Enter creates it with this status;
    // leaving the field otherwise (Escape, a click elsewhere) drops it.
    if let Some((owner, status, title)) = &mut state.forms.board_new
        && *owner == project.id
        && *status == column.status
    {
        let field = ui.add(
            egui::TextEdit::singleline(title)
                .hint_text(text::t().new_task_hint)
                .desired_width(width - 8.0),
        );
        let field = field.labelled_by(heading.id);
        if std::mem::take(&mut state.focus_board_new) {
            field.request_focus();
        }
        let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let title = title.trim().to_owned();
        if entered && !title.is_empty() {
            out.push(Intent::Run {
                command: "task.create",
                params: json!({"project": project.id.to_string(), "title": title,
                               "status": column.status.name()}),
                then: Then::Nothing,
                clear: Some(Clear::BoardNew),
            });
            // Created, but the filter would hide it: said, so it is not lost from sight.
            if !filter.keeps_new(&title) {
                out.push(Intent::Say(crate::state::Message::info(
                    text::created_but_filtered(&title),
                )));
            }
        } else if field.lost_focus() {
            state.forms.board_new = None;
        }
    }
    // Dragging to "Erledigt" what still waits for open work: said, not stopped.
    let waiting = dragged
        .filter(|_| column.status == TaskStatus::Done)
        .filter(|id| ws.task(*id).is_some_and(|t| t.status != TaskStatus::Done))
        .map(|id| open_before(ws, id))
        .unwrap_or(0);
    let mut rects = Vec::new();
    // The column: a quiet surface (not egui's drop zone, whose frame it would replace with a
    // grey box); framed in the accent while a card is held over it.
    let p = palette(ui);
    let frame = egui::Frame::new()
        .fill(p.shade)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(4));
    // Down to the bottom of what is in view, so a card can be dropped anywhere below the
    // last one – also into a short column beside a long one.
    let reach = (ui.clip_rect().bottom() - ui.cursor().top() - 12.0).max(160.0);
    let zone = frame.show(ui, |ui| {
        ui.set_min_size(vec2(width - 8.0, reach));
        for card in &shown {
            let folded = state.board_folded.contains(&card.id);
            // Folded: the subtasks are not drawn, so not looked up either.
            let subtasks = if folded {
                Vec::new()
            } else {
                children(ws, card.id)
            };
            let held = ui.dnd_drag_source(Id::new(("card", card.id)), card.id, |ui| {
                card_ui(
                    ui,
                    card,
                    &subtasks,
                    (state.selected == Some(card.id), folded),
                    (width - 8.0, now, counts),
                    out,
                )
            });
            let response = held.response;
            let (head, ticks, fold_at, fold_key) = held.inner;
            if fold_key {
                *fold = Some(card.id);
            }
            // Above the drag source, so a click on a tick or the fold arrow reaches it.
            for (subtask, done, rect) in ticks {
                // Mouse only: keyboard and screen readers use the tick.
                let tick = ui.interact(rect, Id::new(("tick", subtask)), Sense::CLICK);
                if tick.clicked() {
                    out.push(toggle(subtask, done));
                }
            }
            if let Some(at) = fold_at
                && ui
                    .interact(at, Id::new(("fold-click", card.id)), Sense::CLICK)
                    .clicked()
            {
                *fold = Some(card.id);
            }
            // The head of the card (title and details) selects on click; dragging stays
            // with the drag source (egui routes clicks and drags separately).
            let click = ui.interact(head, Id::new(("card-click", card.id)), Sense::click());
            if click.clicked() {
                out.push(Intent::Select(Some(card.id)));
            }
            rects.push((card.id, response.rect));
        }
    });
    let over = dragged.is_some() && zone.response.contains_pointer();
    if over {
        ui.painter().rect_stroke(
            zone.response.rect,
            egui::CornerRadius::same(8),
            Stroke::new(2.0, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    // Said at the pointer, where the eyes are – also when the column's top is scrolled away.
    if waiting > 0
        && over
        && let Some(at) = ui.ctx().pointer_interact_pos()
    {
        egui::Area::new(Id::new("board-waits-hint"))
            .order(egui::Order::Tooltip)
            .fixed_pos(at + vec2(16.0, 18.0))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    widgets::field_error(ui, &text::still_waits_for(waiting));
                });
            });
    }
    let dropped = zone.response.dnd_release_payload::<TaskId>();
    // Where a dragged card would land: a line in the gap (the same index as the drop).
    if let Some(task) = dragged
        && over
        && let Some(y) = ui.ctx().pointer_interact_pos().map(|p| p.y)
    {
        let others: Vec<egui::Rect> = rects
            .iter()
            .filter(|(id, _)| *id != task)
            .map(|(_, r)| *r)
            .collect();
        let index = drop_index(&rects, task, y);
        let line_y = match (
            index.checked_sub(1).and_then(|i| others.get(i)),
            others.get(index),
        ) {
            (_, Some(next)) => next.top() - 3.0,
            (Some(last), None) => last.bottom() + 3.0,
            (None, None) => zone.response.rect.top() + 6.0,
        };
        let x = zone.response.rect.x_range();
        ui.painter().hline(
            x.min + 4.0..=x.max - 4.0,
            line_y,
            Stroke::new(2.0, p.accent),
        );
    }
    if let Some(task) = dropped {
        let y = ui
            .ctx()
            .pointer_interact_pos()
            .map_or(f32::INFINITY, |p| p.y);
        let index = drop_index(&rects, *task, y);
        let ids: Vec<TaskId> = shown.iter().map(|c| c.id).collect();
        let from = ws.task(*task).map(|t| t.status);
        if let Some(from) = from
            && let Some(params) = move_params(*task, from, column.status, &ids, index)
        {
            let open = open_below(ws, *task);
            if column.status == TaskStatus::Done && from != TaskStatus::Done && open > 0 {
                out.push(Intent::Open(Dialog::CloseSubtasks {
                    task: *task,
                    params,
                    open,
                }));
            } else {
                out.push(Intent::run("task.move", params));
            }
        }
    }
}

/// Where a card dropped at height `y` lands among the cards of a column (`rects`, in order):
/// the number of the other cards above it. One rule for the line and the drop.
fn drop_index(rects: &[(TaskId, egui::Rect)], task: TaskId, y: f32) -> usize {
    rects
        .iter()
        .filter(|(id, rect)| *id != task && rect.center().y < y)
        .count()
}

/// How many tasks `task` waits for that are not done.
fn open_before(ws: &Workspace, task: TaskId) -> usize {
    ws.dependencies()
        .filter(|d| d.to == task)
        .filter_map(|d| ws.task(d.from))
        .filter(|t| t.status != TaskStatus::Done)
        .count()
}

/// Every subtask below a task, at any depth, in the order of the list (parents before their
/// subtasks), with its depth (1 = direct subtask).
fn children(ws: &Workspace, task: TaskId) -> Vec<(&Task, usize)> {
    let below = |parent: TaskId| {
        let mut children: Vec<&Task> = ws.tasks().filter(|t| t.parent == Some(parent)).collect();
        children.sort_by(|a, b| b.rank.cmp(&a.rank));
        children
    };
    let mut out = Vec::new();
    let mut stack: Vec<(&Task, usize)> = below(task).into_iter().map(|t| (t, 1)).collect();
    while let Some((child, depth)) = stack.pop() {
        out.push((child, depth));
        stack.extend(below(child.id).into_iter().map(|t| (t, depth + 1)));
    }
    out
}

/// How many subtasks below a task (at any depth) are not done.
fn open_below(ws: &Workspace, task: TaskId) -> usize {
    let mut open = 0;
    let mut next = vec![task];
    while let Some(parent) = next.pop() {
        for child in ws.tasks().filter(|t| t.parent == Some(parent)) {
            next.push(child.id);
            if child.status != TaskStatus::Done {
                open += 1;
            }
        }
    }
    open
}

/// One card: title (two lines at most), deadline chip, person, priority, the subtasks with a bar
/// and a fold arrow. Returns the head (selects on click), the ticks, and the fold arrow.
#[expect(
    clippy::type_complexity,
    reason = "the parts the column makes clickable"
)]
fn card_ui(
    ui: &mut egui::Ui,
    card: &TaskCard,
    subtasks: &[(&Task, usize)],
    (selected, folded): (bool, bool),
    (width, now, counts): (f32, DateTime, bool),
    out: &mut Vec<Intent>,
) -> (
    egui::Rect,
    Vec<(TaskId, bool, egui::Rect)>,
    Option<egui::Rect>,
    bool,
) {
    let p = palette(ui);
    let stroke = if selected {
        Stroke::new(2.0, p.accent)
    } else {
        Stroke::new(1.0, p.line)
    };
    egui::Frame::new()
        .fill(p.surface)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(width - 18.0);
            let title = if card.milestone {
                format!("♦ {}", card.title)
            } else {
                card.title.clone()
            };
            // Two lines at most; the whole title in the tooltip and for the screen reader.
            let mut job = egui::text::LayoutJob::single_section(
                title.clone(),
                egui::TextFormat::simple(
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().text_color(),
                ),
            );
            job.wrap.max_width = width - 18.0;
            job.wrap.max_rows = 2;
            let head = ui
                .add(egui::Label::new(job).sense(Sense::click()))
                .on_hover_text(&title);
            head.widget_info(|| {
                let mut info = egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &title);
                info.selected = Some(selected);
                info
            });
            if head.clicked() {
                out.push(Intent::Select(Some(card.id)));
            }
            let details = ui.horizontal_wrapped(|ui| {
                due_chip(ui, card, now, counts);
                if let Some(person) = &card.assignee {
                    person_circle(ui, &person.initials, &person.name);
                }
                // High: a flag; critical: said in a red chip (not colour alone apart).
                let name = text::priority(card.priority);
                if card.priority == Priority::Critical {
                    widgets::chip(ui, Tone::Danger, name);
                } else if card.priority == Priority::High {
                    let (rect, response) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    Icon::Priority.paint(ui.painter(), rect, p.warning);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name)
                    });
                    response.on_hover_text(name);
                }
            });
            let head_rect = head.rect.union(details.response.rect);
            // The subtasks: a bar of what is done, the arrow to fold them, then each to tick off.
            // A mouse click is caught above the drag source (see `column_ui`); keyboard and
            // screen readers act here.
            let mut ticks = Vec::new();
            let mut fold_at = None;
            let mut fold_key = false;
            if card.subtasks > 0 {
                ui.horizontal(|ui| {
                    let icon = if folded {
                        Icon::ChevronRight
                    } else {
                        Icon::ChevronDown
                    };
                    let name = if folded {
                        text::t().unfold_subtasks
                    } else {
                        text::t().fold_subtasks
                    };
                    let arrow = widgets::icon_button(ui, icon, name);
                    // Keyboard: the button itself (a mouse click lands on the column's interact
                    // above the drag source).
                    fold_key = arrow.clicked();
                    fold_at = Some(arrow.rect);
                    let fraction = card.subtasks_done as f32 / card.subtasks as f32;
                    widgets::progress(
                        ui,
                        fraction,
                        &format!("{}/{}", card.subtasks_done, card.subtasks),
                    );
                });
                if !folded {
                    for (subtask, depth) in subtasks {
                        let done = subtask.status == TaskStatus::Done;
                        let mut shown = done;
                        let response = ui
                            .horizontal(|ui| {
                                ui.add_space(depth.saturating_sub(1) as f32 * 14.0);
                                ui.checkbox(&mut shown, &subtask.title)
                            })
                            .inner;
                        if response.changed() {
                            out.push(toggle(subtask.id, done));
                        }
                        ticks.push((subtask.id, done, response.rect));
                    }
                }
            }
            (head_rect, ticks, fold_at, fold_key)
        })
        .inner
}

/// The deadline as a chip coloured by how it stands, saying it in words too.
fn due_chip(ui: &mut egui::Ui, card: &TaskCard, now: DateTime, counts: bool) {
    if let Some((tone, said)) = due_look(card, now, counts) {
        let date = card
            .due
            .map(|due| dates::format_deadline(due, card.due_time))
            .unwrap_or_default();
        widgets::chip(ui, tone, &said).on_hover_text(date);
    }
}

/// How a card's deadline looks: overdue, today, soon, later – in colour and words. Done work
/// and projects where deadlines do not count: the date, plain.
fn due_look(card: &TaskCard, now: DateTime, counts: bool) -> Option<(Tone, String)> {
    let due = card.due?;
    let days = query::days_between(now.date(), due);
    let state = || text::deadline_state(due, card.due_time, now).0;
    Some(if !counts || card.status == TaskStatus::Done {
        (Tone::Neutral, dates::format_deadline(due, card.due_time))
    } else if card.overdue {
        (Tone::Danger, state())
    } else if days == 0 {
        (Tone::Warning, state())
    } else if days <= SOON_DAYS {
        (Tone::Info, state())
    } else {
        (Tone::Neutral, dates::format_short(due))
    })
}

/// The person in charge as a small circle with initials; the name on hover and when read out.
fn person_circle(ui: &mut egui::Ui, initials: &str, name: &str) {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
    ui.painter()
        .circle(rect.center(), 11.0, p.accent_soft, Stroke::NONE);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        egui::TextStyle::Small.resolve(ui.style()),
        p.accent,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name));
    response.on_hover_text(name);
}

/// Ticks a subtask off, or opens it again.
fn toggle(subtask: TaskId, done: bool) -> Intent {
    let status = if done {
        TaskStatus::Todo
    } else {
        TaskStatus::Done
    };
    Intent::run(
        "task.set_status",
        json!({"task": subtask.to_string(), "status": status.name()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: u128) -> Vec<TaskId> {
        (1..=n).map(|i| TaskId::from(ulid::Ulid(i))).collect()
    }

    fn card(due: &str, status: TaskStatus, overdue: bool) -> TaskCard {
        TaskCard {
            id: ids(1)[0],
            project: plnwrk_core::ProjectId::from_ulid(ulid::Ulid(9)),
            parent: None,
            title: "Karte".to_owned(),
            status,
            priority: Priority::Normal,
            assignee: None,
            due: Some(due.parse().unwrap()),
            due_time: None,
            milestone: false,
            overdue,
            progress: 0,
            tags: Vec::new(),
            subtasks: 0,
            subtasks_done: 0,
            classification: plnwrk_core::Classification::default(),
        }
    }

    /// The colour of a deadline (the words are checked on the board): overdue red, today
    /// amber, up to 3 days blue, later and done plain; where deadlines do not count, plain.
    #[test]
    fn a_deadline_is_coloured_by_how_it_stands() {
        let now: DateTime = "2026-10-17T09:00".parse().unwrap();
        let tone = |due: &str, status, overdue, counts| {
            due_look(&card(due, status, overdue), now, counts).map(|(tone, _)| tone)
        };
        let open = TaskStatus::Todo;
        assert_eq!(tone("2026-10-14", open, true, true), Some(Tone::Danger));
        assert_eq!(tone("2026-10-17", open, false, true), Some(Tone::Warning));
        assert_eq!(tone("2026-10-20", open, false, true), Some(Tone::Info));
        assert_eq!(tone("2026-10-21", open, false, true), Some(Tone::Neutral));
        assert_eq!(
            tone("2026-10-14", TaskStatus::Done, false, true),
            Some(Tone::Neutral)
        );
        assert_eq!(tone("2026-10-17", open, false, false), Some(Tone::Neutral));
    }

    fn rect(top: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, top), egui::vec2(100.0, 40.0))
    }

    /// Where a dropped card lands: the cards above the pointer, not counting itself (the line
    /// while dragging uses the same rule).
    #[test]
    fn a_card_lands_below_the_cards_above_the_pointer() {
        let all = ids(3);
        let [a, b, c] = all[..] else { panic!() };
        let rects = [(a, rect(0.0)), (b, rect(50.0)), (c, rect(100.0))];
        assert_eq!(drop_index(&rects, c, -5.0), 0);
        assert_eq!(drop_index(&rects, c, 30.0), 1);
        assert_eq!(drop_index(&rects, c, 200.0), 2, "itself not counted");
        assert_eq!(drop_index(&rects, a, 200.0), 2);
        assert_eq!(drop_index(&rects, a, 75.0), 1, "b above, a itself not");
    }

    #[test]
    fn into_an_empty_column_only_changes_the_status() {
        let [a] = ids(1)[..] else { panic!() };
        let params = move_params(a, TaskStatus::Todo, TaskStatus::Done, &[], 0).unwrap();
        assert_eq!(params, json!({"task": a.to_string(), "status": "done"}));
    }

    #[test]
    fn between_cards_of_another_column() {
        let all = ids(4);
        let [a, b, c, d] = all[..] else { panic!() };
        let column = [b, c, d];
        let first = move_params(a, TaskStatus::Todo, TaskStatus::Review, &column, 0).unwrap();
        assert_eq!(first["before"], json!(b.to_string()));
        assert!(first.get("after").is_none());
        let middle = move_params(a, TaskStatus::Todo, TaskStatus::Review, &column, 2).unwrap();
        assert_eq!(middle["after"], json!(c.to_string()));
        assert_eq!(middle["before"], json!(d.to_string()));
        let last = move_params(a, TaskStatus::Todo, TaskStatus::Review, &column, 9).unwrap();
        assert_eq!(last["after"], json!(d.to_string()));
        assert!(last.get("before").is_none());
    }

    #[test]
    fn within_a_column_and_onto_its_own_place() {
        let all = ids(3);
        let [a, b, c] = all[..] else { panic!() };
        let column = [a, b, c];
        // b dropped where it is (index counts the other cards above it: a).
        assert_eq!(
            move_params(b, TaskStatus::Todo, TaskStatus::Todo, &column, 1),
            None
        );
        // a to the end: after c, no status change.
        let params = move_params(a, TaskStatus::Todo, TaskStatus::Todo, &column, 2).unwrap();
        assert_eq!(
            params,
            json!({"task": a.to_string(), "after": c.to_string()})
        );
        // c to the top.
        let params = move_params(c, TaskStatus::Todo, TaskStatus::Todo, &column, 0).unwrap();
        assert_eq!(
            params,
            json!({"task": c.to_string(), "before": a.to_string()})
        );
        // Alone in its column: nothing to do.
        assert_eq!(
            move_params(a, TaskStatus::Done, TaskStatus::Done, &[a], 0),
            None
        );
    }
}
