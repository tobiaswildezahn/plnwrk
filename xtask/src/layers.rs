//! Layering rule of docs/PLAN.md §3.1: a workspace crate may depend only on workspace crates
//! in a strictly lower layer. The pure crates (core, engine) may in addition use only the
//! external crates listed in [`PURE_EXTERNAL`].

use std::collections::BTreeSet;

/// Every workspace crate and its layer. Adding a crate means adding it here.
pub const LAYERS: &[(&str, u8)] = &[
    ("plnwrk-core", 0),
    ("plnwrk-engine", 1),
    ("plnwrk-store", 2),
    ("plnwrk-schedule", 2),
    ("plnwrk-platform", 2),
    ("plnwrk-assistant", 3),
    ("plnwrk-mcp", 3),
    ("plnwrk-ui", 4),
    ("plnwrk", 5),
    ("plnwrk-web", 5),
    ("plnwrk-cli", 5),
];

/// Tooling crates outside the layer model.
const EXEMPT: &[&str] = &["xtask"];

/// External crates the pure crates may use at run time (dev-dependencies are free).
/// Anything that could reach the clock, randomness, files or the network stays out.
const PURE_EXTERNAL: &[(&str, &[&str])] = &[
    ("plnwrk-core", &["jiff", "serde", "thiserror", "ulid"]),
    (
        "plnwrk-engine",
        &["jiff", "serde", "serde_json", "thiserror", "ulid"],
    ),
];

pub fn layer_of(name: &str) -> Option<u8> {
    LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

/// One dependency of a package. Build dependencies count as normal ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub dev: bool,
}

impl Dep {
    #[cfg(test)]
    pub fn normal(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            dev: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub deps: Vec<Dep>,
}

/// Checks every workspace package. Empty result means the rules hold. `packages` must be the
/// complete member list (`cargo metadata --no-deps`): membership decides what is a workspace
/// crate, not the name.
pub fn check(packages: &[Package]) -> Vec<String> {
    let members: BTreeSet<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let mut errors = Vec::new();
    for package in packages {
        if EXEMPT.contains(&package.name.as_str()) {
            continue;
        }
        let Some(own) = layer_of(&package.name) else {
            errors.push(format!(
                "{} is not in the layer table (xtask/src/layers.rs); add it with its layer",
                package.name
            ));
            continue;
        };
        let allowed_external = PURE_EXTERNAL
            .iter()
            .find(|(n, _)| *n == package.name)
            .map(|(_, allowed)| *allowed);
        for dep in &package.deps {
            if members.contains(dep.name.as_str()) {
                match layer_of(&dep.name) {
                    Some(theirs) if theirs < own => {}
                    Some(theirs) => errors.push(format!(
                        "{} (layer {own}) must not depend on {} (layer {theirs})",
                        package.name, dep.name
                    )),
                    None => {} // reported for the member itself
                }
            } else if let Some(allowed) = allowed_external
                && !dep.dev
                && !allowed.contains(&dep.name.as_str())
            {
                errors.push(format!(
                    "{} is a pure crate and must not depend on {}; allowed: {}",
                    package.name,
                    dep.name,
                    allowed.join(", ")
                ));
            }
        }
    }
    errors
}

/// Reads the output of `cargo metadata --format-version 1 --no-deps`.
pub fn parse_metadata(json: &str) -> Result<Vec<Package>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("cargo metadata: {e}"))?;
    let packages = value
        .get("packages")
        .and_then(|p| p.as_array())
        .ok_or("cargo metadata: no packages array")?;
    packages
        .iter()
        .map(|p| {
            let name = p
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or("cargo metadata: package without name")?;
            let deps = p
                .get("dependencies")
                .and_then(|d| d.as_array())
                .map(|deps| {
                    deps.iter()
                        .filter_map(|d| {
                            let name = d.get("name").and_then(|n| n.as_str())?;
                            let dev = d.get("kind").and_then(|k| k.as_str()) == Some("dev");
                            Some(Dep {
                                name: name.to_owned(),
                                dev,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(Package {
                name: name.to_owned(),
                deps,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, deps: &[&str]) -> Package {
        Package {
            name: name.into(),
            deps: deps.iter().map(|d| Dep::normal(d)).collect(),
        }
    }

    fn workspace(extra: Package) -> Vec<Package> {
        let mut all: Vec<Package> = LAYERS.iter().map(|(n, _)| pkg(n, &[])).collect();
        all.retain(|p| p.name != extra.name);
        all.push(extra);
        all
    }

    #[test]
    fn downward_dependency_is_ok() {
        assert!(check(&workspace(pkg("plnwrk-engine", &["plnwrk-core"]))).is_empty());
    }

    #[test]
    fn upward_dependency_is_reported() {
        let errors = check(&workspace(pkg("plnwrk-core", &["plnwrk-engine"])));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("plnwrk-core"));
        assert!(errors[0].contains("plnwrk-engine"));
    }

    #[test]
    fn same_layer_dependency_is_reported() {
        let errors = check(&workspace(pkg("plnwrk-mcp", &["plnwrk-assistant"])));
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn unknown_workspace_crate_is_reported() {
        let errors = check(&workspace(pkg("plnwrk-new", &[])));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("layer table"));
    }

    #[test]
    fn workspace_crates_are_recognised_by_membership_not_by_name() {
        // A member called `helper` is still a workspace crate and must be in the table.
        let mut all = workspace(pkg("plnwrk-store", &["helper"]));
        all.push(pkg("helper", &[]));
        let errors = check(&all);
        assert!(
            errors
                .iter()
                .any(|e| e.contains("helper") && e.contains("layer table"))
        );
    }

    #[test]
    fn external_dependencies_of_outer_crates_are_ignored() {
        assert!(check(&workspace(pkg("plnwrk-ui", &["eframe", "plnwrk-engine"]))).is_empty());
    }

    #[test]
    fn pure_crates_may_only_use_allowed_external_crates() {
        assert!(check(&workspace(pkg("plnwrk-core", &["serde", "jiff"]))).is_empty());
        let errors = check(&workspace(pkg("plnwrk-core", &["tokio"])));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("tokio"));
        let errors = check(&workspace(pkg("plnwrk-engine", &["getrandom"])));
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn pure_crates_may_use_any_dev_dependency() {
        let mut core = pkg("plnwrk-core", &[]);
        core.deps.push(Dep {
            name: "proptest".into(),
            dev: true,
        });
        assert!(check(&workspace(core)).is_empty());
    }

    #[test]
    fn xtask_is_exempt() {
        let mut all = workspace(pkg("plnwrk-cli", &[]));
        all.push(pkg("xtask", &["plnwrk-ui"]));
        assert!(check(&all).is_empty());
    }

    #[test]
    fn parses_cargo_metadata_with_kinds_and_renames() {
        let json = r#"{"packages":[{"name":"plnwrk-core","dependencies":[
            {"name":"plnwrk-engine","kind":"dev","rename":null},
            {"name":"serde","kind":null,"rename":"serde_renamed"},
            {"name":"cc","kind":"build"}]}]}"#;
        let packages = parse_metadata(json).unwrap();
        let deps = &packages[0].deps;
        assert_eq!(
            deps[0],
            Dep {
                name: "plnwrk-engine".into(),
                dev: true
            }
        );
        assert_eq!(deps[1], Dep::normal("serde"));
        assert_eq!(deps[2], Dep::normal("cc"));
    }

    #[test]
    fn a_dev_dependency_upwards_is_still_reported() {
        let mut core = pkg("plnwrk-core", &[]);
        core.deps.push(Dep {
            name: "plnwrk-engine".into(),
            dev: true,
        });
        assert_eq!(check(&workspace(core)).len(), 1);
    }

    #[test]
    fn malformed_metadata_is_an_error_not_a_panic() {
        assert!(parse_metadata("not json").is_err());
        assert!(parse_metadata(r#"{"packages":[{"dependencies":[]}]}"#).is_err());
    }
}
