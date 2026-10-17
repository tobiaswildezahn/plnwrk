//! Domain model of plnwrk (layer 0).
//!
//! Pure data and rules: no I/O, no clock, no randomness. See `docs/PLAN.md` §3.2.

pub mod clock;
pub mod ids;
pub mod invariants;
pub mod kinds;
pub mod model;
pub mod policy;
pub mod rank;
pub mod text;
pub mod workspace;

pub use clock::{Clock, FixedClock};
pub use ids::{
    CommentId, DependencyId, IdSource, MilestoneId, PersonId, ProjectId, SequentialIds, TaskId,
};
pub use invariants::{Rule, Violation};
pub use kinds::{
    Classification, DependencyKind, MilestoneStatus, Priority, ProjectStatus, TaskStatus,
};
pub use model::{
    Comment, Dependency, ESTIMATE_MAX, Entity, EntityKey, EntityKind, LAG_MAX, Milestone, Person,
    Project, Task, is_case,
};
pub use policy::{Destination, NetworkPolicy};
pub use rank::{Rank, RankError};
pub use workspace::Workspace;
