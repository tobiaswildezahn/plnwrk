//! The milestones that come before what they wait for, over all projects whose deadlines count
//! (Phase 9g): the overview block "Überholte Meilensteine" and the MCP query. The warning of
//! [`Plan::analyse`], not built again.

use plnwrk_core::{ProjectId, Workspace};

use crate::{Calendar, Overtaking, Plan};

/// Every overtaken milestone with its project, the soonest first.
pub fn overtaken_milestones(
    workspace: &Workspace,
    calendar: Calendar,
) -> Vec<(ProjectId, Overtaking)> {
    let mut all: Vec<(ProjectId, Overtaking)> = workspace
        .projects()
        .filter(|p| p.deadlines_count())
        .flat_map(|p| {
            Plan::of(workspace, p.id, calendar)
                .overtaking()
                .into_iter()
                .map(move |o| (p.id, o))
        })
        .collect();
    all.sort_by_key(|(_, o)| o.date);
    all
}
