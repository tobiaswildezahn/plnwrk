//! Dependencies between tasks: add, remove.

use plnwrk_core::{Dependency, DependencyId, Entity, EntityKey, EntityKind};

use crate::command::{CommandSpec, Runner, always};
use crate::params::{self as p, ParamKind as K, ParamSpec as P, Params};
use crate::{CommandError, Tx};

const KIND: &[&str] = &["finish_to_start", "start_to_start", "finish_to_finish"];

fn add(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let id: DependencyId = tx.new_id();
    tx.put(Entity::Dependency(Dependency {
        id,
        from: params.req("from", p::id)?,
        to: params.req("to", p::id)?,
        kind: params.opt("kind", p::choice)?.unwrap_or_default(),
        lag_days: params.opt("lag_days", p::lag)?.unwrap_or(0),
    }));
    Ok(())
}

fn remove(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let id: DependencyId = params.req("dependency", p::id)?;
    tx.remove(EntityKey::Dependency(id))?;
    Ok(())
}

/// `dependency.add`
pub const ADD: CommandSpec = CommandSpec {
    id: "dependency.add",
    summary: "Makes task `to` wait for task `from` (same project, no circles).",
    params: &[
        P::required("from", K::Id(EntityKind::Task), "The task that comes first"),
        P::required("to", K::Id(EntityKind::Task), "The task that waits"),
        P::optional("kind", K::Choice(KIND), "Default: finish_to_start"),
        P::optional(
            "lag_days",
            K::Integer {
                min: -365,
                max: 365,
            },
            "Extra days in between",
        ),
    ],
    enabled: always,
    run: Runner::Edit(add),
};

/// `dependency.remove`
pub const REMOVE: CommandSpec = CommandSpec {
    id: "dependency.remove",
    summary: "Removes a dependency between two tasks.",
    params: &[P::required(
        "dependency",
        K::Id(EntityKind::Dependency),
        "The dependency",
    )],
    enabled: always,
    run: Runner::Edit(remove),
};
