//! Typed identifiers. Every entity has its own id type, so a task id can never be passed where a
//! project id is expected. Ids are ULIDs: sortable by creation time and creatable offline.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
pub use ulid::Ulid;

/// A text that is not a valid id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a valid id: {0:?}")]
pub struct IdParseError(pub String);

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Ulid);

        impl $name {
            /// Wraps a ULID.
            pub const fn from_ulid(ulid: Ulid) -> Self {
                Self(ulid)
            }

            /// The ULID inside.
            pub const fn ulid(self) -> Ulid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Ulid::from_string(text)
                    .map(Self)
                    .map_err(|_| IdParseError(text.chars().take(40).collect()))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }

        impl From<Ulid> for $name {
            fn from(ulid: Ulid) -> Self {
                Self(ulid)
            }
        }
    };
}

id_type!(
    /// Identifies a project (or case – they are the same thing).
    ProjectId
);
id_type!(
    /// Identifies a task.
    TaskId
);
id_type!(
    /// Identifies a milestone.
    MilestoneId
);
id_type!(
    /// Identifies a dependency between two tasks.
    DependencyId
);
id_type!(
    /// Identifies a person.
    PersonId
);
id_type!(
    /// Identifies a comment on a task.
    CommentId
);

/// Where new ids come from. Apps use time and randomness; tests use [`SequentialIds`].
pub trait IdSource {
    /// A ULID never handed out before by this source.
    fn next_ulid(&mut self) -> Ulid;
}

/// Deterministic ids for tests and reproducible examples: 1, 2, 3, …
#[derive(Debug, Clone)]
pub struct SequentialIds {
    next: u128,
}

impl SequentialIds {
    /// Starts counting at `first`.
    pub const fn starting_at(first: u128) -> Self {
        Self { next: first }
    }
}

impl Default for SequentialIds {
    fn default() -> Self {
        Self::starting_at(1)
    }
}

impl IdSource for SequentialIds {
    fn next_ulid(&mut self) -> Ulid {
        let ulid = Ulid(self.next);
        self.next = self.next.wrapping_add(1);
        ulid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_parse_round_trip() {
        let id = TaskId::from_ulid(Ulid::from_parts(1_700_000_000_000, 42));
        let text = id.to_string();
        assert_eq!(text.len(), 26);
        assert_eq!(text.parse::<TaskId>().unwrap(), id);
    }

    #[test]
    fn serializes_as_plain_string() {
        let id = ProjectId::from_ulid(Ulid::from_parts(1, 2));
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<ProjectId>(&json).unwrap(), id);
    }

    #[test]
    fn rejects_garbage() {
        assert!("kaputt".parse::<TaskId>().is_err());
        assert!("".parse::<TaskId>().is_err());
        assert!(serde_json::from_str::<TaskId>("42").is_err());
        assert!(serde_json::from_str::<TaskId>("\"ZZZZZZZZZZZZZZZZZZZZZZZZZZZ\"").is_err());
    }

    #[test]
    fn sequential_ids_are_unique_and_increasing() {
        let mut ids = SequentialIds::default();
        let a = ids.next_ulid();
        let b = ids.next_ulid();
        let c = ids.next_ulid();
        assert!(a < b && b < c);
    }
}
