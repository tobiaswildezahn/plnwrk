//! Milestones: create, update, reach, delete.

use plnwrk_core::{
    Entity, EntityKey, EntityKind, Milestone, MilestoneId, MilestoneStatus, ProjectId,
};

use crate::command::{CommandSpec, Runner, always};
use crate::params::{self as p, ParamKind as K, ParamSpec as P, Params};
use crate::{CommandError, Tx};

const STATUS: &[&str] = &["open", "reached"];

fn load(tx: &Tx<'_>, id: MilestoneId) -> Result<Milestone, CommandError> {
    tx.ws()
        .milestone(id)
        .cloned()
        .ok_or(CommandError::NotFound {
            entity: EntityKey::Milestone(id),
        })
}

fn create(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let project: ProjectId = params.req("project", p::id)?;
    let id: MilestoneId = tx.new_id();
    tx.put(Entity::Milestone(Milestone {
        id,
        project,
        title: params.req("title", p::title)?,
        date: params.req("date", p::date)?,
        status: MilestoneStatus::Open,
    }));
    Ok(())
}

fn update(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut milestone = load(tx, params.req("milestone", p::id)?)?;
    if let Some(title) = params.patch_required("title", p::title)? {
        milestone.title = title;
    }
    if let Some(date) = params.patch_required("date", p::date)? {
        milestone.date = date;
    }
    if let Some(status) = params.patch_required("status", p::choice)? {
        milestone.status = status;
    }
    tx.put(Entity::Milestone(milestone));
    Ok(())
}

fn reach(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut milestone = load(tx, params.req("milestone", p::id)?)?;
    milestone.status = MilestoneStatus::Reached;
    tx.put(Entity::Milestone(milestone));
    Ok(())
}

fn delete(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let id: MilestoneId = params.req("milestone", p::id)?;
    tx.remove(EntityKey::Milestone(id))?;
    Ok(())
}

const MILESTONE: P = P::required("milestone", K::Id(EntityKind::Milestone), "The milestone");

/// `milestone.create`
pub const CREATE: CommandSpec = CommandSpec {
    id: "milestone.create",
    summary: "Adds a milestone (a date that matters) to a project.",
    params: &[
        P::required("project", K::Id(EntityKind::Project), "The project or case"),
        P::required("title", K::Title, "Name"),
        P::required("date", K::Date, "The day"),
    ],
    enabled: always,
    run: Runner::Edit(create),
};

/// `milestone.update`
pub const UPDATE: CommandSpec = CommandSpec {
    id: "milestone.update",
    summary: "Changes name, date or status of a milestone.",
    params: &[
        MILESTONE,
        P::optional("title", K::Title, "Name"),
        P::optional("date", K::Date, "The day"),
        P::optional("status", K::Choice(STATUS), "open or reached"),
    ],
    enabled: always,
    run: Runner::Edit(update),
};

/// `milestone.reach`
pub const REACH: CommandSpec = CommandSpec {
    id: "milestone.reach",
    summary: "Marks a milestone as reached.",
    params: &[MILESTONE],
    enabled: always,
    run: Runner::Edit(reach),
};

/// `milestone.delete`
pub const DELETE: CommandSpec = CommandSpec {
    id: "milestone.delete",
    summary: "Deletes a milestone.",
    params: &[MILESTONE],
    enabled: always,
    run: Runner::Edit(delete),
};
