//! Tasks: create, update, move, reparent, assign, set status or progress, delete.

use plnwrk_core::{Entity, EntityKey, EntityKind, ProjectId, Rank, Task, TaskId};

use crate::command::{CommandSpec, Runner, always};
use crate::params::{self as p, ParamKind as K, ParamSpec as P, Params};
use crate::{CommandError, Tx};

pub(crate) const TASK_STATUS: &[&str] = &["todo", "in_progress", "blocked", "review", "done"];
const PRIORITY: &[&str] = &["low", "normal", "high", "critical"];
const CLASSIFICATION: &[&str] = &["open", "vs_nfd"];

pub(crate) fn load(tx: &Tx<'_>, id: TaskId) -> Result<Task, CommandError> {
    tx.ws().task(id).cloned().ok_or(CommandError::NotFound {
        entity: EntityKey::Task(id),
    })
}

/// Ranks of the tasks of a project, sorted, without `except`.
fn ranks(tx: &Tx<'_>, project: ProjectId, except: Option<TaskId>) -> Vec<Rank> {
    let mut ranks: Vec<Rank> = tx
        .ws()
        .tasks_of(project)
        .filter(|t| Some(t.id) != except)
        .map(|t| t.rank.clone())
        .collect();
    ranks.sort();
    ranks
}

fn rank_error(param: &str, error: plnwrk_core::RankError) -> CommandError {
    CommandError::bad(param, error.to_string())
}

/// A neighbour task given by parameter `param`: must exist, be in `project` and not be `me`.
fn neighbour(
    tx: &Tx<'_>,
    params: &Params<'_>,
    param: &str,
    project: ProjectId,
    me: Option<TaskId>,
) -> Result<Option<Task>, CommandError> {
    let Some(id) = params.opt::<TaskId>(param, p::id)? else {
        return Ok(None);
    };
    let task = tx
        .ws()
        .task(id)
        .cloned()
        .ok_or_else(|| CommandError::bad(param, "task does not exist"))?;
    if task.project != project {
        return Err(CommandError::bad(param, "task is in another project"));
    }
    if Some(task.id) == me {
        return Err(CommandError::bad(
            param,
            "a task cannot be its own neighbour",
        ));
    }
    Ok(Some(task))
}

/// The rank for a task placed after `after` and/or before `before` (both optional).
fn place(
    tx: &Tx<'_>,
    project: ProjectId,
    me: Option<TaskId>,
    after: Option<&Task>,
    before: Option<&Task>,
) -> Result<Rank, CommandError> {
    let all = ranks(tx, project, me);
    let low = match (after, before) {
        (Some(a), _) => Some(a.rank.clone()),
        (None, Some(b)) => all.iter().rev().find(|r| **r < b.rank).cloned(),
        (None, None) => all.last().cloned(),
    };
    let high = match (after, before) {
        (_, Some(b)) => Some(b.rank.clone()),
        (Some(a), None) => all.iter().find(|r| **r > a.rank).cloned(),
        (None, None) => None,
    };
    Rank::between(low.as_ref(), high.as_ref()).map_err(|e| rank_error("before", e))
}

fn create(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let project: ProjectId = params.req("project", p::id)?;
    if tx.ws().project(project).is_none() {
        return Err(CommandError::NotFound {
            entity: EntityKey::Project(project),
        });
    }
    let after = neighbour(tx, params, "after", project, None)?;
    let rank = place(tx, project, None, after.as_ref(), None)?;
    let id: TaskId = tx.new_id();
    let task = Task {
        id,
        project,
        parent: params.opt("parent", p::id)?,
        title: params.req("title", p::title)?,
        description: params.opt("description", p::body)?.unwrap_or_default(),
        status: params.opt("status", p::choice)?.unwrap_or_default(),
        priority: params.opt("priority", p::choice)?.unwrap_or_default(),
        assignee: params.opt("assignee", p::id)?,
        start: params.opt("start", p::date)?,
        due: params.opt("due", p::date)?,
        estimate_minutes: params.opt("estimate_minutes", p::minutes)?,
        progress: params.opt("progress", p::percent)?.unwrap_or(0),
        tags: params.opt("tags", p::labels)?.unwrap_or_default(),
        classification: params.opt("classification", p::choice)?,
        rank,
        created: tx.now(),
    };
    tx.put(Entity::Task(task));
    Ok(())
}

fn update(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut task = load(tx, params.req("task", p::id)?)?;
    if let Some(title) = params.patch_required("title", p::title)? {
        task.title = title;
    }
    if params.has("description") {
        task.description = params.opt("description", p::body)?.unwrap_or_default();
    }
    if let Some(status) = params.patch_required("status", p::choice)? {
        task.status = status;
    }
    if let Some(priority) = params.patch_required("priority", p::choice)? {
        task.priority = priority;
    }
    params
        .patch("assignee", p::id)?
        .apply_to(&mut task.assignee);
    params.patch("start", p::date)?.apply_to(&mut task.start);
    params.patch("due", p::date)?.apply_to(&mut task.due);
    params
        .patch("estimate_minutes", p::minutes)?
        .apply_to(&mut task.estimate_minutes);
    if let Some(progress) = params.patch_required("progress", p::percent)? {
        task.progress = progress;
    }
    if params.has("tags") {
        task.tags = params.opt("tags", p::labels)?.unwrap_or_default();
    }
    params
        .patch("classification", p::choice)?
        .apply_to(&mut task.classification);
    tx.put(Entity::Task(task));
    Ok(())
}

fn move_task(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut task = load(tx, params.req("task", p::id)?)?;
    if let Some(status) = params.opt("status", p::choice)? {
        task.status = status;
    }
    let after = neighbour(tx, params, "after", task.project, Some(task.id))?;
    let before = neighbour(tx, params, "before", task.project, Some(task.id))?;
    if let (Some(a), Some(b)) = (&after, &before)
        && a.rank >= b.rank
    {
        return Err(CommandError::bad("before", "must come after `after`"));
    }
    if after.is_some() || before.is_some() {
        task.rank = place(
            tx,
            task.project,
            Some(task.id),
            after.as_ref(),
            before.as_ref(),
        )?;
    }
    tx.put(Entity::Task(task));
    Ok(())
}

fn reparent(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut task = load(tx, params.req("task", p::id)?)?;
    task.parent = params.opt("parent", p::id)?;
    tx.put(Entity::Task(task));
    Ok(())
}

fn assign(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut task = load(tx, params.req("task", p::id)?)?;
    task.assignee = params.opt("assignee", p::id)?;
    tx.put(Entity::Task(task));
    Ok(())
}

fn set_status(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut task = load(tx, params.req("task", p::id)?)?;
    task.status = params.req("status", p::choice)?;
    tx.put(Entity::Task(task));
    Ok(())
}

fn set_progress(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut task = load(tx, params.req("task", p::id)?)?;
    task.progress = params.req("progress", p::percent)?;
    tx.put(Entity::Task(task));
    Ok(())
}

/// The task and all its descendants, parents before children.
pub(crate) fn subtree(tx: &Tx<'_>, root: TaskId) -> Vec<TaskId> {
    let mut out = vec![root];
    let mut index = 0;
    while let Some(&current) = out.get(index) {
        let children: Vec<TaskId> = tx
            .ws()
            .children(current)
            .map(|t| t.id)
            .filter(|id| !out.contains(id))
            .collect();
        out.extend(children);
        index += 1;
    }
    out
}

fn delete(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let root = load(tx, params.req("task", p::id)?)?;
    let doomed = subtree(tx, root.id);
    let comments: Vec<EntityKey> = tx
        .ws()
        .comments()
        .filter(|c| doomed.contains(&c.task))
        .map(|c| EntityKey::Comment(c.id))
        .collect();
    let dependencies: Vec<EntityKey> = tx
        .ws()
        .dependencies()
        .filter(|d| doomed.contains(&d.from) || doomed.contains(&d.to))
        .map(|d| EntityKey::Dependency(d.id))
        .collect();
    for key in comments.into_iter().chain(dependencies) {
        tx.remove(key)?;
    }
    for id in doomed.into_iter().rev() {
        tx.remove(EntityKey::Task(id))?;
    }
    Ok(())
}

const TASK: P = P::required("task", K::Id(EntityKind::Task), "The task");

/// `task.create`
pub const CREATE: CommandSpec = CommandSpec {
    id: "task.create",
    summary: "Adds a task (or subtask) to a project; it goes to the end unless `after` is given.",
    params: &[
        P::required("project", K::Id(EntityKind::Project), "The project or case"),
        P::required("title", K::Title, "Short name"),
        P::optional(
            "parent",
            K::Id(EntityKind::Task),
            "Makes it a subtask of this task",
        ),
        P::optional("description", K::LongText, "Markdown"),
        P::optional("status", K::Choice(TASK_STATUS), "Default: todo"),
        P::optional("priority", K::Choice(PRIORITY), "Default: normal"),
        P::optional("assignee", K::Id(EntityKind::Person), "Who does it"),
        P::optional("start", K::Date, "First day"),
        P::optional("due", K::Date, "Deadline"),
        P::optional(
            "estimate_minutes",
            K::Integer {
                min: 0,
                max: 1_000_000,
            },
            "Expected effort in minutes",
        ),
        P::optional("progress", K::Integer { min: 0, max: 100 }, "Percent done"),
        P::optional("tags", K::Labels, "Free labels"),
        P::optional(
            "classification",
            K::Choice(CLASSIFICATION),
            "Own classification, not lower than the project's",
        ),
        P::optional(
            "after",
            K::Id(EntityKind::Task),
            "Place it right after this task",
        ),
    ],
    enabled: always,
    run: Runner::Edit(create),
};

/// `task.update`
pub const UPDATE: CommandSpec = CommandSpec {
    id: "task.update",
    summary: "Changes fields of a task; null clears optional fields.",
    params: &[
        TASK,
        P::optional("title", K::Title, "Short name"),
        P::clearable("description", K::LongText, "Markdown"),
        P::optional("status", K::Choice(TASK_STATUS), "Where it stands"),
        P::optional("priority", K::Choice(PRIORITY), "How urgent"),
        P::clearable("assignee", K::Id(EntityKind::Person), "Who does it"),
        P::clearable("start", K::Date, "First day"),
        P::clearable("due", K::Date, "Deadline"),
        P::clearable(
            "estimate_minutes",
            K::Integer {
                min: 0,
                max: 1_000_000,
            },
            "Expected effort in minutes",
        ),
        P::optional("progress", K::Integer { min: 0, max: 100 }, "Percent done"),
        P::clearable("tags", K::Labels, "Free labels; replaces all"),
        P::clearable(
            "classification",
            K::Choice(CLASSIFICATION),
            "Own classification",
        ),
    ],
    enabled: always,
    run: Runner::Edit(update),
};

/// `task.move`
pub const MOVE: CommandSpec = CommandSpec {
    id: "task.move",
    summary: "Moves a task on the board: new status and/or a place between neighbours.",
    params: &[
        TASK,
        P::optional(
            "status",
            K::Choice(TASK_STATUS),
            "New status (board column)",
        ),
        P::optional(
            "after",
            K::Id(EntityKind::Task),
            "Place it right after this task",
        ),
        P::optional(
            "before",
            K::Id(EntityKind::Task),
            "Place it right before this task",
        ),
    ],
    enabled: always,
    run: Runner::Edit(move_task),
};

/// `task.reparent`
pub const REPARENT: CommandSpec = CommandSpec {
    id: "task.reparent",
    summary: "Makes a task a subtask of another task, or a top-level task without `parent`.",
    params: &[
        TASK,
        P::clearable(
            "parent",
            K::Id(EntityKind::Task),
            "New parent; omit or null for top level",
        ),
    ],
    enabled: always,
    run: Runner::Edit(reparent),
};

/// `task.assign`
pub const ASSIGN: CommandSpec = CommandSpec {
    id: "task.assign",
    summary: "Gives a task to a person, or to nobody without `assignee`.",
    params: &[
        TASK,
        P::clearable(
            "assignee",
            K::Id(EntityKind::Person),
            "Who does it; omit or null for nobody",
        ),
    ],
    enabled: always,
    run: Runner::Edit(assign),
};

/// `task.set_status`
pub const SET_STATUS: CommandSpec = CommandSpec {
    id: "task.set_status",
    summary: "Sets the status of a task.",
    params: &[
        TASK,
        P::required("status", K::Choice(TASK_STATUS), "New status"),
    ],
    enabled: always,
    run: Runner::Edit(set_status),
};

/// `task.set_progress`
pub const SET_PROGRESS: CommandSpec = CommandSpec {
    id: "task.set_progress",
    summary: "Sets how many percent of a task are done.",
    params: &[
        TASK,
        P::required("progress", K::Integer { min: 0, max: 100 }, "Percent done"),
    ],
    enabled: always,
    run: Runner::Edit(set_progress),
};

/// `task.delete`
pub const DELETE: CommandSpec = CommandSpec {
    id: "task.delete",
    summary: "Deletes a task with its subtasks, their dependencies and comments (one undo step).",
    params: &[TASK],
    enabled: always,
    run: Runner::Edit(delete),
};
