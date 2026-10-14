//! Domain model of plnwrk (layer 0).
//!
//! Pure data and rules: no I/O, no clock, no randomness. See `docs/PLAN.md` §3.2.

pub mod clock;
pub mod ids;

pub use clock::{Clock, FixedClock};
pub use ids::{
    CommentId, DependencyId, IdSource, MilestoneId, PersonId, ProjectId, SequentialIds, TaskId,
};
