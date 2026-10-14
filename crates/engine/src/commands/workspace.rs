//! Bringing older data up to date. `workspace.upgrade` turns milestones of format version 1
//! (their own entity) into tasks marked `milestone` – one model for both (docs/PLAN.md §9).

use plnwrk_core::{
    Entity, EntityKey, Milestone, MilestoneStatus, Priority, ProjectId, Task, TaskId, TaskStatus,
};

use crate::command::{CommandSpec, Runner};
use crate::commands::task::place;
use crate::params::Params;
use crate::{CommandError, Session, Tx};

/// The task that takes the place of a milestone: same title, its date as deadline, done when
/// reached, placed at the end of `project`.
pub(crate) fn milestone_task(
    tx: &mut Tx<'_>,
    milestone: &Milestone,
    project: ProjectId,
    date: jiff::civil::Date,
    reached: bool,
) -> Result<Task, CommandError> {
    let id: TaskId = tx.new_id();
    let rank = place(tx, project, None, (None, None), "project")?;
    Ok(Task {
        id,
        project,
        parent: None,
        title: milestone.title.clone(),
        description: String::new(),
        status: if reached {
            TaskStatus::Done
        } else {
            TaskStatus::Todo
        },
        priority: Priority::Normal,
        assignee: None,
        start: None,
        due: Some(date),
        due_time: None,
        milestone: true,
        estimate_minutes: None,
        progress: 0,
        tags: Vec::new(),
        classification: None,
        rank,
        created: tx.now(),
    })
}

fn upgrade(tx: &mut Tx<'_>, _params: &Params<'_>) -> Result<(), CommandError> {
    let milestones: Vec<Milestone> = tx.ws().milestones().cloned().collect();
    for milestone in milestones {
        let task = milestone_task(
            tx,
            &milestone,
            milestone.project,
            milestone.date,
            milestone.status == MilestoneStatus::Reached,
        )?;
        tx.insert(Entity::Task(task))?;
        tx.remove(EntityKey::Milestone(milestone.id))?;
    }
    Ok(())
}

fn has_old_milestones(session: &Session) -> Result<(), String> {
    if session.workspace().milestones().next().is_some() {
        Ok(())
    } else {
        Err("there is nothing to upgrade".to_owned())
    }
}

/// `workspace.upgrade`
pub const UPGRADE: CommandSpec = CommandSpec {
    id: "workspace.upgrade",
    summary: "Brings data of older files up to date: milestones become tasks marked as milestone.",
    params: &[],
    enabled: has_old_milestones,
    run: Runner::Edit(upgrade),
};
