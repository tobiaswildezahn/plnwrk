//! Every rule the stored model must satisfy. The engine runs [`Workspace::check`] after each
//! command and rolls back if anything is violated, so no command can leave broken data behind –
//! even one that forgot a check of its own.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::text::{TAGS_MAX, is_body, is_label, is_title};
use crate::{DependencyId, ESTIMATE_MAX, EntityKey, LAG_MAX, PersonId, TaskId, Workspace};

/// Which kind of rule was broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    /// A text is empty, too long, untrimmed or contains control characters.
    Text,
    /// Something refers to an entity that does not exist.
    MissingReference,
    /// A subtask or dependency crosses project borders.
    CrossProject,
    /// A task is its own ancestor, or dependencies go round in a circle.
    Cycle,
    /// A deadline lies before the start.
    DateOrder,
    /// A number is out of its range.
    Range,
    /// A task is classified lower than its project.
    Classification,
    /// The same dependency exists twice.
    Duplicate,
    /// A task depends on itself.
    SelfReference,
}

/// One broken rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Violation {
    /// The entity that breaks the rule.
    pub entity: EntityKey,
    /// The rule.
    pub rule: Rule,
    /// What is wrong, in English.
    pub message: String,
}

struct Report(Vec<Violation>);

impl Report {
    fn add(&mut self, entity: EntityKey, rule: Rule, message: impl Into<String>) {
        self.0.push(Violation {
            entity,
            rule,
            message: message.into(),
        });
    }

    fn title(&mut self, entity: EntityKey, field: &str, text: &str) {
        if !is_title(text) {
            self.add(entity, Rule::Text, format!("{field} is not a valid title"));
        }
    }

    fn label(&mut self, entity: EntityKey, field: &str, text: Option<&str>) {
        if text.is_some_and(|t| !is_label(t)) {
            self.add(entity, Rule::Text, format!("{field} is not a valid label"));
        }
    }

    fn body(&mut self, entity: EntityKey, field: &str, text: &str) {
        if !is_body(text) {
            self.add(entity, Rule::Text, format!("{field} is not a valid text"));
        }
    }

    fn person(&mut self, ws: &Workspace, entity: EntityKey, field: &str, id: Option<PersonId>) {
        if let Some(id) = id
            && ws.person(id).is_none()
        {
            self.add(
                entity,
                Rule::MissingReference,
                format!("{field} {id} does not exist"),
            );
        }
    }

    fn dates(
        &mut self,
        entity: EntityKey,
        start: Option<jiff::civil::Date>,
        due: Option<jiff::civil::Date>,
    ) {
        if let (Some(start), Some(due)) = (start, due)
            && due < start
        {
            self.add(
                entity,
                Rule::DateOrder,
                format!("due {due} is before start {start}"),
            );
        }
    }
}

impl Workspace {
    /// Every broken rule, in a stable order. Empty means the workspace is consistent.
    pub fn check(&self) -> Vec<Violation> {
        let mut report = Report(Vec::new());
        self.check_projects(&mut report);
        self.check_tasks(&mut report);
        self.check_milestones(&mut report);
        self.check_dependencies(&mut report);
        self.check_people(&mut report);
        self.check_comments(&mut report);
        report.0
    }

    fn check_projects(&self, report: &mut Report) {
        for project in self.projects() {
            let key = EntityKey::Project(project.id);
            report.title(key, "title", &project.title);
            report.body(key, "description", &project.description);
            report.label(key, "reference", project.reference.as_deref());
            report.label(key, "kind", project.kind.as_deref());
            report.person(self, key, "owner", project.owner);
            report.dates(key, project.start, project.due);
        }
    }

    fn check_tasks(&self, report: &mut Report) {
        for task in self.tasks() {
            let key = EntityKey::Task(task.id);
            report.title(key, "title", &task.title);
            report.body(key, "description", &task.description);
            let tags_ok = task.tags.len() <= TAGS_MAX
                && task.tags.iter().all(|t| is_label(t))
                && task.tags.windows(2).all(|w| matches!(w, [a, b] if a < b));
            if !tags_ok {
                report.add(
                    key,
                    Rule::Text,
                    "tags must be valid labels, sorted, without duplicates",
                );
            }
            report.person(self, key, "assignee", task.assignee);
            report.dates(key, task.start, task.due);
            if task.progress > 100 {
                report.add(
                    key,
                    Rule::Range,
                    format!("progress {} is above 100", task.progress),
                );
            }
            if task.estimate_minutes.is_some_and(|m| m > ESTIMATE_MAX) {
                report.add(
                    key,
                    Rule::Range,
                    format!("estimate_minutes is above {ESTIMATE_MAX}"),
                );
            }
            if self.project(task.project).is_none() {
                report.add(
                    key,
                    Rule::MissingReference,
                    format!("project {} does not exist", task.project),
                );
                continue;
            }
            if task
                .classification
                .is_some_and(|own| own < self.inherited_classification(task))
            {
                report.add(
                    key,
                    Rule::Classification,
                    "classification is lower than the project's or the parent task's",
                );
            }
            if let Some(parent_id) = task.parent {
                match self.task(parent_id) {
                    None => report.add(
                        key,
                        Rule::MissingReference,
                        format!("parent {parent_id} does not exist"),
                    ),
                    Some(parent) if parent.project != task.project => {
                        report.add(key, Rule::CrossProject, "parent belongs to another project")
                    }
                    Some(_) => {
                        if self.is_own_ancestor(task.id) {
                            report.add(key, Rule::Cycle, "task is its own ancestor");
                        }
                    }
                }
            }
        }
    }

    /// Walks up the parent chain; true if it comes back to `id`.
    fn is_own_ancestor(&self, id: TaskId) -> bool {
        let mut current = self.task(id).and_then(|t| t.parent);
        let mut steps = 0usize;
        let limit = self.task_count();
        while let Some(parent) = current {
            if parent == id {
                return true;
            }
            steps += 1;
            if steps > limit {
                return false;
            }
            current = self.task(parent).and_then(|t| t.parent);
        }
        false
    }

    fn check_milestones(&self, report: &mut Report) {
        for milestone in self.milestones() {
            let key = EntityKey::Milestone(milestone.id);
            report.title(key, "title", &milestone.title);
            if self.project(milestone.project).is_none() {
                report.add(
                    key,
                    Rule::MissingReference,
                    format!("project {} does not exist", milestone.project),
                );
            }
        }
    }

    fn check_dependencies(&self, report: &mut Report) {
        let mut seen: BTreeSet<(TaskId, TaskId)> = BTreeSet::new();
        let mut graph: BTreeMap<TaskId, Vec<(TaskId, DependencyId)>> = BTreeMap::new();
        for dep in self.dependencies() {
            let key = EntityKey::Dependency(dep.id);
            if !(-LAG_MAX..=LAG_MAX).contains(&dep.lag_days) {
                report.add(
                    key,
                    Rule::Range,
                    format!("lag_days must be within ±{LAG_MAX}"),
                );
            }
            if dep.from == dep.to {
                report.add(key, Rule::SelfReference, "a task cannot depend on itself");
                continue;
            }
            let (Some(from), Some(to)) = (self.task(dep.from), self.task(dep.to)) else {
                report.add(
                    key,
                    Rule::MissingReference,
                    "both tasks of a dependency must exist",
                );
                continue;
            };
            if from.project != to.project {
                report.add(
                    key,
                    Rule::CrossProject,
                    "a dependency must stay within one project",
                );
                continue;
            }
            if !seen.insert((dep.from, dep.to)) {
                report.add(
                    key,
                    Rule::Duplicate,
                    "the same two tasks are already linked",
                );
                continue;
            }
            graph.entry(dep.from).or_default().push((dep.to, dep.id));
        }
        for dependency in find_back_edges(&graph) {
            report.add(
                EntityKey::Dependency(dependency),
                Rule::Cycle,
                "dependencies form a circle",
            );
        }
    }

    fn check_people(&self, report: &mut Report) {
        for person in self.people() {
            let key = EntityKey::Person(person.id);
            report.title(key, "name", &person.name);
            report.label(key, "email", person.email.as_deref());
            report.label(key, "role", person.role.as_deref());
            report.label(key, "initials", Some(&person.initials));
        }
    }

    fn check_comments(&self, report: &mut Report) {
        for comment in self.comments() {
            let key = EntityKey::Comment(comment.id);
            if comment.body.trim().is_empty() || !is_body(&comment.body) {
                report.add(key, Rule::Text, "body must be a non-empty valid text");
            }
            if self.task(comment.task).is_none() {
                report.add(
                    key,
                    Rule::MissingReference,
                    format!("task {} does not exist", comment.task),
                );
            }
            report.person(self, key, "author", comment.author);
        }
    }
}

/// Dependencies that close a circle (back edges of an iterative depth-first search).
fn find_back_edges(graph: &BTreeMap<TaskId, Vec<(TaskId, DependencyId)>>) -> Vec<DependencyId> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Active,
        Done,
    }
    let mut marks: BTreeMap<TaskId, Mark> = BTreeMap::new();
    let mut back = Vec::new();
    for &start in graph.keys() {
        if marks.contains_key(&start) {
            continue;
        }
        // Stack of (node, index of the next edge to follow).
        let mut stack: Vec<(TaskId, usize)> = vec![(start, 0)];
        marks.insert(start, Mark::Active);
        while let Some((node, next)) = stack.last().copied() {
            let edge = graph.get(&node).and_then(|edges| edges.get(next)).copied();
            match edge {
                Some((target, dependency)) => {
                    if let Some((_, index)) = stack.last_mut() {
                        *index += 1;
                    }
                    match marks.get(&target) {
                        Some(Mark::Active) => back.push(dependency),
                        Some(Mark::Done) => {}
                        None => {
                            marks.insert(target, Mark::Active);
                            stack.push((target, 0));
                        }
                    }
                }
                None => {
                    marks.insert(node, Mark::Done);
                    stack.pop();
                }
            }
        }
    }
    back
}

#[cfg(test)]
mod tests {
    use crate::*;
    use jiff::civil::date;

    struct Ids {
        person: PersonId,
        project: ProjectId,
        a: TaskId,
        b: TaskId,
        c: TaskId,
        milestone: MilestoneId,
        dependency: DependencyId,
        comment: CommentId,
    }

    fn ts() -> jiff::Timestamp {
        jiff::Timestamp::UNIX_EPOCH
    }

    fn task(id: TaskId, project: ProjectId, title: &str, rank: &str) -> Task {
        Task {
            id,
            project,
            parent: None,
            title: title.into(),
            description: String::new(),
            status: TaskStatus::Todo,
            priority: Priority::Normal,
            assignee: None,
            start: None,
            due: None,
            estimate_minutes: None,
            progress: 0,
            tags: Vec::new(),
            classification: None,
            rank: Rank::parse(rank).unwrap(),
            created: ts(),
        }
    }

    /// A valid workspace: a person, a project with three tasks (b under a), a milestone,
    /// a dependency a→c and a comment on a.
    fn valid() -> (Workspace, Ids) {
        let mut seq = SequentialIds::default();
        let ids = Ids {
            person: seq.next_ulid().into(),
            project: seq.next_ulid().into(),
            a: seq.next_ulid().into(),
            b: seq.next_ulid().into(),
            c: seq.next_ulid().into(),
            milestone: seq.next_ulid().into(),
            dependency: seq.next_ulid().into(),
            comment: seq.next_ulid().into(),
        };
        let mut ws = Workspace::default();
        ws.put(Entity::Person(Person {
            id: ids.person,
            name: "Tobias Wildezahn".into(),
            email: None,
            role: Some("Leitung".into()),
            initials: "TW".into(),
        }));
        ws.put(Entity::Project(Project {
            id: ids.project,
            title: "Übung Nordflanke".into(),
            description: String::new(),
            reference: Some("A 311-2026/14".into()),
            kind: Some("Vorgang".into()),
            owner: Some(ids.person),
            start: Some(date(2026, 10, 1)),
            due: Some(date(2026, 12, 1)),
            status: ProjectStatus::Active,
            template: false,
            classification: Classification::Open,
            created: ts(),
        }));
        ws.put(Entity::Task(task(ids.a, ids.project, "Lage", "a0")));
        let mut b = task(ids.b, ids.project, "Teil", "a1");
        b.parent = Some(ids.a);
        ws.put(Entity::Task(b));
        ws.put(Entity::Task(task(ids.c, ids.project, "Bericht", "a2")));
        ws.put(Entity::Milestone(Milestone {
            id: ids.milestone,
            project: ids.project,
            title: "Abgabe".into(),
            date: date(2026, 11, 15),
            status: MilestoneStatus::Open,
        }));
        ws.put(Entity::Dependency(Dependency {
            id: ids.dependency,
            from: ids.a,
            to: ids.c,
            kind: DependencyKind::FinishToStart,
            lag_days: 0,
        }));
        ws.put(Entity::Comment(Comment {
            id: ids.comment,
            task: ids.a,
            author: Some(ids.person),
            created: ts(),
            body: "Bitte bis Freitag.".into(),
        }));
        (ws, ids)
    }

    fn rules(ws: &Workspace) -> Vec<Rule> {
        ws.check().into_iter().map(|v| v.rule).collect()
    }

    fn edit_task(ws: &mut Workspace, id: TaskId, f: impl FnOnce(&mut Task)) {
        let mut task = ws.task(id).cloned().unwrap();
        f(&mut task);
        ws.put(Entity::Task(task));
    }

    #[test]
    fn valid_workspace_has_no_violations() {
        let (ws, _) = valid();
        assert_eq!(ws.check(), Vec::new());
    }

    #[test]
    fn bad_title_is_reported() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| t.title = "  ".into());
        assert_eq!(rules(&ws), vec![Rule::Text]);
    }

    #[test]
    fn unsorted_or_duplicate_tags_are_reported() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| t.tags = vec!["b".into(), "a".into()]);
        assert_eq!(rules(&ws), vec![Rule::Text]);
    }

    #[test]
    fn task_in_missing_project_is_reported() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| {
            t.project = ProjectId::from_ulid(ulid::Ulid(999));
        });
        assert!(rules(&ws).contains(&Rule::MissingReference));
    }

    #[test]
    fn missing_assignee_owner_author_are_reported() {
        let (mut ws, ids) = valid();
        let ghost = PersonId::from_ulid(ulid::Ulid(999));
        edit_task(&mut ws, ids.c, |t| t.assignee = Some(ghost));
        assert_eq!(rules(&ws), vec![Rule::MissingReference]);
        let (mut ws, ids) = valid();
        ws.remove(EntityKey::Person(ids.person));
        assert_eq!(
            rules(&ws),
            vec![Rule::MissingReference, Rule::MissingReference]
        );
    }

    #[test]
    fn parent_cycle_over_three_levels_is_reported() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| t.parent = Some(ids.b));
        edit_task(&mut ws, ids.a, |t| t.parent = Some(ids.c));
        let found = rules(&ws);
        assert!(!found.is_empty());
        assert!(found.iter().all(|r| *r == Rule::Cycle), "{found:?}");
    }

    #[test]
    fn subtask_in_other_project_is_reported() {
        let (mut ws, ids) = valid();
        let mut other = ws.project(ids.project).cloned().unwrap();
        other.id = ProjectId::from_ulid(ulid::Ulid(500));
        ws.put(Entity::Project(other.clone()));
        edit_task(&mut ws, ids.b, |t| t.project = other.id);
        assert_eq!(rules(&ws), vec![Rule::CrossProject]);
    }

    #[test]
    fn due_before_start_is_reported() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| {
            t.start = Some(date(2026, 10, 10));
            t.due = Some(date(2026, 10, 9));
        });
        assert_eq!(rules(&ws), vec![Rule::DateOrder]);
        let (mut ws, ids) = valid();
        let mut project = ws.project(ids.project).cloned().unwrap();
        project.due = Some(date(2026, 1, 1));
        ws.put(Entity::Project(project));
        assert_eq!(rules(&ws), vec![Rule::DateOrder]);
    }

    #[test]
    fn numbers_out_of_range_are_reported() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| t.progress = 101);
        assert_eq!(rules(&ws), vec![Rule::Range]);
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| {
            t.estimate_minutes = Some(ESTIMATE_MAX + 1)
        });
        assert_eq!(rules(&ws), vec![Rule::Range]);
        let (mut ws, ids) = valid();
        let mut dep = ws.dependency(ids.dependency).cloned().unwrap();
        dep.lag_days = LAG_MAX + 1;
        ws.put(Entity::Dependency(dep));
        assert_eq!(rules(&ws), vec![Rule::Range]);
    }

    #[test]
    fn task_classification_below_project_is_reported() {
        let (mut ws, ids) = valid();
        let mut project = ws.project(ids.project).cloned().unwrap();
        project.classification = Classification::VsNfd;
        ws.put(Entity::Project(project));
        assert!(ws.check().is_empty(), "tasks without own level inherit");
        edit_task(&mut ws, ids.c, |t| {
            t.classification = Some(Classification::Open)
        });
        assert_eq!(rules(&ws), vec![Rule::Classification]);
    }

    #[test]
    fn subtask_classification_below_parent_is_reported() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.a, |t| {
            t.classification = Some(Classification::VsNfd)
        });
        assert!(ws.check().is_empty());
        let b = ws.task(ids.b).cloned().unwrap();
        assert_eq!(ws.effective_classification(&b), Classification::VsNfd);
        edit_task(&mut ws, ids.b, |t| {
            t.classification = Some(Classification::Open)
        });
        assert_eq!(rules(&ws), vec![Rule::Classification]);
    }

    #[test]
    fn dependency_cycle_a_c_b_a_is_reported() {
        let (mut ws, ids) = valid();
        // b is a's subtask; dependencies may still cross the hierarchy.
        for (n, (from, to)) in [(ids.c, ids.b), (ids.b, ids.a)].into_iter().enumerate() {
            ws.put(Entity::Dependency(Dependency {
                id: DependencyId::from_ulid(ulid::Ulid(700 + n as u128)),
                from,
                to,
                kind: DependencyKind::FinishToStart,
                lag_days: 0,
            }));
        }
        let found = rules(&ws);
        assert!(found.contains(&Rule::Cycle), "{found:?}");
        assert!(found.iter().all(|r| *r == Rule::Cycle));
    }

    #[test]
    fn self_dependency_and_duplicates_are_reported() {
        let (mut ws, ids) = valid();
        ws.put(Entity::Dependency(Dependency {
            id: DependencyId::from_ulid(ulid::Ulid(800)),
            from: ids.c,
            to: ids.c,
            kind: DependencyKind::FinishToStart,
            lag_days: 0,
        }));
        assert_eq!(rules(&ws), vec![Rule::SelfReference]);
        let (mut ws, ids) = valid();
        ws.put(Entity::Dependency(Dependency {
            id: DependencyId::from_ulid(ulid::Ulid(801)),
            from: ids.a,
            to: ids.c,
            kind: DependencyKind::StartToStart,
            lag_days: 2,
        }));
        assert_eq!(rules(&ws), vec![Rule::Duplicate]);
    }

    #[test]
    fn dependency_across_projects_is_reported() {
        let (mut ws, ids) = valid();
        let mut other = ws.project(ids.project).cloned().unwrap();
        other.id = ProjectId::from_ulid(ulid::Ulid(500));
        ws.put(Entity::Project(other.clone()));
        edit_task(&mut ws, ids.c, |t| t.project = other.id);
        assert_eq!(rules(&ws), vec![Rule::CrossProject]);
    }

    #[test]
    fn empty_comment_and_missing_milestone_project_are_reported() {
        let (mut ws, ids) = valid();
        let mut comment = ws.comment(ids.comment).cloned().unwrap();
        comment.body = "  \n".into();
        ws.put(Entity::Comment(comment));
        assert_eq!(rules(&ws), vec![Rule::Text]);
        let (mut ws, ids) = valid();
        let mut milestone = ws.milestone(ids.milestone).cloned().unwrap();
        milestone.project = ProjectId::from_ulid(ulid::Ulid(999));
        ws.put(Entity::Milestone(milestone));
        assert_eq!(rules(&ws), vec![Rule::MissingReference]);
    }

    #[test]
    fn violations_name_the_entity() {
        let (mut ws, ids) = valid();
        edit_task(&mut ws, ids.c, |t| t.progress = 200);
        let found = ws.check();
        assert_eq!(found[0].entity, EntityKey::Task(ids.c));
        assert!(found[0].message.contains("progress"));
    }

    #[test]
    fn workspace_round_trips_through_json() {
        let (ws, _) = valid();
        let json = serde_json::to_string(&ws).unwrap();
        let back: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ws);
    }

    #[test]
    fn put_and_remove_return_previous_values() {
        let (mut ws, ids) = valid();
        let removed = ws.remove(EntityKey::Task(ids.c)).unwrap();
        assert_eq!(removed.key(), EntityKey::Task(ids.c));
        assert!(ws.get(EntityKey::Task(ids.c)).is_none());
        assert!(ws.put(removed.clone()).is_none());
        assert_eq!(ws.put(removed.clone()), Some(removed));
    }

    #[test]
    fn children_and_effective_classification() {
        let (mut ws, ids) = valid();
        let kids: Vec<TaskId> = ws.children(ids.a).map(|t| t.id).collect();
        assert_eq!(kids, vec![ids.b]);
        let mut project = ws.project(ids.project).cloned().unwrap();
        project.classification = Classification::VsNfd;
        ws.put(Entity::Project(project));
        let task = ws.task(ids.c).cloned().unwrap();
        assert_eq!(ws.effective_classification(&task), Classification::VsNfd);
    }
}
