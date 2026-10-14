//! Projects and cases (one model): `project.create`, `project.update`, `project.archive`,
//! `project.instantiate`.

use std::collections::BTreeMap;

use jiff::civil::Date;
use plnwrk_core::{
    Classification, Dependency, Entity, EntityKey, EntityKind, Milestone, Project, ProjectId,
    ProjectStatus, Task, TaskId, TaskStatus,
};

use crate::command::{CommandSpec, Runner, always};
use crate::commands::workspace::milestone_task;
use crate::params::{self as p, ParamKind as K, ParamSpec as P, Params};
use crate::{CommandError, Tx};

pub(crate) const PROJECT_STATUS: &[&str] = &ProjectStatus::NAMES;
pub(crate) const CLASSIFICATION: &[&str] = &Classification::NAMES;

pub(crate) fn load(tx: &Tx<'_>, id: ProjectId) -> Result<Project, CommandError> {
    tx.ws().project(id).cloned().ok_or(CommandError::NotFound {
        entity: EntityKey::Project(id),
    })
}

fn create(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let id: ProjectId = tx.new_id();
    let due = params.opt("due", p::deadline)?;
    let project = Project {
        id,
        title: params.req("title", p::title)?,
        description: params.opt("description", p::body)?.unwrap_or_default(),
        reference: params.opt("reference", p::label)?,
        kind: params.opt("kind", p::label)?,
        owner: params.opt("owner", p::id)?,
        start: params.opt("start", p::date)?,
        due: due.map(|(day, _)| day),
        due_time: due.and_then(|(_, time)| time),
        status: params.opt("status", p::choice)?.unwrap_or_default(),
        template: params.opt("template", p::boolean)?.unwrap_or(false),
        classification: params.opt("classification", p::choice)?.unwrap_or_default(),
        created: tx.now(),
    };
    tx.insert(Entity::Project(project))
}

fn update(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut project = load(tx, params.req("project", p::id)?)?;
    if let Some(title) = params.patch_required("title", p::title)? {
        project.title = title;
    }
    if let Some(description) = params.opt("description", p::body)? {
        project.description = description;
    } else if params.has("description") {
        project.description.clear();
    }
    params
        .patch("reference", p::label)?
        .apply_to(&mut project.reference);
    params.patch("kind", p::label)?.apply_to(&mut project.kind);
    params.patch("owner", p::id)?.apply_to(&mut project.owner);
    params.patch("start", p::date)?.apply_to(&mut project.start);
    p::apply_deadline(
        params.patch("due", p::deadline)?,
        &mut project.due,
        &mut project.due_time,
    );
    if let Some(status) = params.patch_required("status", p::choice)? {
        project.status = status;
    }
    if let Some(template) = params.patch_required("template", p::boolean)? {
        project.template = template;
    }
    if let Some(classification) = params.patch_required("classification", p::choice)? {
        project.classification = classification;
    }
    tx.put(Entity::Project(project));
    Ok(())
}

fn archive(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let mut project = load(tx, params.req("project", p::id)?)?;
    project.status = ProjectStatus::Archived;
    tx.put(Entity::Project(project));
    Ok(())
}

/// Moves `date` by the distance between two anchors.
fn shift(date: Date, from: Date, to: Date) -> Result<Date, CommandError> {
    from.until(to)
        .ok()
        .and_then(|span| {
            date.checked_add(jiff::Span::new().days(span.get_days()))
                .ok()
        })
        .ok_or_else(|| {
            CommandError::bad(
                "start",
                format!("moving the template to this start puts {date} out of the calendar"),
            )
        })
}

fn instantiate(tx: &mut Tx<'_>, params: &Params<'_>) -> Result<(), CommandError> {
    let template = load(tx, params.req("template", p::id)?)?;
    if !template.template {
        return Err(CommandError::bad("template", "is not a template"));
    }
    let start: Date = params.req("start", p::date)?;
    // Every date moves along with the start. Without a start in the template there is nothing
    // to measure the shift from, so dated templates need one.
    if template.start.is_none() {
        let dated = tx
            .ws()
            .tasks_of(template.id)
            .any(|t| t.start.is_some() || t.due.is_some())
            || tx.ws().milestones().any(|m| m.project == template.id)
            || template.due.is_some();
        if dated {
            return Err(CommandError::bad(
                "template",
                "has dates but no start date; set the template's start first",
            ));
        }
    }
    let moved = |date: Option<Date>| -> Result<Option<Date>, CommandError> {
        match (date, template.start) {
            (Some(date), Some(anchor)) => shift(date, anchor, start).map(Some),
            _ => Ok(None),
        }
    };
    let id: ProjectId = tx.new_id();
    let due = params.opt("due", p::deadline)?;
    let project = Project {
        id,
        title: params.req("title", p::title)?,
        description: template.description.clone(),
        reference: params.opt("reference", p::label)?,
        kind: template.kind.clone(),
        owner: params.opt("owner", p::id)?.or(template.owner),
        start: Some(start),
        due: match due {
            Some((day, _)) => Some(day),
            None => moved(template.due)?,
        },
        due_time: match due {
            Some((_, time)) => time,
            None => template.due_time,
        },
        status: ProjectStatus::Active,
        template: false,
        classification: template.classification,
        created: tx.now(),
    };
    tx.insert(Entity::Project(project))?;

    let source: Vec<Task> = tx.ws().tasks_of(template.id).cloned().collect();
    let mut new_ids: BTreeMap<TaskId, TaskId> = BTreeMap::new();
    for task in &source {
        new_ids.insert(task.id, tx.new_id());
    }
    let now = tx.now();
    for task in source {
        let Some(&new) = new_ids.get(&task.id) else {
            continue;
        };
        let start = moved(task.start)?;
        let due = moved(task.due)?;
        tx.insert(Entity::Task(Task {
            id: new,
            project: id,
            parent: task.parent.and_then(|old| new_ids.get(&old).copied()),
            status: TaskStatus::Todo,
            progress: 0,
            start,
            due,
            created: now,
            ..task
        }))?;
    }
    let milestones: Vec<Milestone> = tx
        .ws()
        .milestones()
        .filter(|m| m.project == template.id)
        .cloned()
        .collect();
    // Templates from format version 1 may still hold milestones of their own; the new
    // project gets milestone tasks instead.
    for milestone in milestones {
        let date = moved(Some(milestone.date))?.unwrap_or(start);
        let task = milestone_task(tx, &milestone, id, date, false)?;
        tx.insert(Entity::Task(task))?;
    }
    let dependencies: Vec<Dependency> = tx
        .ws()
        .dependencies()
        .filter(|d| new_ids.contains_key(&d.from))
        .cloned()
        .collect();
    for dependency in dependencies {
        if let (Some(&from), Some(&to)) =
            (new_ids.get(&dependency.from), new_ids.get(&dependency.to))
        {
            let new = tx.new_id();
            tx.insert(Entity::Dependency(Dependency {
                id: new,
                from,
                to,
                ..dependency
            }))?;
        }
    }
    Ok(())
}

/// `project.create`
pub const CREATE: CommandSpec = CommandSpec {
    id: "project.create",
    summary: "Creates a project or a case (a case is a project with reference and deadline).",
    params: &[
        P::required("title", K::Title, "Name"),
        P::optional("description", K::LongText, "Markdown"),
        P::optional(
            "reference",
            K::Label,
            "File or reference number (Aktenzeichen)",
        ),
        P::optional(
            "kind",
            K::Label,
            "Free label, e.g. Projekt, SKA, Beschaffung",
        ),
        P::optional("owner", K::Id(EntityKind::Person), "Who is responsible"),
        P::optional("start", K::Date, "First day"),
        P::optional(
            "due",
            K::Deadline,
            "Deadline, optionally with a time of day",
        ),
        P::optional("status", K::Choice(PROJECT_STATUS), "Default: active"),
        P::optional(
            "template",
            K::Bool,
            "Use as a template for new projects or cases",
        ),
        P::optional("classification", K::Choice(CLASSIFICATION), "Default: open"),
    ],
    enabled: always,
    run: Runner::Edit(create),
};

/// `project.update`
pub const UPDATE: CommandSpec = CommandSpec {
    id: "project.update",
    summary: "Changes fields of a project or case; null clears optional fields.",
    params: &[
        P::required("project", K::Id(EntityKind::Project), "The project or case"),
        P::optional("title", K::Title, "Name"),
        P::clearable("description", K::LongText, "Markdown"),
        P::clearable("reference", K::Label, "File or reference number"),
        P::clearable("kind", K::Label, "Free label"),
        P::clearable("owner", K::Id(EntityKind::Person), "Who is responsible"),
        P::clearable("start", K::Date, "First day"),
        P::clearable(
            "due",
            K::Deadline,
            "Deadline, optionally with a time of day",
        ),
        P::optional("status", K::Choice(PROJECT_STATUS), "Lifecycle"),
        P::optional("template", K::Bool, "Use as a template"),
        P::optional(
            "classification",
            K::Choice(CLASSIFICATION),
            "Security classification",
        ),
    ],
    enabled: always,
    run: Runner::Edit(update),
};

/// `project.archive`
pub const ARCHIVE: CommandSpec = CommandSpec {
    id: "project.archive",
    summary: "Archives a project or case; it disappears from overviews but is kept.",
    params: &[P::required(
        "project",
        K::Id(EntityKind::Project),
        "The project or case",
    )],
    enabled: always,
    run: Runner::Edit(archive),
};

/// `project.instantiate`
pub const INSTANTIATE: CommandSpec = CommandSpec {
    id: "project.instantiate",
    summary: "Creates a new project or case from a template, shifting all dates to the new start.",
    params: &[
        P::required("template", K::Id(EntityKind::Project), "The template"),
        P::required("title", K::Title, "Name of the new project or case"),
        P::required("start", K::Date, "First day; template dates move along"),
        P::optional("reference", K::Label, "File or reference number"),
        P::optional(
            "due",
            K::Deadline,
            "Deadline, optionally with a time of day; default: the template's, moved",
        ),
        P::optional("owner", K::Id(EntityKind::Person), "Who is responsible"),
    ],
    enabled: always,
    run: Runner::Edit(instantiate),
};
