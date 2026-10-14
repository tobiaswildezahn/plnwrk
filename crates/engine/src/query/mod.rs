//! Read-only queries. Like commands they are described by [`QuerySpec`]s, so the UI, the command
//! line, MCP and the assistant share one list (docs/PLAN.md §3.3).

mod history;
mod overview;
mod tasks;

use plnwrk_core::{
    Comment, DependencyKind, EntityKey, EntityKind, Project, ProjectId, Task, TaskId,
};
use serde::Serialize;
use serde_json::Value;

pub use history::{Action, FieldChange, HistoryEntry, history};
pub use overview::{
    MilestoneRef, MilestoneState, OverdueItem, PersonRef, Progress, ProjectCard,
    effective_progress, milestone_state, overdue, overview, progress, templates,
};
pub use tasks::{
    Board, Column, TaskCard, TaskGroup, TaskGroupRows, TaskIndex, TaskQuery, TaskSort, board, card,
    tasks,
};

use crate::command::search_words;
use crate::commands::task::TASK_STATUS;
use crate::params::{self as p, ParamKind as K, ParamSpec, ParamSpec as P, Params, input_schema};
use crate::{CommandError, Session};

/// The body of a query.
pub type QueryFn = fn(&Session, &Params<'_>) -> Result<Value, CommandError>;

/// Description and implementation of one query.
#[derive(Debug, Clone, Copy)]
pub struct QuerySpec {
    /// Stable name, e.g. `board`.
    pub name: &'static str,
    /// One sentence in English.
    pub summary: &'static str,
    /// Accepted parameters.
    pub params: &'static [ParamSpec],
    /// What it returns.
    pub run: QueryFn,
}

impl QuerySpec {
    /// JSON schema of the parameters.
    pub fn input_schema(&self) -> Value {
        input_schema(self.params)
    }
}

fn json<T: Serialize>(value: &T) -> Result<Value, CommandError> {
    serde_json::to_value(value).map_err(|e| CommandError::Conflict {
        reason: format!("cannot serialize the result: {e}"),
    })
}

fn not_found(entity: EntityKey) -> CommandError {
    CommandError::NotFound { entity }
}

fn q_overview(s: &Session, _: &Params<'_>) -> Result<Value, CommandError> {
    json(&overview(s.workspace(), s.clock().today()))
}

fn q_templates(s: &Session, _: &Params<'_>) -> Result<Value, CommandError> {
    json(&templates(s.workspace(), s.clock().today()))
}

fn q_board(s: &Session, params: &Params<'_>) -> Result<Value, CommandError> {
    let id: ProjectId = params.req("project", p::id)?;
    json(&board(s.workspace(), id, s.clock().today()).ok_or(not_found(EntityKey::Project(id)))?)
}

fn q_tasks(s: &Session, params: &Params<'_>) -> Result<Value, CommandError> {
    let query = TaskQuery {
        project: params.opt("project", p::id)?,
        status: params.opt("status", p::choice)?.into_iter().collect(),
        assignee: params.opt("assignee", p::id)?,
        tag: params.opt("tag", p::label)?,
        text: params.opt("text", p::label)?,
        include_done: params.opt("include_done", p::boolean)?.unwrap_or(false),
        sort: params.opt("sort", p::choice)?.unwrap_or_default(),
        group: params.opt("group", p::choice)?,
    };
    json(&tasks(s.workspace(), &query, s.clock().today()))
}

fn q_overdue(s: &Session, _: &Params<'_>) -> Result<Value, CommandError> {
    json(&overdue(s.workspace(), s.clock().today()))
}

fn q_progress(s: &Session, params: &Params<'_>) -> Result<Value, CommandError> {
    let id: ProjectId = params.req("project", p::id)?;
    json(&progress(s.workspace(), id).ok_or(not_found(EntityKey::Project(id)))?)
}

fn entity_key(params: &Params<'_>) -> Result<EntityKey, CommandError> {
    let kind: EntityKind = params.req("kind", p::choice)?;
    Ok(match kind {
        EntityKind::Project => EntityKey::Project(params.req("id", p::id)?),
        EntityKind::Task => EntityKey::Task(params.req("id", p::id)?),
        EntityKind::Milestone => EntityKey::Milestone(params.req("id", p::id)?),
        EntityKind::Dependency => EntityKey::Dependency(params.req("id", p::id)?),
        EntityKind::Person => EntityKey::Person(params.req("id", p::id)?),
        EntityKind::Comment => EntityKey::Comment(params.req("id", p::id)?),
    })
}

fn q_history(s: &Session, params: &Params<'_>) -> Result<Value, CommandError> {
    json(&history(s.log(), entity_key(params)?))
}

/// A link to another task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    /// The dependency.
    pub dependency: plnwrk_core::DependencyId,
    /// The other task.
    pub task: TaskId,
    /// Its name.
    pub title: String,
    /// How they are linked.
    pub kind: DependencyKind,
    /// Extra days.
    pub lag_days: i32,
}

/// Everything about one task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskDetail {
    /// All stored fields.
    pub task: Task,
    /// The card (derived values).
    pub card: TaskCard,
    /// Direct subtasks.
    pub subtasks: Vec<TaskCard>,
    /// Tasks this one waits for.
    pub waits_for: Vec<Link>,
    /// Tasks waiting for this one.
    pub blocks: Vec<Link>,
    /// Comments, oldest first.
    pub comments: Vec<Comment>,
    /// Changes, oldest first.
    pub history: Vec<HistoryEntry>,
}

fn q_task(s: &Session, params: &Params<'_>) -> Result<Value, CommandError> {
    let ws = s.workspace();
    let today = s.clock().today();
    let id: TaskId = params.req("task", p::id)?;
    let task = ws.task(id).ok_or(not_found(EntityKey::Task(id)))?;
    let index = TaskIndex::new(ws);
    let mut subtasks: Vec<&Task> = index.children(id).to_vec();
    subtasks.sort_by(|a, b| a.rank.cmp(&b.rank));
    let link = |dependency: &plnwrk_core::Dependency, other: TaskId| -> Option<Link> {
        Some(Link {
            dependency: dependency.id,
            task: other,
            title: ws.task(other)?.title.clone(),
            kind: dependency.kind,
            lag_days: dependency.lag_days,
        })
    };
    let mut comments: Vec<Comment> = ws.comments().filter(|c| c.task == id).cloned().collect();
    comments.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.cmp(&b.id)));
    json(&TaskDetail {
        task: task.clone(),
        card: card(ws, &index, task, today),
        subtasks: subtasks
            .into_iter()
            .map(|t| card(ws, &index, t, today))
            .collect(),
        waits_for: ws
            .dependencies()
            .filter(|d| d.to == id)
            .filter_map(|d| link(d, d.from))
            .collect(),
        blocks: ws
            .dependencies()
            .filter(|d| d.from == id)
            .filter_map(|d| link(d, d.to))
            .collect(),
        comments,
        history: history(s.log(), EntityKey::Task(id)),
    })
}

/// Everything about one project or case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectDetail {
    /// All stored fields.
    pub project: Project,
    /// Its milestones by date.
    pub milestones: Vec<MilestoneRef>,
    /// Its top-level tasks in manual order.
    pub tasks: Vec<TaskCard>,
    /// How far it is.
    pub progress: Progress,
}

fn q_project(s: &Session, params: &Params<'_>) -> Result<Value, CommandError> {
    let ws = s.workspace();
    let today = s.clock().today();
    let id: ProjectId = params.req("project", p::id)?;
    let project = ws.project(id).ok_or(not_found(EntityKey::Project(id)))?;
    let mut milestones: Vec<MilestoneRef> = ws
        .milestones()
        .filter(|m| m.project == id)
        .map(|m| MilestoneRef {
            id: m.id,
            title: m.title.clone(),
            date: m.date,
            state: milestone_state(m, today),
        })
        .collect();
    milestones.sort_by(|a, b| a.date.cmp(&b.date).then(a.title.cmp(&b.title)));
    let index = TaskIndex::new(ws);
    let mut top: Vec<&Task> = ws.tasks_of(id).filter(|t| t.parent.is_none()).collect();
    top.sort_by(|a, b| a.rank.cmp(&b.rank));
    json(&ProjectDetail {
        project: project.clone(),
        milestones,
        tasks: top
            .into_iter()
            .map(|t| card(ws, &index, t, today))
            .collect(),
        progress: progress(ws, id).ok_or(not_found(EntityKey::Project(id)))?,
    })
}

const KINDS: &[&str] = &[
    "project",
    "task",
    "milestone",
    "dependency",
    "person",
    "comment",
];
const SORTS: &[&str] = &["rank", "due", "priority", "title", "created"];
const GROUPS: &[&str] = &["status", "assignee", "priority"];
const PROJECT: P = P::required("project", K::Id(EntityKind::Project), "The project or case");

static QUERIES: &[QuerySpec] = &[
    QuerySpec {
        name: "overview",
        summary: "All live projects and cases with progress, overdue tasks and next milestone.",
        params: &[],
        run: q_overview,
    },
    QuerySpec {
        name: "templates",
        summary: "All templates from which projects or cases can be created.",
        params: &[],
        run: q_templates,
    },
    QuerySpec {
        name: "project",
        summary: "Everything about one project or case: fields, milestones, top-level tasks.",
        params: &[PROJECT],
        run: q_project,
    },
    QuerySpec {
        name: "board",
        summary: "The Kanban board of a project: one column per status, cards in manual order.",
        params: &[PROJECT],
        run: q_board,
    },
    QuerySpec {
        name: "tasks",
        summary: "Tasks filtered by project, status, person, tag or text; sorted and grouped.",
        params: &[
            P::optional("project", K::Id(EntityKind::Project), "Only this project"),
            P::optional("status", K::Choice(TASK_STATUS), "Only this status"),
            P::optional(
                "assignee",
                K::Id(EntityKind::Person),
                "Only this person's tasks",
            ),
            P::optional("tag", K::Label, "Only tasks with this tag"),
            P::optional("text", K::Label, "Title contains this text"),
            P::optional(
                "include_done",
                K::Bool,
                "Include done tasks (default false)",
            ),
            P::optional("sort", K::Choice(SORTS), "Default: rank"),
            P::optional("group", K::Choice(GROUPS), "Group the result"),
        ],
        run: q_tasks,
    },
    QuerySpec {
        name: "task",
        summary: "Everything about one task: fields, subtasks, dependencies, comments, history.",
        params: &[P::required("task", K::Id(EntityKind::Task), "The task")],
        run: q_task,
    },
    QuerySpec {
        name: "overdue",
        summary: "Everything past its deadline: tasks, projects and missed milestones, latest first.",
        params: &[],
        run: q_overdue,
    },
    QuerySpec {
        name: "progress",
        summary: "How far a project is: leaf tasks, done, percent.",
        params: &[PROJECT],
        run: q_progress,
    },
    QuerySpec {
        name: "history",
        summary: "Who changed what and when for one entity, read from the log.",
        params: &[
            P::required("kind", K::Choice(KINDS), "Type of the entity"),
            P::required("id", K::Label, "Its id"),
        ],
        run: q_history,
    },
];

/// All queries.
pub fn query_registry() -> &'static [QuerySpec] {
    QUERIES
}

/// A query by name.
pub fn find_query(name: &str) -> Option<&'static QuerySpec> {
    QUERIES.iter().find(|q| q.name == name)
}

/// Queries matching the words of `query`, best first.
pub fn search_queries(query: &str) -> Vec<&'static QuerySpec> {
    let words = search_words(query);
    let mut scored: Vec<(usize, &'static QuerySpec)> = QUERIES
        .iter()
        .filter_map(|q| {
            let haystack = format!("{} {}", q.name, q.summary.to_lowercase());
            let score = words
                .iter()
                .filter(|w| haystack.contains(w.as_str()))
                .count();
            (score > 0).then_some((score, q))
        })
        .collect();
    scored.sort_by(|(a, x), (b, y)| b.cmp(a).then(x.name.cmp(y.name)));
    scored.into_iter().map(|(_, q)| q).collect()
}

impl Session {
    /// Runs a query by name.
    pub fn query(&self, name: &str, params: &Value) -> Result<Value, CommandError> {
        let spec = find_query(name).ok_or_else(|| CommandError::UnknownCommand {
            command: name.chars().take(60).collect(),
        })?;
        let checked = Params::check(params, spec.params)?;
        (spec.run)(self, &checked)
    }
}
