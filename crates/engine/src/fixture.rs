//! A small, realistic workspace for demos, documentation and tests: one project, one template
//! ("SKA beantworten") and one case created from it. Built only through commands.

use plnwrk_core::{EntityKey, PersonId, ProjectId, TaskId};
use serde_json::{Value, json};

use crate::{CommandError, Origin, Session};

/// Ids of the main entities of [`sample`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// Head of department.
    pub tobias: PersonId,
    /// Case worker.
    pub anna: PersonId,
    /// The project "Digitalfunk Umrüstung Wachen".
    pub project: ProjectId,
    /// The template "SKA beantworten".
    pub template: ProjectId,
    /// The case "SKA 23/1234 Sirenenwarnung", created from the template.
    pub case: ProjectId,
}

fn run(
    session: &mut Session,
    command: &str,
    params: Value,
) -> Result<Vec<EntityKey>, CommandError> {
    session
        .run(command, &params, Origin::Script)
        .map(|outcome| outcome.created)
}

fn missing(what: &str) -> CommandError {
    CommandError::Conflict {
        reason: format!("sample: {what} was not created"),
    }
}

fn person(session: &mut Session, params: Value) -> Result<PersonId, CommandError> {
    match run(session, "person.create", params)?.first() {
        Some(EntityKey::Person(id)) => Ok(*id),
        _ => Err(missing("person")),
    }
}

fn project(session: &mut Session, command: &str, params: Value) -> Result<ProjectId, CommandError> {
    match run(session, command, params)?.first() {
        Some(EntityKey::Project(id)) => Ok(*id),
        _ => Err(missing("project")),
    }
}

fn task(
    session: &mut Session,
    project: ProjectId,
    title: &str,
    extra: Value,
) -> Result<TaskId, CommandError> {
    let mut params = json!({ "project": project.to_string(), "title": title });
    if let (Some(target), Some(extra)) = (params.as_object_mut(), extra.as_object()) {
        target.extend(extra.clone());
    }
    match run(session, "task.create", params)?.first() {
        Some(EntityKey::Task(id)) => Ok(*id),
        _ => Err(missing("task")),
    }
}

/// Fills an empty session with the sample. Dates are chosen around 2026-10-14.
pub fn sample(session: &mut Session) -> Result<Sample, CommandError> {
    let tobias = person(
        session,
        json!({"name": "Tobias Wildezahn", "role": "Abteilungsleitung"}),
    )?;
    let anna = person(
        session,
        json!({"name": "Anna Berg", "role": "Sachbearbeitung"}),
    )?;
    session.set_actor(Some(tobias));

    // A classic project.
    let project_id = project(
        session,
        "project.create",
        json!({
            "title": "Digitalfunk Umrüstung Wachen", "kind": "Projekt",
            "owner": tobias.to_string(), "start": "2026-09-01", "due": "2026-12-15",
            "description": "Umrüstung aller Wachen auf neue Digitalfunkgeräte."
        }),
    )?;
    let p = project_id.to_string();
    task(
        session,
        project_id,
        "Bestandsaufnahme Geräte",
        json!({"status": "done", "due": "2026-09-15"}),
    )?;
    let tender = task(
        session,
        project_id,
        "Ausschreibung vorbereiten",
        json!({"status": "in_progress", "priority": "high", "assignee": anna.to_string(),
               "due": "2026-10-10", "progress": 60, "tags": ["Vergabe"]}),
    )?;
    task(
        session,
        project_id,
        "Leistungsverzeichnis",
        json!({"parent": tender.to_string(), "status": "done"}),
    )?;
    task(
        session,
        project_id,
        "Vergabevermerk",
        json!({"parent": tender.to_string(), "assignee": anna.to_string()}),
    )?;
    let install = task(
        session,
        project_id,
        "Einbau Wache 11",
        json!({"start": "2026-11-02", "due": "2026-11-20", "estimate_minutes": 2400}),
    )?;
    let training = task(
        session,
        project_id,
        "Schulung Einsatzkräfte",
        json!({"due": "2026-12-10"}),
    )?;
    run(
        session,
        "dependency.add",
        json!({"from": tender.to_string(), "to": install.to_string()}),
    )?;
    run(
        session,
        "dependency.add",
        json!({"from": install.to_string(), "to": training.to_string(), "lag_days": 3}),
    )?;
    run(
        session,
        "milestone.create",
        json!({"project": p, "title": "Zuschlag erteilt", "date": "2026-10-12"}),
    )?;
    run(
        session,
        "milestone.create",
        json!({"project": p, "title": "Inbetriebnahme", "date": "2026-12-15"}),
    )?;
    run(
        session,
        "comment.add",
        json!({"task": tender.to_string(), "body": "Rückfrage der Vergabestelle zu Los 2 ist beantwortet."}),
    )?;

    // A template for a recurring case.
    let template = project(
        session,
        "project.create",
        json!({
            "title": "SKA beantworten", "kind": "SKA", "template": true,
            "start": "2026-01-01", "due": "2026-01-08",
            "description": "Ablauf für Schriftliche Kleine Anfragen der Bürgerschaft."
        }),
    )?;
    task(
        session,
        template,
        "Eingang erfassen und Federführung klären",
        json!({"due": "2026-01-01"}),
    )?;
    let input = task(
        session,
        template,
        "Zuarbeiten anfordern",
        json!({"due": "2026-01-05"}),
    )?;
    task(
        session,
        template,
        "Fachamt",
        json!({"parent": input.to_string(), "due": "2026-01-04"}),
    )?;
    let draft = task(
        session,
        template,
        "Antwort entwerfen",
        json!({"due": "2026-01-07", "priority": "high"}),
    )?;
    run(
        session,
        "dependency.add",
        json!({"from": input.to_string(), "to": draft.to_string()}),
    )?;
    run(
        session,
        "milestone.create",
        json!({"project": template.to_string(), "title": "Abgabe Senatskanzlei", "date": "2026-01-08"}),
    )?;

    // A case created from the template.
    let case = project(
        session,
        "project.instantiate",
        json!({
            "template": template.to_string(), "title": "SKA 23/1234 Sirenenwarnung",
            "start": "2026-10-12", "reference": "23/1234", "owner": anna.to_string()
        }),
    )?;
    let first = session
        .workspace()
        .tasks_of(case)
        .min_by(|a, b| a.rank.cmp(&b.rank))
        .map(|t| t.id)
        .ok_or_else(|| missing("case task"))?;
    run(
        session,
        "task.set_status",
        json!({"task": first.to_string(), "status": "done"}),
    )?;

    Ok(Sample {
        tobias,
        anna,
        project: project_id,
        template,
        case,
    })
}
