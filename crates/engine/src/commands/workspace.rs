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
/// reached, placed at the end of `project`. `id`: the task's id; a new one if `None`.
pub(crate) fn milestone_task(
    tx: &mut Tx<'_>,
    milestone: &Milestone,
    (project, id): (ProjectId, Option<TaskId>),
    date: jiff::civil::Date,
    reached: bool,
) -> Result<Task, CommandError> {
    let id: TaskId = match id {
        Some(id) => id,
        None => tx.new_id(),
    };
    let created = tx
        .ws()
        .project(project)
        .map_or_else(|| tx.now(), |p| p.created);
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
        created,
    })
}

fn upgrade(tx: &mut Tx<'_>, _params: &Params<'_>) -> Result<(), CommandError> {
    let milestones: Vec<Milestone> = tx.ws().milestones().cloned().collect();
    for milestone in milestones {
        // The task keeps the milestone's id (ids are unique across kinds) and the project's
        // creation time, so converting the same file twice gives the same result – reading
        // commands convert a copy and must hand out stable ids.
        let same_id = TaskId::from_ulid(milestone.id.ulid());
        let id = tx.ws().task(same_id).is_none().then_some(same_id);
        let task = milestone_task(
            tx,
            &milestone,
            (milestone.project, id),
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
