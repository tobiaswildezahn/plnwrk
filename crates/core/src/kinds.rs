//! The small fixed vocabularies of the model: statuses, priorities, classification levels.

use serde::{Deserialize, Serialize};

macro_rules! names {
    ($ty:ident { $($variant:ident => $name:literal),+ $(,)? }) => {
        impl $ty {
            /// Every value, in display order.
            pub const ALL: [Self; [$(stringify!($variant)),+].len()] = [$(Self::$variant),+];

            /// The machine name, as in JSON (`snake_case`).
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            /// Parses a machine name.
            pub fn from_name(name: &str) -> Option<Self> {
                Self::ALL.into_iter().find(|value| value.name() == name)
            }
        }
    };
}

/// Lifecycle of a project or case.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    /// Being prepared, not started.
    Draft,
    /// In progress.
    #[default]
    Active,
    /// Paused.
    OnHold,
    /// Finished.
    Done,
    /// Put away; hidden from overviews.
    Archived,
}
names!(ProjectStatus { Draft => "draft", Active => "active", OnHold => "on_hold", Done => "done", Archived => "archived" });

/// Where a task stands. The order is the column order of the board.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Not started.
    #[default]
    Todo,
    /// Being worked on.
    InProgress,
    /// Waiting for something.
    Blocked,
    /// Waiting for review or sign-off.
    Review,
    /// Finished.
    Done,
}
names!(TaskStatus { Todo => "todo", InProgress => "in_progress", Blocked => "blocked", Review => "review", Done => "done" });

/// How urgent a task is.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Can wait.
    Low,
    /// Usual.
    #[default]
    Normal,
    /// Soon.
    High,
    /// Now.
    Critical,
}
names!(Priority { Low => "low", Normal => "normal", High => "high", Critical => "critical" });

/// Stored state of a milestone. "Missed" is derived from the date, never stored.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    /// Not reached yet.
    #[default]
    Open,
    /// Reached.
    Reached,
}
names!(MilestoneStatus { Open => "open", Reached => "reached" });

/// How two tasks depend on each other.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// `to` starts after `from` finishes.
    #[default]
    FinishToStart,
    /// `to` starts after `from` starts.
    StartToStart,
    /// `to` finishes after `from` finishes.
    FinishToFinish,
}
names!(DependencyKind { FinishToStart => "finish_to_start", StartToStart => "start_to_start", FinishToFinish => "finish_to_finish" });

/// Security classification. Ordered: a higher value is more restricted.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    /// Not classified.
    #[default]
    Open,
    /// VS – Nur für den Dienstgebrauch.
    VsNfd,
}
names!(Classification { Open => "open", VsNfd => "vs_nfd" });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_as_snake_case() {
        let json = serde_json::to_string(&TaskStatus::InProgress).unwrap();
        assert_eq!(json, "\"in_progress\"");
        let back: TaskStatus = serde_json::from_str("\"on_hold\"").unwrap_or(TaskStatus::Todo);
        assert_eq!(back, TaskStatus::Todo);
        let hold: ProjectStatus = serde_json::from_str("\"on_hold\"").unwrap();
        assert_eq!(hold, ProjectStatus::OnHold);
        let vs: Classification = serde_json::from_str("\"vs_nfd\"").unwrap();
        assert_eq!(vs, Classification::VsNfd);
    }

    #[test]
    fn orders_reflect_meaning() {
        assert!(Classification::VsNfd > Classification::Open);
        assert!(Priority::Critical > Priority::High && Priority::Low < Priority::Normal);
    }

    #[test]
    fn defaults() {
        assert_eq!(ProjectStatus::default(), ProjectStatus::Active);
        assert_eq!(TaskStatus::default(), TaskStatus::Todo);
        assert_eq!(Priority::default(), Priority::Normal);
        assert_eq!(Classification::default(), Classification::Open);
        assert_eq!(DependencyKind::default(), DependencyKind::FinishToStart);
        assert_eq!(MilestoneStatus::default(), MilestoneStatus::Open);
    }

    #[test]
    fn task_statuses_are_listed_in_board_order() {
        assert_eq!(TaskStatus::ALL.first(), Some(&TaskStatus::Todo));
        assert_eq!(TaskStatus::ALL.last(), Some(&TaskStatus::Done));
        assert_eq!(TaskStatus::ALL.len(), 5);
    }

    #[test]
    fn names_match_serde() {
        for status in TaskStatus::ALL {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{}\"", status.name()));
        }
    }
}
