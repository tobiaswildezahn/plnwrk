//! Layering rule of docs/PLAN.md §3.1: a workspace crate may depend only on workspace crates
//! in a strictly lower layer.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub deps: Vec<String>,
}

/// Every workspace crate and its layer. Adding a crate means adding it here.
const LAYERS: &[(&str, u8)] = &[
    ("plnwrk-core", 0),
    ("plnwrk-engine", 1),
    ("plnwrk-store", 2),
    ("plnwrk-schedule", 2),
    ("plnwrk-assistant", 3),
    ("plnwrk-mcp", 3),
    ("plnwrk-ui", 4),
    ("plnwrk", 5),
    ("plnwrk-web", 5),
    ("plnwrk-cli", 5),
];

/// Tooling crates outside the layer model.
const EXEMPT: &[&str] = &["xtask"];

pub fn layer_of(name: &str) -> Option<u8> {
    LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l)
}

fn is_workspace_name(name: &str) -> bool {
    name == "plnwrk" || name.starts_with("plnwrk-")
}

/// Checks every package against the layer table. Empty result means the rule holds.
pub fn check(packages: &[Package]) -> Vec<String> {
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
        for dep in package.deps.iter().filter(|d| is_workspace_name(d)) {
            match layer_of(dep) {
                Some(theirs) if theirs < own => {}
                Some(theirs) => errors.push(format!(
                    "{} (layer {own}) must not depend on {dep} (layer {theirs})",
                    package.name
                )),
                None => errors.push(format!(
                    "{} depends on {dep}, which is not in the layer table",
                    package.name
                )),
            }
        }
    }
    errors
}

/// Reads the output of `cargo metadata --format-version 1 --no-deps`. All dependency kinds
/// (normal, dev, build) count.
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
                        .filter_map(|d| d.get("name").and_then(|n| n.as_str()))
                        .map(String::from)
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
            deps: deps.iter().map(|d| (*d).into()).collect(),
        }
    }

    #[test]
    fn downward_dependency_is_ok() {
        assert!(check(&[pkg("plnwrk-engine", &["plnwrk-core"])]).is_empty());
    }

    #[test]
    fn upward_dependency_is_reported() {
        let errors = check(&[pkg("plnwrk-core", &["plnwrk-engine"])]);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("plnwrk-core"));
        assert!(errors[0].contains("plnwrk-engine"));
    }

    #[test]
    fn same_layer_dependency_is_reported() {
        assert_eq!(check(&[pkg("plnwrk-mcp", &["plnwrk-assistant"])]).len(), 1);
    }

    #[test]
    fn unknown_workspace_crate_is_reported() {
        let errors = check(&[pkg("plnwrk-new", &[])]);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("layer table"));
    }

    #[test]
    fn external_dependencies_are_ignored() {
        assert!(check(&[pkg("plnwrk-ui", &["eframe", "plnwrk-engine"])]).is_empty());
    }

    #[test]
    fn xtask_is_exempt() {
        assert!(check(&[pkg("xtask", &["plnwrk-ui"])]).is_empty());
    }

    #[test]
    fn parses_cargo_metadata_including_dev_dependencies() {
        let json = r#"{"packages":[{"name":"plnwrk-core","dependencies":[
            {"name":"plnwrk-engine","kind":"dev"},{"name":"serde","kind":null}]}]}"#;
        let packages = parse_metadata(json).unwrap();
        assert_eq!(packages[0].deps, vec!["plnwrk-engine", "serde"]);
        assert_eq!(check(&packages).len(), 1);
    }

    #[test]
    fn malformed_metadata_is_an_error_not_a_panic() {
        assert!(parse_metadata("not json").is_err());
        assert!(parse_metadata(r#"{"packages":[{"dependencies":[]}]}"#).is_err());
    }
}
