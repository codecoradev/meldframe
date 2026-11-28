//! Checks of `group` and `composite` (L4 of `docs/standards/modular-thin-ui.md`) that the matrix
//! does not make: a group belongs to a feature crate, and a composite names a group that exists.

use std::collections::HashSet;

use crate::rules::{Kind, Package, Violation};

/// The declared kind of a package when it is a known one.
pub fn kind_of(p: &Package) -> Option<Kind> {
    p.kind.as_deref().and_then(Kind::parse)
}

fn has_groups(kind: Kind) -> bool {
    matches!(kind, Kind::Feature | Kind::FeatureUi)
}

/// A package with a missing or unknown kind is left to the kind rules of `rules::check`.
pub fn check(packages: &[Package]) -> Vec<Violation> {
    let declared: HashSet<&str> = packages
        .iter()
        .filter(|p| kind_of(p).is_some_and(has_groups))
        .filter_map(|p| p.group.as_deref())
        .collect();
    let mut out = Vec::new();
    for p in packages {
        let Some(kind) = kind_of(p) else {
            continue;
        };
        if let Some(group) = &p.group
            && !has_groups(kind)
        {
            out.push(Violation::GroupOnWrongKind {
                krate: p.name.clone(),
                kind: p.kind.clone().unwrap_or_default(),
                group: group.clone(),
            });
        }
        out.extend(
            p.composite
                .iter()
                .filter(|c| !declared.contains(c.as_str()))
                .map(|c| Violation::UnknownComposite {
                    krate: p.name.clone(),
                    composite: c.clone(),
                }),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, kind: &str, group: Option<&str>, composite: &[&str]) -> Package {
        Package {
            name: name.into(),
            kind: Some(kind.into()),
            group: group.map(str::to_string),
            composite: composite.iter().map(|s| s.to_string()).collect(),
            ..Package::default()
        }
    }

    #[test]
    fn a_group_on_a_kind_that_is_not_feature_or_feature_ui_fails() {
        for kind in [
            "core",
            "engine",
            "integration",
            "os",
            "app",
            "ffi",
            "ui",
            "bin",
            "tool",
        ] {
            let v = check(&[pkg("x", kind, Some("g"), &[])]);
            assert_eq!(
                v,
                vec![Violation::GroupOnWrongKind {
                    krate: "x".into(),
                    kind: kind.into(),
                    group: "g".into()
                }],
                "{kind}"
            );
        }
    }

    #[test]
    fn a_group_on_feature_and_feature_ui_passes() {
        for kind in ["feature", "feature-ui"] {
            assert_eq!(check(&[pkg("x", kind, Some("g"), &[])]), vec![]);
        }
    }

    #[test]
    fn a_composite_naming_a_group_that_no_feature_declares_fails() {
        let pkgs = [
            pkg("one", "feature", Some("a"), &[]),
            pkg("two", "feature", Some("b"), &["a", "nope"]),
        ];
        assert_eq!(
            check(&pkgs),
            vec![Violation::UnknownComposite {
                krate: "two".into(),
                composite: "nope".into()
            }]
        );
    }

    #[test]
    fn a_group_declared_on_the_wrong_kind_does_not_count_as_a_group() {
        let pkgs = [
            pkg("tool-a", "tool", Some("a"), &[]),
            pkg("two", "feature", Some("b"), &["a"]),
        ];
        let v = check(&pkgs);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(
            v.iter()
                .any(|v| matches!(v, Violation::UnknownComposite { .. }))
        );
    }

    #[test]
    fn a_composite_naming_an_existing_group_passes() {
        let pkgs = [
            pkg("one", "feature", Some("a"), &[]),
            pkg("two", "feature", Some("b"), &["a"]),
            pkg("ui-a", "feature-ui", Some("a"), &[]),
        ];
        assert_eq!(check(&pkgs), vec![]);
    }

    #[test]
    fn a_package_without_a_valid_kind_is_left_to_the_kind_rules() {
        let mut p = pkg("x", "widget", Some("g"), &["nope"]);
        assert_eq!(check(std::slice::from_ref(&p)), vec![]);
        p.kind = None;
        assert_eq!(check(&[p]), vec![]);
    }
}
