//! Progress, overdue items and project cards.

use jiff::civil::{Date, DateTime, Time};
use plnwrk_core::{
    Classification, EntityKey, PersonId, Project, ProjectId, ProjectStatus, Task, TaskId,
    TaskStatus, Workspace,
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

/// Progress of a project over its leaf tasks (milestones mark dates, they are not work);
/// `None` if the project does not exist.
pub fn progress(ws: &Workspace, project: ProjectId) -> Option<Progress> {
    progress_with(ws, &TaskIndex::new(ws), project)
}

pub(crate) fn progress_with(
    ws: &Workspace,
    index: &TaskIndex<'_>,
    project: ProjectId,
) -> Option<Progress> {
    ws.project(project)?;
    let leaves: Vec<&Task> = index
        .tasks_of(project)
        .iter()
        .copied()
        .filter(|t| !t.milestone && index.is_leaf(t.id))
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

/// Whether a deadline has passed at `now` (local time). Without a time of day the whole day
/// counts: overdue from the next day on.
pub fn is_past(due: Date, time: Option<Time>, now: DateTime) -> bool {
    match time {
        Some(time) => due.to_datetime(time) < now,
        None => due < now.date(),
    }
}

/// Order of deadlines: earlier first; on the same day a time comes before "the whole day";
/// without a deadline last.
pub fn deadline_order(due: Option<Date>, time: Option<Time>) -> impl Ord {
    (due.is_none(), due, time.is_none(), time)
}

/// A milestone (a task marked as such) is reached when done, missed when its deadline passed.
pub fn milestone_state(task: &Task, now: DateTime) -> MilestoneState {
    if task.status == TaskStatus::Done {
        MilestoneState::Reached
    } else if task.due.is_some_and(|due| is_past(due, task.due_time, now)) {
        MilestoneState::Missed
    } else {
        MilestoneState::Open
    }
}

/// A milestone as shown on cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MilestoneRef {
    /// Identity: the milestone task.
    pub id: TaskId,
    /// Name.
    pub title: String,
    /// The day, if set.
    pub date: Option<Date>,
    /// The time of day, if set.
    pub time: Option<Time>,
    /// Open, reached or missed.
    pub state: MilestoneState,
}

/// The card of a milestone task in its project.
pub fn milestone_ref(project: &Project, task: &Task, now: DateTime) -> MilestoneRef {
    MilestoneRef {
        id: task.id,
        title: task.title.clone(),
        date: task.due,
        time: task.due_time,
        state: milestone_state_in(project, task, now),
    }
}

fn is_live(project: &Project) -> bool {
    !project.template && project.status != ProjectStatus::Archived
}

/// Whether deadlines of a project and of its tasks and milestones still count: not for
/// templates, finished or archived projects. The one rule behind every "overdue" and "missed".
pub fn deadlines_count(project: &Project) -> bool {
    !project.template
        && !matches!(
            project.status,
            ProjectStatus::Done | ProjectStatus::Archived
        )
}

/// Whether a task is past its deadline and that still counts.
pub fn task_overdue(project: Option<&Project>, task: &Task, now: DateTime) -> bool {
    project.is_some_and(deadlines_count)
        && task.status != TaskStatus::Done
        && task.due.is_some_and(|due| is_past(due, task.due_time, now))
}

/// The state of a milestone inside its project: never "missed" where deadlines do not count.
pub fn milestone_state_in(project: &Project, milestone: &Task, now: DateTime) -> MilestoneState {
    match milestone_state(milestone, now) {
        MilestoneState::Missed if !deadlines_count(project) => MilestoneState::Open,
        state => state,
    }
}

/// Days from `from` to `to` (negative if `to` is earlier).
pub fn days_between(from: Date, to: Date) -> i64 {
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
    /// Its time of day, if set.
    pub due_time: Option<Time>,
    /// Days since the deadline.
    pub days_late: i64,
}

/// Every unfinished task, project and milestone past its date, latest first. Templates,
/// archived and finished projects (with their tasks and milestones) are left out.
pub fn overdue(ws: &Workspace, now: DateTime) -> Vec<OverdueItem> {
    let today = now.date();
    let index = TaskIndex::new(ws);
    let mut items = Vec::new();
    for project in ws.projects().filter(|p| deadlines_count(p)) {
        if let Some(due) = project.due
            && is_past(due, project.due_time, now)
        {
            items.push(OverdueItem {
                entity: EntityKey::Project(project.id),
                project: project.id,
                title: project.title.clone(),
                due,
                due_time: project.due_time,
                days_late: days_between(due, today),
            });
        }
        for task in index.tasks_of(project.id) {
            if let Some(due) = task.due
                && task_overdue(Some(project), task, now)
            {
                items.push(OverdueItem {
                    entity: EntityKey::Task(task.id),
                    project: project.id,
                    title: task.title.clone(),
                    due,
                    due_time: task.due_time,
                    days_late: days_between(due, today),
                });
            }
        }
    }
    // Latest first; on the same day the earlier time has passed longer.
    items.sort_by(|a, b| {
        b.days_late
            .cmp(&a.days_late)
            .then_with(|| {
                deadline_order(Some(a.due), a.due_time)
                    .cmp(&deadline_order(Some(b.due), b.due_time))
            })
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
    /// Its time of day, if set.
    pub due_time: Option<Time>,
    /// Whether its deadlines still count (not for templates, finished or archived projects).
    pub deadlines_count: bool,
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

fn card(ws: &Workspace, index: &TaskIndex<'_>, project: &Project, now: DateTime) -> ProjectCard {
    let tasks = index.tasks_of(project.id);
    let next_milestone = index
        .milestones_of(project.id)
        .iter()
        .filter(|m| m.status != TaskStatus::Done)
        .min_by_key(|m| deadline_order(m.due, m.due_time))
        .map(|m| milestone_ref(project, m, now));
    ProjectCard {
        id: project.id,
        title: project.title.clone(),
        kind: project.kind.clone(),
        reference: project.reference.clone(),
        status: project.status,
        owner: person_ref(ws, project.owner),
        due: project.due,
        due_time: project.due_time,
        deadlines_count: deadlines_count(project),
        days_left: project.due.map(|due| days_between(now.date(), due)),
        progress: progress_with(ws, index, project.id).unwrap_or(Progress {
            tasks: 0,
            done: 0,
            percent: 0,
        }),
        // Milestones are counted on their own (`next_milestone`), not as work.
        open_tasks: tasks
            .iter()
            .filter(|t| !t.milestone && t.status != TaskStatus::Done)
            .count(),
        overdue_tasks: tasks
            .iter()
            .filter(|t| !t.milestone && task_overdue(Some(project), t, now))
            .count(),
        next_milestone,
        classification: project.classification,
    }
}

/// All live projects and cases (no templates, nothing archived), soonest deadline first.
pub fn overview(ws: &Workspace, now: DateTime) -> Vec<ProjectCard> {
    let index = TaskIndex::new(ws);
    let mut cards: Vec<ProjectCard> = ws
        .projects()
        .filter(|p| is_live(p))
        .map(|p| card(ws, &index, p, now))
        .collect();
    cards.sort_by(|a, b| {
        deadline_order(a.due, a.due_time)
            .cmp(&deadline_order(b.due, b.due_time))
            .then_with(|| a.title.cmp(&b.title))
    });
    cards
}

/// All templates, by title.
pub fn templates(ws: &Workspace, now: DateTime) -> Vec<ProjectCard> {
    let index = TaskIndex::new(ws);
    let mut cards: Vec<ProjectCard> = ws
        .projects()
        .filter(|p| p.template)
        .map(|p| card(ws, &index, p, now))
        .collect();
    cards.sort_by(|a, b| a.title.cmp(&b.title));
    cards
}
