//! The Kanban board: one column per status; cards are moved by drag and drop.

use egui::{Id, RichText};
use plnwrk_core::{Priority, Project, TaskId, TaskStatus};
use plnwrk_engine::query::{self, TaskCard};
use serde_json::{Map, Value, json};

use crate::doc::OpenDoc;
use crate::state::State;
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
    let today = doc.session.clock().today();
    let Some(board) = query::board(ws, project.id, today) else {
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
                            let response = ui
                                .dnd_drag_source(Id::new(("card", card.id)), card.id, |ui| {
                                    card_ui(ui, card, state.selected == Some(card.id), out);
                                })
                                .response;
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
                            out.push(Intent::run("task.move", params));
                        }
                    }
                });
            }
        });
    });
}

fn card_ui(ui: &mut egui::Ui, card: &TaskCard, selected: bool, out: &mut Vec<Intent>) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(COLUMN_WIDTH - 24.0);
        if ui.selectable_label(selected, &card.title).clicked() {
            out.push(Intent::Select(Some(card.id)));
        }
        ui.horizontal_wrapped(|ui| {
            if let Some(person) = &card.assignee {
                ui.label(RichText::new(&person.initials).small().weak())
                    .on_hover_text(&person.name);
            }
            due_label(ui, card.due, card.overdue);
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
    });
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
