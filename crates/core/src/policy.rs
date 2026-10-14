//! Whether anything may leave the device (docs/PLAN.md §2, §3.8). Off by default.

use serde::{Deserialize, Serialize};

use crate::Classification;

/// Where a request would go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    /// This machine (`localhost`), e.g. a local language model.
    Loopback,
    /// Anywhere else.
    External,
}

/// How much network access is allowed. Ordered from strict to open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// Nothing leaves the device.
    #[default]
    Offline,
    /// Only this machine.
    LocalOnly,
    /// Anywhere the user allowed.
    Allowed,
}

impl NetworkPolicy {
    /// Whether a request to `destination` is allowed.
    pub const fn permits(self, destination: Destination) -> bool {
        match (self, destination) {
            (Self::Offline, _) | (Self::LocalOnly, Destination::External) => false,
            (Self::LocalOnly, Destination::Loopback) | (Self::Allowed, _) => true,
        }
    }

    /// The policy that applies while content of `classification` is open: classified content
    /// never goes beyond this machine.
    pub fn capped_by(self, classification: Classification) -> Self {
        match classification {
            Classification::Open => self,
            Classification::VsNfd => self.min(Self::LocalOnly),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_is_the_default_and_allows_nothing() {
        let policy = NetworkPolicy::default();
        assert_eq!(policy, NetworkPolicy::Offline);
        assert!(!policy.permits(Destination::Loopback));
        assert!(!policy.permits(Destination::External));
    }

    #[test]
    fn local_only_allows_this_machine() {
        assert!(NetworkPolicy::LocalOnly.permits(Destination::Loopback));
        assert!(!NetworkPolicy::LocalOnly.permits(Destination::External));
        assert!(NetworkPolicy::Allowed.permits(Destination::External));
    }

    #[test]
    fn classified_content_never_goes_external() {
        assert_eq!(
            NetworkPolicy::Allowed.capped_by(Classification::VsNfd),
            NetworkPolicy::LocalOnly
        );
        assert_eq!(
            NetworkPolicy::Offline.capped_by(Classification::VsNfd),
            NetworkPolicy::Offline
        );
        assert_eq!(
            NetworkPolicy::Allowed.capped_by(Classification::Open),
            NetworkPolicy::Allowed
        );
    }
}
