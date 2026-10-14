//! Scheduling of plnwrk (layer 2): working-day calendar, the plan of a project, moving
//! successors along, critical path and warnings. Pure: dates come in as parameters.
//! See `docs/PLAN.md` §3.5 and `docs/plans/2026-10-14-phase-5.md`.

mod calendar;
mod plan;

pub use calendar::{Calendar, easter};
pub use plan::{Analysis, Item, Link, Move, Plan, Warning};
