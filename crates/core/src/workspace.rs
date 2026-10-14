//! A workspace (Arbeitsbereich) holds everything one `.plnwrk` file contains: projects and cases,
//! their tasks, milestones and dependencies, people and comments.
//!
//! Only the engine writes to it ([`Workspace::put`], [`Workspace::remove`]), and only inside a
//! command; everyone else reads.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    Classification, Comment, CommentId, Dependency, DependencyId, Entity, EntityKey, Milestone,
    MilestoneId, Person, PersonId, Project, ProjectId, Task, TaskId,
};

/// Everything in one file, kept in id order so that output is deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    projects: BTreeMap<ProjectId, Project>,
    tasks: BTreeMap<TaskId, Task>,
    milestones: BTreeMap<MilestoneId, Milestone>,
    dependencies: BTreeMap<DependencyId, Dependency>,
    people: BTreeMap<PersonId, Person>,
    comments: BTreeMap<CommentId, Comment>,
}

impl Workspace {
    /// A project or case by id.
    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.get(&id)
    }

    /// A task by id.
    pub fn task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.get(&id)
    }

    /// A milestone by id.
    pub fn milestone(&self, id: MilestoneId) -> Option<&Milestone> {
        self.milestones.get(&id)
    }

    /// A dependency by id.
    pub fn dependency(&self, id: DependencyId) -> Option<&Dependency> {
        self.dependencies.get(&id)
    }

    /// A person by id.
    pub fn person(&self, id: PersonId) -> Option<&Person> {
        self.people.get(&id)
    }

    /// A comment by id.
    pub fn comment(&self, id: CommentId) -> Option<&Comment> {
        self.comments.get(&id)
    }

    /// All projects and cases, templates included.
    pub fn projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.values()
    }

    /// All tasks of all projects.
    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }

    /// All milestones.
    pub fn milestones(&self) -> impl Iterator<Item = &Milestone> {
        self.milestones.values()
    }

    /// All dependencies.
    pub fn dependencies(&self) -> impl Iterator<Item = &Dependency> {
        self.dependencies.values()
    }

    /// All people.
    pub fn people(&self) -> impl Iterator<Item = &Person> {
        self.people.values()
    }

    /// All comments.
    pub fn comments(&self) -> impl Iterator<Item = &Comment> {
        self.comments.values()
    }

    /// The direct subtasks of a task.
    pub fn children(&self, parent: TaskId) -> impl Iterator<Item = &Task> {
        self.tasks
            .values()
            .filter(move |task| task.parent == Some(parent))
    }

    /// The tasks of one project.
    pub fn tasks_of(&self, project: ProjectId) -> impl Iterator<Item = &Task> {
        self.tasks
            .values()
            .filter(move |task| task.project == project)
    }

    /// Entries whose map key differs from their own id (possible only in data read from a
    /// file). Reported by [`Workspace::check`].
    pub fn misfiled(&self) -> Vec<EntityKey> {
        let mut out = Vec::new();
        out.extend(
            self.projects
                .iter()
                .filter(|(k, v)| **k != v.id)
                .map(|(k, _)| EntityKey::Project(*k)),
        );
        out.extend(
            self.tasks
                .iter()
                .filter(|(k, v)| **k != v.id)
                .map(|(k, _)| EntityKey::Task(*k)),
        );
        out.extend(
            self.milestones
                .iter()
                .filter(|(k, v)| **k != v.id)
                .map(|(k, _)| EntityKey::Milestone(*k)),
        );
        out.extend(
            self.dependencies
                .iter()
                .filter(|(k, v)| **k != v.id)
                .map(|(k, _)| EntityKey::Dependency(*k)),
        );
        out.extend(
            self.people
                .iter()
                .filter(|(k, v)| **k != v.id)
                .map(|(k, _)| EntityKey::Person(*k)),
        );
        out.extend(
            self.comments
                .iter()
                .filter(|(k, v)| **k != v.id)
                .map(|(k, _)| EntityKey::Comment(*k)),
        );
        out
    }

    /// The highest classification anywhere in the workspace (projects and tasks).
    pub fn highest_classification(&self) -> Classification {
        let projects = self.projects.values().map(|p| p.classification);
        let tasks = self.tasks.values().filter_map(|t| t.classification);
        projects.chain(tasks).max().unwrap_or_default()
    }

    /// Number of tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// The classification a task gets from above: the highest of its project and all its
    /// ancestor tasks (own level not included).
    pub fn inherited_classification(&self, task: &Task) -> Classification {
        let mut level = self
            .project(task.project)
            .map(|p| p.classification)
            .unwrap_or_default();
        let mut current = task.parent;
        let mut steps = 0usize;
        while let Some(id) = current {
            let Some(parent) = self.task(id) else {
                break;
            };
            if let Some(own) = parent.classification {
                level = level.max(own);
            }
            steps += 1;
            if steps > self.tasks.len() {
                break;
            }
            current = parent.parent;
        }
        level
    }

    /// The classification that applies to a task: the highest of its own, its project's and
    /// its ancestor tasks'.
    pub fn effective_classification(&self, task: &Task) -> Classification {
        let inherited = self.inherited_classification(task);
        task.classification
            .map_or(inherited, |own| own.max(inherited))
    }

    /// Any entity by key (cloned).
    pub fn get(&self, key: EntityKey) -> Option<Entity> {
        match key {
            EntityKey::Project(id) => self.project(id).cloned().map(Entity::Project),
            EntityKey::Task(id) => self.task(id).cloned().map(Entity::Task),
            EntityKey::Milestone(id) => self.milestone(id).cloned().map(Entity::Milestone),
            EntityKey::Dependency(id) => self.dependency(id).cloned().map(Entity::Dependency),
            EntityKey::Person(id) => self.person(id).cloned().map(Entity::Person),
            EntityKey::Comment(id) => self.comment(id).cloned().map(Entity::Comment),
        }
    }

    /// Stores an entity, replacing one with the same key. Returns the replaced one.
    /// Does not check anything: call [`Workspace::check`] afterwards.
    pub fn put(&mut self, entity: Entity) -> Option<Entity> {
        match entity {
            Entity::Project(p) => self.projects.insert(p.id, p).map(Entity::Project),
            Entity::Task(t) => self.tasks.insert(t.id, t).map(Entity::Task),
            Entity::Milestone(m) => self.milestones.insert(m.id, m).map(Entity::Milestone),
            Entity::Dependency(d) => self.dependencies.insert(d.id, d).map(Entity::Dependency),
            Entity::Person(p) => self.people.insert(p.id, p).map(Entity::Person),
            Entity::Comment(c) => self.comments.insert(c.id, c).map(Entity::Comment),
        }
    }

    /// Removes an entity. Returns it, or `None` if there was none.
    /// Does not remove anything that refers to it: call [`Workspace::check`] afterwards.
    pub fn remove(&mut self, key: EntityKey) -> Option<Entity> {
        match key {
            EntityKey::Project(id) => self.projects.remove(&id).map(Entity::Project),
            EntityKey::Task(id) => self.tasks.remove(&id).map(Entity::Task),
            EntityKey::Milestone(id) => self.milestones.remove(&id).map(Entity::Milestone),
            EntityKey::Dependency(id) => self.dependencies.remove(&id).map(Entity::Dependency),
            EntityKey::Person(id) => self.people.remove(&id).map(Entity::Person),
            EntityKey::Comment(id) => self.comments.remove(&id).map(Entity::Comment),
        }
    }

    /// Number of entities of all kinds.
    pub fn len(&self) -> usize {
        self.projects.len()
            + self.tasks.len()
            + self.milestones.len()
            + self.dependencies.len()
            + self.people.len()
            + self.comments.len()
    }

    /// Whether the workspace holds nothing at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
