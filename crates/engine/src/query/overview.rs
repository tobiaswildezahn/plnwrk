//! Progress, overdue items and project cards.

use jiff::civil::Date;
use plnwrk_core::{
    Classification, EntityKey, Milestone, MilestoneId, MilestoneStatus, PersonId, Project,
    ProjectId, ProjectStatus, Task, TaskStatus, Workspace,
};
use serde::Serialize;

use super::tasks::TaskIndex;

/// A person as shown on cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersonRef {
    /// Identity.
    pub id: PersonId,
    /// Full name.
    pub name: String,
    /// Short form.
    pub initials: String,
}

pub(crate) fn person_ref(ws: &Workspace, id: Option<PersonId>) -> Option<PersonRef> {
    let person = ws.person(id?)?;
    Some(PersonRef {
        id: person.id,
        name: person.name.clone(),
        initials: person.initials.clone(),
    })
}

/// How far a project is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// Tasks without subtasks (the ones that carry the work).
    pub tasks: usize,
    /// Of those, done.
    pub done: usize,
    /// Mean progress of those, 0–100 (done counts as 100).
    pub percent: u8,
}

/// Progress of a task as it counts: done is always 100 %.
pub fn effective_progress(task: &Task) -> u8 {
    if task.status == TaskStatus::Done {
        100
    } else {
        task.progress.min(100)
    }
}

/// Progress of a project over its leaf tasks; `None` if the project does not exist.
pub fn progress(ws: &Workspace, project: ProjectId) -> Option<Progress> {
    progress_with(ws, &TaskIndex::new(ws), project)
}

pub(crate) fn progress_with(
    ws: &Workspace,
    index: &TaskIndex<'_>,
    project: ProjectId,
) -> Option<Progress> {
    ws.project(project)?;
    let leaves: Vec<&Task> = ws
        .tasks_of(project)
        .filter(|t| index.is_leaf(t.id))
        .collect();
    let done = leaves
        .iter()
        .filter(|t| t.status == TaskStatus::Done)
        .count();
    let sum: usize = leaves
        .iter()
        .map(|t| usize::from(effective_progress(t)))
        .sum();
    let percent = if leaves.is_empty() {
        0
    } else {
        u8::try_from((sum + leaves.len() / 2) / leaves.len()).unwrap_or(100)
    };
    Some(Progress {
        tasks: leaves.len(),
        done,
        percent,
    })
}

/// State of a milestone on a given day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneState {
    /// Ahead.
    Open,
    /// Done.
    Reached,
    /// Date passed without being reached.
    Missed,
}

/// Open milestones whose day has passed are missed.
pub fn milestone_state(milestone: &Milestone, today: Date) -> MilestoneState {
    match milestone.status {
        MilestoneStatus::Reached => MilestoneState::Reached,
        MilestoneStatus::Open if milestone.date < today => MilestoneState::Missed,
        MilestoneStatus::Open => MilestoneState::Open,
    }
}

/// A milestone as shown on cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MilestoneRef {
    /// Identity.
    pub id: MilestoneId,
    /// Name.
    pub title: String,
    /// The day.
    pub date: Date,
    /// Open, reached or missed.
    pub state: MilestoneState,
}

fn is_live(project: &Project) -> bool {
    !project.template && project.status != ProjectStatus::Archived
}

/// Days from `from` to `to` (negative if `to` is earlier).
pub(crate) fn days_between(from: Date, to: Date) -> i64 {
    from.until(to).map_or(0, |span| i64::from(span.get_days()))
}

/// Something past its deadline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverdueItem {
    /// What it is.
    pub entity: EntityKey,
    /// The project it belongs to.
    pub project: ProjectId,
    /// Its name.
    pub title: String,
    /// The deadline that passed.
    pub due: Date,
    /// Days since the deadline.
    pub days_late: i64,
}

/// Every unfinished task, project and milestone past its date, latest first. Templates,
/// archived and finished projects (with their tasks and milestones) are left out.
pub fn overdue(ws: &Workspace, today: Date) -> Vec<OverdueItem> {
    let mut items = Vec::new();
    for project in ws
        .projects()
        .filter(|p| is_live(p) && p.status != ProjectStatus::Done)
    {
        if let Some(due) = project.due
            && due < today
        {
            items.push(OverdueItem {
                entity: EntityKey::Project(project.id),
                project: project.id,
                title: project.title.clone(),
                due,
                days_late: days_between(due, today),
            });
        }
        for task in ws.tasks_of(project.id) {
            if let Some(due) = task.due
                && due < today
                && task.status != TaskStatus::Done
            {
                items.push(OverdueItem {
                    entity: EntityKey::Task(task.id),
                    project: project.id,
                    title: task.title.clone(),
                    due,
                    days_late: days_between(due, today),
                });
            }
        }
        for milestone in ws.milestones().filter(|m| m.project == project.id) {
            if milestone_state(milestone, today) == MilestoneState::Missed {
                items.push(OverdueItem {
                    entity: EntityKey::Milestone(milestone.id),
                    project: project.id,
                    title: milestone.title.clone(),
                    due: milestone.date,
                    days_late: days_between(milestone.date, today),
                });
            }
        }
    }
    items.sort_by(|a, b| {
        b.days_late
            .cmp(&a.days_late)
            .then_with(|| a.title.cmp(&b.title))
    });
    items
}

/// A project or case as shown in the overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectCard {
    /// Identity.
    pub id: ProjectId,
    /// Name.
    pub title: String,
    /// Free label (e.g. SKA).
    pub kind: Option<String>,
    /// Reference number.
    pub reference: Option<String>,
    /// Lifecycle.
    pub status: ProjectStatus,
    /// Who is responsible.
    pub owner: Option<PersonRef>,
    /// Deadline.
    pub due: Option<Date>,
    /// Days until the deadline (negative: late).
    pub days_left: Option<i64>,
    /// How far it is.
    pub progress: Progress,
    /// Tasks not done.
    pub open_tasks: usize,
    /// Tasks past their deadline.
    pub overdue_tasks: usize,
    /// The next milestone not reached yet.
    pub next_milestone: Option<MilestoneRef>,
    /// Security classification.
    pub classification: Classification,
}

fn card(ws: &Workspace, index: &TaskIndex<'_>, project: &Project, today: Date) -> ProjectCard {
    let tasks: Vec<&Task> = ws.tasks_of(project.id).collect();
    let next_milestone = ws
        .milestones()
        .filter(|m| m.project == project.id && m.status == MilestoneStatus::Open)
        .min_by_key(|m| m.date)
        .map(|m| MilestoneRef {
            id: m.id,
            title: m.title.clone(),
            date: m.date,
            state: milestone_state(m, today),
        });
    ProjectCard {
        id: project.id,
        title: project.title.clone(),
        kind: project.kind.clone(),
        reference: project.reference.clone(),
        status: project.status,
        owner: person_ref(ws, project.owner),
        due: project.due,
        days_left: project.due.map(|due| days_between(today, due)),
        progress: progress_with(ws, index, project.id).unwrap_or(Progress {
            tasks: 0,
            done: 0,
            percent: 0,
        }),
        open_tasks: tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Done)
            .count(),
        overdue_tasks: tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Done && t.due.is_some_and(|d| d < today))
            .count(),
        next_milestone,
        classification: project.classification,
    }
}

/// All live projects and cases (no templates, nothing archived), soonest deadline first.
pub fn overview(ws: &Workspace, today: Date) -> Vec<ProjectCard> {
    let index = TaskIndex::new(ws);
    let mut cards: Vec<ProjectCard> = ws
        .projects()
        .filter(|p| is_live(p))
        .map(|p| card(ws, &index, p, today))
        .collect();
    cards.sort_by(|a, b| match (a.due, b.due) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.title.cmp(&b.title)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.cmp(&b.title),
    });
    cards
}

/// All templates, by title.
pub fn templates(ws: &Workspace, today: Date) -> Vec<ProjectCard> {
    let index = TaskIndex::new(ws);
    let mut cards: Vec<ProjectCard> = ws
        .projects()
        .filter(|p| p.template)
        .map(|p| card(ws, &index, p, today))
        .collect();
    cards.sort_by(|a, b| a.title.cmp(&b.title));
    cards
}
