//! The Kanban board: one column per status; cards are moved by drag and drop. Cards are the
//! top-level tasks; their subtasks are listed in the card, each with its own status (decision
//! 2026-10-15), and go wherever the card goes.

use egui::{Id, RichText};
use plnwrk_core::{Priority, Project, Task, TaskId, TaskStatus, Workspace};
use plnwrk_engine::query::{self, TaskCard};
use serde_json::{Map, Value, json};

use crate::doc::OpenDoc;
use crate::state::{Dialog, State};
use crate::views::due_label;
use crate::{Intent, text};

const COLUMN_WIDTH: f32 = 210.0;

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
    state: &State,
    out: &mut Vec<Intent>,
) {
    let ws = doc.session.workspace();
    let now = doc.session.clock().now_local();
    let Some(board) = query::board(ws, project.id, now) else {
        return;
    };
    egui::ScrollArea::both().show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for column in &board.columns {
                ui.vertical(|ui| {
                    ui.set_width(COLUMN_WIDTH);
                    ui.strong(format!(
                        "{} ({})",
                        text::task_status(column.status),
                        column.tasks.len()
                    ));
                    let mut rects = Vec::new();
                    let frame = egui::Frame::default().inner_margin(6.0);
                    let (_, dropped) = ui.dnd_drop_zone::<TaskId, ()>(frame, |ui| {
                        ui.set_min_size(egui::vec2(COLUMN_WIDTH, 160.0));
                        for card in &column.tasks {
                            let dragged =
                                ui.dnd_drag_source(Id::new(("card", card.id)), card.id, |ui| {
                                    card_ui(
                                        ui,
                                        card,
                                        &children(ws, card.id),
                                        state.selected == Some(card.id),
                                        out,
                                    )
                                });
                            let response = dragged.response;
                            let (head, ticks) = dragged.inner;
                            // Above the drag source, so a click on a tick reaches it.
                            for (subtask, done, rect) in ticks {
                                let tick = ui.interact(
                                    rect,
                                    Id::new(("tick", subtask)),
                                    egui::Sense::click(),
                                );
                                if tick.clicked() {
                                    out.push(toggle(subtask, done));
                                }
                            }
                            // The head of the card (title and details) selects on click;
                            // dragging stays with the drag source (egui routes clicks and
                            // drags separately). The subtasks below keep their own clicks.
                            let click = ui.interact(
                                head,
                                Id::new(("card-click", card.id)),
                                egui::Sense::click(),
                            );
                            if click.clicked() {
                                out.push(Intent::Select(Some(card.id)));
                            }
                            rects.push((card.id, response.rect));
                        }
                    });
                    if let Some(task) = dropped {
                        let y = ui
                            .ctx()
                            .pointer_interact_pos()
                            .map_or(f32::INFINITY, |p| p.y);
                        let index = rects
                            .iter()
                            .filter(|(id, rect)| *id != *task && rect.center().y < y)
                            .count();
                        let ids: Vec<TaskId> = column.tasks.iter().map(|c| c.id).collect();
                        let from = ws.task(*task).map(|t| t.status);
                        if let Some(from) = from
                            && let Some(params) =
                                move_params(*task, from, column.status, &ids, index)
                        {
                            let open = open_below(ws, *task);
                            if column.status == TaskStatus::Done
                                && from != TaskStatus::Done
                                && open > 0
                            {
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
                });
            }
        });
    });
}

/// The direct subtasks of a task, in their order.
fn children(ws: &Workspace, task: TaskId) -> Vec<&Task> {
    let mut children: Vec<&Task> = ws.tasks().filter(|t| t.parent == Some(task)).collect();
    children.sort_by(|a, b| a.rank.cmp(&b.rank));
    children
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

fn card_ui(
    ui: &mut egui::Ui,
    card: &TaskCard,
    subtasks: &[&Task],
    selected: bool,
    out: &mut Vec<Intent>,
) -> (egui::Rect, Vec<(TaskId, bool, egui::Rect)>) {
    egui::Frame::group(ui.style())
        .show(ui, |ui| {
            ui.set_width(COLUMN_WIDTH - 24.0);
            let title = if card.milestone {
                format!("♦ {}", card.title)
            } else {
                card.title.clone()
            };
            let head = ui.selectable_label(selected, title);
            if head.clicked() {
                out.push(Intent::Select(Some(card.id)));
            }
            let mut head = head.rect;
            let details = ui.horizontal_wrapped(|ui| {
                if let Some(person) = &card.assignee {
                    ui.label(RichText::new(&person.initials).small().weak())
                        .on_hover_text(&person.name);
                }
                due_label(ui, card.due, card.due_time, card.overdue);
                if card.subtasks > 0 {
                    ui.label(
                        RichText::new(format!("☑ {}/{}", card.subtasks_done, card.subtasks))
                            .small()
                            .weak(),
                    );
                }
                if card.priority >= Priority::High {
                    ui.label(
                        RichText::new(text::priority(card.priority))
                            .small()
                            .color(ui.visuals().warn_fg_color),
                    );
                }
            });
            head = head.union(details.response.rect);
            // The subtasks travel with the card; each is ticked off on its own. A mouse click is
            // caught above the drag source (see `show`); keyboard and screen readers act here.
            let mut ticks = Vec::new();
            for subtask in subtasks {
                let done = subtask.status == TaskStatus::Done;
                let mut shown = done;
                let response = ui.checkbox(&mut shown, &subtask.title);
                if response.changed() {
                    out.push(toggle(subtask.id, done));
                }
                ticks.push((subtask.id, done, response.rect));
            }
            (head, ticks)
        })
        .inner
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
