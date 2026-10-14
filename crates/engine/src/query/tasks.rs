//! Task lists and the board.

use std::collections::BTreeMap;

use jiff::civil::{Date, DateTime, Time};
use plnwrk_core::{
    Classification, PersonId, Priority, ProjectId, Task, TaskId, TaskStatus, Workspace,
};
use serde::Serialize;

use super::overview::{PersonRef, effective_progress, person_ref, task_overdue};

/// A task as shown in lists and on the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskCard {
    /// Identity.
    pub id: TaskId,
    /// The project.
    pub project: ProjectId,
    /// The parent task, for subtasks.
    pub parent: Option<TaskId>,
    /// Name.
    pub title: String,
    /// Where it stands.
    pub status: TaskStatus,
    /// How urgent.
    pub priority: Priority,
    /// Who does it.
    pub assignee: Option<PersonRef>,
    /// Deadline.
    pub due: Option<Date>,
    /// Its time of day, if set.
    pub due_time: Option<Time>,
    /// A milestone.
    pub milestone: bool,
    /// Past its deadline and not done.
    pub overdue: bool,
    /// Percent done (done counts as 100).
    pub progress: u8,
    /// Labels.
    pub tags: Vec<String>,
    /// Number of direct subtasks.
    pub subtasks: usize,
    /// Of those, done.
    pub subtasks_done: usize,
    /// Classification that applies (own or project's).
    pub classification: Classification,
}

/// Tasks by parent and by project, milestones by project – built once per query so that cards,
/// progress and overviews stay linear in the size of the workspace.
#[derive(Debug, Default)]
pub struct TaskIndex<'a> {
    children: BTreeMap<TaskId, Vec<&'a Task>>,
    by_project: BTreeMap<ProjectId, Vec<&'a Task>>,
    milestones: BTreeMap<ProjectId, Vec<&'a Task>>,
}

impl<'a> TaskIndex<'a> {
    /// Indexes all tasks and milestones of the workspace.
    pub fn new(ws: &'a Workspace) -> Self {
        let mut index = Self::default();
        for task in ws.tasks() {
            if let Some(parent) = task.parent {
                index.children.entry(parent).or_default().push(task);
            }
            index.by_project.entry(task.project).or_default().push(task);
            if task.milestone {
                index.milestones.entry(task.project).or_default().push(task);
            }
        }
        index
    }

    /// The tasks of a project.
    pub fn tasks_of(&self, project: ProjectId) -> &[&'a Task] {
        self.by_project.get(&project).map_or(&[], Vec::as_slice)
    }

    /// The milestone tasks of a project.
    pub fn milestones_of(&self, project: ProjectId) -> &[&'a Task] {
        self.milestones.get(&project).map_or(&[], Vec::as_slice)
    }

    /// The direct subtasks of a task.
    pub fn children(&self, id: TaskId) -> &[&'a Task] {
        self.children.get(&id).map_or(&[], Vec::as_slice)
    }

    /// Whether a task has no subtasks.
    pub fn is_leaf(&self, id: TaskId) -> bool {
        self.children(id).is_empty()
    }
}

/// Builds the card of a task.
pub fn card(ws: &Workspace, index: &TaskIndex<'_>, task: &Task, now: DateTime) -> TaskCard {
    let children = index.children(task.id);
    TaskCard {
        id: task.id,
        project: task.project,
        parent: task.parent,
        title: task.title.clone(),
        status: task.status,
        priority: task.priority,
        assignee: person_ref(ws, task.assignee),
        due: task.due,
        due_time: task.due_time,
        milestone: task.milestone,
        overdue: task_overdue(ws.project(task.project), task, now),
        progress: effective_progress(task),
        tags: task.tags.clone(),
        subtasks: children.len(),
        subtasks_done: children
            .iter()
            .filter(|c| c.status == TaskStatus::Done)
            .count(),
        classification: ws.effective_classification(task),
    }
}

/// Order of a task list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSort {
    /// Manual order (as on the board).
    #[default]
    Rank,
    /// Soonest deadline first; without deadline last.
    Due,
    /// Most urgent first.
    Priority,
    /// Alphabetical.
    Title,
    /// Oldest first.
    Created,
}

/// Grouping of a task list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskGroup {
    /// One group per status, in board order.
    Status,
    /// One group per person; unassigned last.
    Assignee,
    /// One group per priority, most urgent first.
    Priority,
}

/// Filter, order and grouping for [`tasks`]. Without `project`, tasks of templates and of
/// archived projects are left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskQuery {
    /// Only this project.
    pub project: Option<ProjectId>,
    /// Only these statuses (empty: all).
    pub status: Vec<TaskStatus>,
    /// Only tasks of this person.
    pub assignee: Option<PersonId>,
    /// Only tasks with this tag.
    pub tag: Option<String>,
    /// Only tasks whose title contains this text (case-insensitive).
    pub text: Option<String>,
    /// Include done tasks.
    pub include_done: bool,
    /// Order inside each group.
    pub sort: TaskSort,
    /// Grouping; `None` gives one group named `all`.
    pub group: Option<TaskGroup>,
}

/// A named group of task cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskGroupRows {
    /// Group name: a status, a priority, a person's name, `unassigned` or `all`.
    pub group: String,
    /// The cards.
    pub tasks: Vec<TaskCard>,
}

fn matches(ws: &Workspace, task: &Task, query: &TaskQuery) -> bool {
    let text = query.text.as_ref().map(|t| t.to_lowercase());
    let in_scope = match query.project {
        Some(project) => task.project == project,
        None => ws
            .project(task.project)
            .is_some_and(|p| !p.template && p.status != plnwrk_core::ProjectStatus::Archived),
    };
    in_scope
        && (query.status.is_empty() || query.status.contains(&task.status))
        && (query.include_done || !query.status.is_empty() || task.status != TaskStatus::Done)
        && query.assignee.is_none_or(|a| task.assignee == Some(a))
        && query.tag.as_ref().is_none_or(|tag| task.tags.contains(tag))
        && text.is_none_or(|t| task.title.to_lowercase().contains(&t))
}

fn sort(tasks: &mut [&Task], order: TaskSort) {
    tasks.sort_by(|a, b| {
        let primary = match order {
            TaskSort::Rank => a.project.cmp(&b.project).then_with(|| a.rank.cmp(&b.rank)),
            TaskSort::Due => match (a.due, b.due) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
            TaskSort::Priority => b.priority.cmp(&a.priority),
            TaskSort::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            TaskSort::Created => a.created.cmp(&b.created),
        };
        primary
            .then_with(|| a.rank.cmp(&b.rank))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Filtered, sorted and grouped tasks.
pub fn tasks(ws: &Workspace, query: &TaskQuery, now: DateTime) -> Vec<TaskGroupRows> {
    let index = TaskIndex::new(ws);
    let mut found: Vec<&Task> = ws.tasks().filter(|t| matches(ws, t, query)).collect();
    sort(&mut found, query.sort);
    let cards = |list: Vec<&Task>| -> Vec<TaskCard> {
        list.into_iter().map(|t| card(ws, &index, t, now)).collect()
    };
    match query.group {
        None => vec![TaskGroupRows {
            group: "all".to_owned(),
            tasks: cards(found),
        }],
        Some(TaskGroup::Status) => TaskStatus::ALL
            .iter()
            .map(|status| TaskGroupRows {
                group: status.name().to_owned(),
                tasks: cards(
                    found
                        .iter()
                        .copied()
                        .filter(|t| t.status == *status)
                        .collect(),
                ),
            })
            .filter(|g| !g.tasks.is_empty())
            .collect(),
        Some(TaskGroup::Priority) => Priority::ALL
            .iter()
            .rev()
            .map(|priority| TaskGroupRows {
                group: priority.name().to_owned(),
                tasks: cards(
                    found
                        .iter()
                        .copied()
                        .filter(|t| t.priority == *priority)
                        .collect(),
                ),
            })
            .filter(|g| !g.tasks.is_empty())
            .collect(),
        Some(TaskGroup::Assignee) => {
            let mut groups: BTreeMap<(bool, String), Vec<&Task>> = BTreeMap::new();
            for task in found {
                let key = match task.assignee.and_then(|id| ws.person(id)) {
                    Some(person) => (false, person.name.clone()),
                    None => (true, "unassigned".to_owned()),
                };
                groups.entry(key).or_default().push(task);
            }
            groups
                .into_iter()
                .map(|((_, name), list)| TaskGroupRows {
                    group: name,
                    tasks: cards(list),
                })
                .collect()
        }
    }
}

/// One column of the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Column {
    /// The status of the column.
    pub status: TaskStatus,
    /// Cards in manual order.
    pub tasks: Vec<TaskCard>,
}

/// The Kanban board of a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Board {
    /// The project.
    pub project: ProjectId,
    /// Its name.
    pub title: String,
    /// One column per status, in order.
    pub columns: Vec<Column>,
}

/// The board of a project (all tasks, subtasks included); `None` if it does not exist.
pub fn board(ws: &Workspace, project: ProjectId, now: DateTime) -> Option<Board> {
    let title = ws.project(project)?.title.clone();
    let index = TaskIndex::new(ws);
    let mut all: Vec<&Task> = index.tasks_of(project).to_vec();
    sort(&mut all, TaskSort::Rank);
    let columns = TaskStatus::ALL
        .iter()
        .map(|status| Column {
            status: *status,
            tasks: all
                .iter()
                .filter(|t| t.status == *status)
                .map(|t| card(ws, &index, t, now))
                .collect(),
        })
        .collect();
    Some(Board {
        project,
        title,
        columns,
    })
}
