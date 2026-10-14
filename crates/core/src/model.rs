//! The entities of plnwrk. A project and a case (Vorgang) are the same thing: a [`Project`] with
//! a reference number and a deadline is a case (docs/PLAN.md §3.2).

use jiff::Timestamp;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::{
    Classification, CommentId, DependencyId, DependencyKind, MilestoneId, MilestoneStatus,
    PersonId, Priority, ProjectId, ProjectStatus, Rank, TaskId, TaskStatus,
};

/// Largest estimate in minutes (about two years of full-time work).
pub const ESTIMATE_MAX: u32 = 1_000_000;
/// Largest lag of a dependency in days, either direction.
pub const LAG_MAX: i32 = 365;

/// A project or a case: a container of tasks with an optional reference number and deadline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Identity.
    pub id: ProjectId,
    /// Name shown everywhere.
    pub title: String,
    /// Markdown.
    pub description: String,
    /// File or reference number (Aktenzeichen), free format.
    pub reference: Option<String>,
    /// Free label such as "Projekt", "SKA" or "Beschaffung"; used for filters and templates.
    pub kind: Option<String>,
    /// Who is responsible.
    pub owner: Option<PersonId>,
    /// First day.
    pub start: Option<Date>,
    /// Deadline (Frist).
    pub due: Option<Date>,
    /// Lifecycle.
    pub status: ProjectStatus,
    /// A template from which new projects or cases are created.
    pub template: bool,
    /// Security classification of the whole project.
    pub classification: Classification,
    /// When it was created.
    pub created: Timestamp,
}

/// A piece of work inside a project. Tasks nest: `parent` makes it a subtask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Identity.
    pub id: TaskId,
    /// The project it belongs to.
    pub project: ProjectId,
    /// The task it is part of, if it is a subtask.
    pub parent: Option<TaskId>,
    /// Short name.
    pub title: String,
    /// Markdown.
    pub description: String,
    /// Where it stands.
    pub status: TaskStatus,
    /// How urgent it is.
    pub priority: Priority,
    /// Who does it.
    pub assignee: Option<PersonId>,
    /// First day.
    pub start: Option<Date>,
    /// Deadline.
    pub due: Option<Date>,
    /// Expected effort in minutes.
    pub estimate_minutes: Option<u32>,
    /// Percent done, 0 to 100.
    pub progress: u8,
    /// Free labels, sorted and without duplicates.
    pub tags: Vec<String>,
    /// Own classification; `None` inherits the project's. Never lower than the project's.
    pub classification: Option<Classification>,
    /// Position among the tasks of the project.
    pub rank: Rank,
    /// When it was created.
    pub created: Timestamp,
}

/// A date that matters in a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Milestone {
    /// Identity.
    pub id: MilestoneId,
    /// The project it belongs to.
    pub project: ProjectId,
    /// Name.
    pub title: String,
    /// The day.
    pub date: Date,
    /// Reached or not. "Missed" is derived from the date.
    pub status: MilestoneStatus,
}

/// `to` waits for `from` (within one project).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// Identity.
    pub id: DependencyId,
    /// The task that comes first.
    pub from: TaskId,
    /// The task that waits.
    pub to: TaskId,
    /// Which ends are linked.
    pub kind: DependencyKind,
    /// Extra days between them (negative: overlap).
    pub lag_days: i32,
}

/// Someone who owns projects, does tasks or writes comments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    /// Identity.
    pub id: PersonId,
    /// Full name.
    pub name: String,
    /// E-mail address.
    pub email: Option<String>,
    /// Role or function, e.g. "Sachbearbeitung".
    pub role: Option<String>,
    /// Short form shown on cards.
    pub initials: String,
}

/// A note on a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Identity.
    pub id: CommentId,
    /// The task it belongs to.
    pub task: TaskId,
    /// Who wrote it.
    pub author: Option<PersonId>,
    /// When it was written.
    pub created: Timestamp,
    /// Markdown, not empty.
    pub body: String,
}

/// Any entity, e.g. as the before or after value of an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entity {
    /// A project or case.
    Project(Project),
    /// A task.
    Task(Task),
    /// A milestone.
    Milestone(Milestone),
    /// A dependency.
    Dependency(Dependency),
    /// A person.
    Person(Person),
    /// A comment.
    Comment(Comment),
}

/// Points at one entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum EntityKey {
    /// A project or case.
    Project(ProjectId),
    /// A task.
    Task(TaskId),
    /// A milestone.
    Milestone(MilestoneId),
    /// A dependency.
    Dependency(DependencyId),
    /// A person.
    Person(PersonId),
    /// A comment.
    Comment(CommentId),
}

/// The type of an entity, without the identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A project or case.
    Project,
    /// A task.
    Task,
    /// A milestone.
    Milestone,
    /// A dependency.
    Dependency,
    /// A person.
    Person,
    /// A comment.
    Comment,
}

impl Entity {
    /// Which entity this is.
    pub fn key(&self) -> EntityKey {
        match self {
            Self::Project(p) => EntityKey::Project(p.id),
            Self::Task(t) => EntityKey::Task(t.id),
            Self::Milestone(m) => EntityKey::Milestone(m.id),
            Self::Dependency(d) => EntityKey::Dependency(d.id),
            Self::Person(p) => EntityKey::Person(p.id),
            Self::Comment(c) => EntityKey::Comment(c.id),
        }
    }
}

impl EntityKey {
    /// The type of the entity.
    pub const fn kind(self) -> EntityKind {
        match self {
            Self::Project(_) => EntityKind::Project,
            Self::Task(_) => EntityKind::Task,
            Self::Milestone(_) => EntityKind::Milestone,
            Self::Dependency(_) => EntityKind::Dependency,
            Self::Person(_) => EntityKind::Person,
            Self::Comment(_) => EntityKind::Comment,
        }
    }
}

impl std::fmt::Display for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project(id) => write!(f, "project {id}"),
            Self::Task(id) => write!(f, "task {id}"),
            Self::Milestone(id) => write!(f, "milestone {id}"),
            Self::Dependency(id) => write!(f, "dependency {id}"),
            Self::Person(id) => write!(f, "person {id}"),
            Self::Comment(id) => write!(f, "comment {id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommentId, DependencyId, MilestoneId, PersonId, ProjectId, TaskId};

    #[test]
    fn keys_know_their_kind_and_read_well() {
        let u = ulid::Ulid(1);
        let keys = [
            (
                EntityKey::Project(ProjectId::from_ulid(u)),
                EntityKind::Project,
                "project",
            ),
            (
                EntityKey::Task(TaskId::from_ulid(u)),
                EntityKind::Task,
                "task",
            ),
            (
                EntityKey::Milestone(MilestoneId::from_ulid(u)),
                EntityKind::Milestone,
                "milestone",
            ),
            (
                EntityKey::Dependency(DependencyId::from_ulid(u)),
                EntityKind::Dependency,
                "dependency",
            ),
            (
                EntityKey::Person(PersonId::from_ulid(u)),
                EntityKind::Person,
                "person",
            ),
            (
                EntityKey::Comment(CommentId::from_ulid(u)),
                EntityKind::Comment,
                "comment",
            ),
        ];
        for (key, kind, word) in keys {
            assert_eq!(key.kind(), kind);
            assert_eq!(
                key.to_string(),
                format!("{word} 00000000000000000000000001")
            );
            let json = serde_json::to_value(key).unwrap();
            assert_eq!(json["kind"], word);
        }
    }
}
