//! Layering rules (L1 to L4 of `docs/standards/modular-thin-ui.md`) as a pure function over
//! package data. Nothing here touches the file system or runs a process.

use std::collections::HashMap;
use std::fmt;

/// One workspace package, reduced to what the rules need.
#[derive(Debug, Clone, Default)]
pub struct Package {
    pub name: String,
    /// Raw `[package.metadata.meldframe] kind`, if present.
    pub kind: Option<String>,
    /// Raw `group`; feature crates must declare one.
    pub group: Option<String>,
    /// Groups this feature crate may import (`composite = [..]`).
    pub composite: Vec<String>,
    /// Names of packages this one depends on (normal and build dependencies).
    pub dependencies: Vec<String>,
    /// True when any dependency is a UI toolkit.
    pub uses_ui_toolkit: bool,
    /// Feature names declared in the manifest (including `default`).
    pub features: Vec<String>,
    /// Manifest directory relative to the workspace root, with `/` separators.
    pub dir: String,
}

/// A crate kind (L1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Core,
    Engine,
    Integration,
    Os,
    Feature,
    App,
    Ffi,
    Ui,
    FeatureUi,
    Bin,
    Tool,
}

/// One broken rule, naming the crates involved.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    MissingKind {
        krate: String,
    },
    UnknownKind {
        krate: String,
        kind: String,
    },
    MissingGroup {
        krate: String,
    },
    ForbiddenEdge {
        krate: String,
        kind: String,
        dep: String,
        dep_kind: String,
    },
    FeatureCrossGroup {
        krate: String,
        group: String,
        dep: String,
        dep_group: String,
    },
    UiToolkit {
        krate: String,
        kind: String,
    },
    GroupOnWrongKind {
        krate: String,
        kind: String,
        group: String,
    },
    UnknownComposite {
        krate: String,
        composite: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::MissingKind { krate } => {
                write!(f, "{krate}: missing [package.metadata.meldframe] kind")
            }
            Violation::UnknownKind { krate, kind } => {
                write!(f, "{krate}: unknown kind `{kind}`")
            }
            Violation::MissingGroup { krate } => {
                write!(f, "{krate}: feature crate without a `group`")
            }
            Violation::ForbiddenEdge {
                krate,
                kind,
                dep,
                dep_kind,
            } => {
                write!(
                    f,
                    "{krate}: a {kind} crate may not depend on {dep} ({dep_kind})"
                )
            }
            Violation::FeatureCrossGroup {
                krate,
                group,
                dep,
                dep_group,
            } => write!(
                f,
                "{krate}: feature in group `{group}` imports {dep} (group `{dep_group}`) \
                 without `composite` listing `{dep_group}`"
            ),
            Violation::UiToolkit { krate, kind } => {
                write!(f, "{krate}: a {kind} crate may not use a UI toolkit")
            }
            Violation::GroupOnWrongKind { krate, kind, group } => write!(
                f,
                "{krate}: a {kind} crate may not declare group `{group}`; only feature and \
                 feature-ui crates have one"
            ),
            Violation::UnknownComposite { krate, composite } => write!(
                f,
                "{krate}: composite names group `{composite}`, which no feature crate declares"
            ),
        }
    }
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "core" => Kind::Core,
            "engine" => Kind::Engine,
            "integration" => Kind::Integration,
            "os" => Kind::Os,
            "feature" => Kind::Feature,
            "app" => Kind::App,
            "ffi" => Kind::Ffi,
            "ui" => Kind::Ui,
            "feature-ui" => Kind::FeatureUi,
            "bin" => Kind::Bin,
            "tool" => Kind::Tool,
            _ => return None,
        })
    }

    /// Kinds this kind may depend on (L2), apart from the feature-to-feature group rule.
    pub fn allowed_deps(self) -> &'static [Kind] {
        use Kind::*;
        match self {
            Core => &[Core],
            Engine | Integration => &[Core, Engine],
            Os => &[Core],
            Feature | App => &[Core, Engine, Integration, Os, Feature],
            Ffi => &[Core, App],
            Ui | FeatureUi => &[Core, App, Ui],
            Bin | Tool => &[
                Core,
                Engine,
                Integration,
                Os,
                Feature,
                App,
                Ffi,
                Ui,
                FeatureUi,
                Bin,
                Tool,
            ],
        }
    }

    /// L3: only ui, feature-ui and bin crates may use a UI toolkit.
    pub fn may_use_ui_toolkit(self) -> bool {
        matches!(self, Kind::Ui | Kind::FeatureUi | Kind::Bin)
    }
}

/// Crates that are UI toolkits (L3). Matched against dependency package names.
const UI_TOOLKITS: &[&str] = &[
    "gpui",
    "gpui-pre",
    "gpui-pre-platform",
    "gpui-component",
    "egui",
    "eframe",
    "iced",
    "slint",
    "dioxus",
    "leptos",
    "yew",
    "tauri",
    "ratatui",
];

/// True when `name` is a known UI toolkit package.
pub fn is_ui_toolkit(name: &str) -> bool {
    UI_TOOLKITS.contains(&name)
}

/// Check every package against L1 to L4 and return all violations, sorted.
pub fn check(packages: &[Package]) -> Vec<Violation> {
    let by_name: HashMap<&str, &Package> = packages.iter().map(|p| (p.name.as_str(), p)).collect();
    let mut out = crate::groups::check(packages);
    for p in packages {
        let krate = p.name.clone();
        let kind = match p.kind.as_deref() {
            None => {
                out.push(Violation::MissingKind { krate });
                continue;
            }
            Some(raw) => match Kind::parse(raw) {
                Some(k) => k,
                None => {
                    out.push(Violation::UnknownKind {
                        krate,
                        kind: raw.to_string(),
                    });
                    continue;
                }
            },
        };
        let kind_name = p.kind.clone().unwrap_or_default();
        if kind == Kind::Feature && p.group.is_none() {
            out.push(Violation::MissingGroup {
                krate: krate.clone(),
            });
        }
        if p.uses_ui_toolkit && !kind.may_use_ui_toolkit() {
            out.push(Violation::UiToolkit {
                krate: krate.clone(),
                kind: kind_name.clone(),
            });
        }
        for dep_name in &p.dependencies {
            let Some(dep) = by_name.get(dep_name.as_str()) else {
                continue;
            };
            // A dependency without a valid kind is reported on its own.
            let Some(dep_kind) = dep.kind.as_deref().and_then(Kind::parse) else {
                continue;
            };
            if !kind.allowed_deps().contains(&dep_kind) {
                out.push(Violation::ForbiddenEdge {
                    krate: krate.clone(),
                    kind: kind_name.clone(),
                    dep: dep_name.clone(),
                    dep_kind: dep.kind.clone().unwrap_or_default(),
                });
            } else if kind == Kind::Feature
                && dep_kind == Kind::Feature
                && let (Some(g), Some(dg)) = (&p.group, &dep.group)
                && g != dg
                && !p.composite.contains(dg)
            {
                out.push(Violation::FeatureCrossGroup {
                    krate: krate.clone(),
                    group: g.clone(),
                    dep: dep_name.clone(),
                    dep_group: dg.clone(),
                });
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, kind: &str) -> Package {
        Package {
            name: name.into(),
            kind: Some(kind.into()),
            ..Package::default()
        }
    }

    fn feat(name: &str, group: &str, composite: &[&str], deps: &[&str]) -> Package {
        Package {
            group: Some(group.into()),
            composite: composite.iter().map(|s| s.to_string()).collect(),
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            ..pkg(name, "feature")
        }
    }

    fn dep(mut p: Package, deps: &[&str]) -> Package {
        p.dependencies = deps.iter().map(|s| s.to_string()).collect();
        p
    }

    #[test]
    fn valid_workspace_has_no_violations() {
        let pkgs = vec![
            pkg("core-a", "core"),
            dep(pkg("engine-a", "engine"), &["core-a"]),
            dep(feat("feat-a", "a", &[], &[]), &["core-a", "engine-a"]),
            dep(pkg("app-a", "app"), &["feat-a"]),
            dep(pkg("ui-a", "ui"), &["app-a", "core-a"]),
            dep(pkg("server", "bin"), &["ui-a", "app-a", "feat-a"]),
            dep(pkg("tool-a", "tool"), &["core-a", "server"]),
        ];
        assert_eq!(check(&pkgs), vec![]);
    }

    #[test]
    fn missing_kind_fails() {
        let p = Package {
            name: "no-kind".into(),
            ..Package::default()
        };
        assert_eq!(
            check(&[p]),
            vec![Violation::MissingKind {
                krate: "no-kind".into()
            }]
        );
    }

    #[test]
    fn unknown_kind_fails() {
        let v = check(&[pkg("odd", "widget")]);
        assert_eq!(
            v,
            vec![Violation::UnknownKind {
                krate: "odd".into(),
                kind: "widget".into()
            }]
        );
    }

    #[test]
    fn forbidden_kind_edge_fails() {
        // core may only depend on core.
        let pkgs = vec![pkg("eng", "engine"), dep(pkg("core-a", "core"), &["eng"])];
        assert_eq!(
            check(&pkgs),
            vec![Violation::ForbiddenEdge {
                krate: "core-a".into(),
                kind: "core".into(),
                dep: "eng".into(),
                dep_kind: "engine".into(),
            }]
        );
    }

    #[test]
    fn feature_ui_may_not_depend_on_feature() {
        let pkgs = vec![
            feat("f", "g", &[], &[]),
            dep(pkg("f-ui", "feature-ui"), &["f"]),
        ];
        let v = check(&pkgs);
        assert_eq!(v.len(), 1);
        assert!(matches!(v[0], Violation::ForbiddenEdge { .. }));
    }

    #[test]
    fn feature_importing_other_group_without_composite_fails() {
        let pkgs = vec![feat("one", "a", &[], &[]), feat("two", "b", &[], &["one"])];
        assert_eq!(
            check(&pkgs),
            vec![Violation::FeatureCrossGroup {
                krate: "two".into(),
                group: "b".into(),
                dep: "one".into(),
                dep_group: "a".into(),
            }]
        );
    }

    #[test]
    fn feature_importing_other_group_with_composite_passes() {
        let pkgs = vec![
            feat("one", "a", &[], &[]),
            feat("two", "b", &["a"], &["one"]),
        ];
        assert_eq!(check(&pkgs), vec![]);
    }

    #[test]
    fn feature_importing_same_group_passes() {
        let pkgs = vec![feat("one", "a", &[], &[]), feat("two", "a", &[], &["one"])];
        assert_eq!(check(&pkgs), vec![]);
    }

    #[test]
    fn feature_without_group_fails() {
        let p = pkg("grouped-not", "feature");
        assert_eq!(
            check(&[p]),
            vec![Violation::MissingGroup {
                krate: "grouped-not".into()
            }]
        );
    }

    #[test]
    fn ui_toolkit_in_non_ui_crate_fails() {
        for kind in [
            "core",
            "engine",
            "integration",
            "os",
            "feature",
            "app",
            "ffi",
            "tool",
        ] {
            let mut p = feat("x", "g", &[], &[]);
            p.kind = Some(kind.into());
            p.group = (kind == "feature").then(|| "g".into());
            p.uses_ui_toolkit = true;
            let v = check(&[p]);
            assert_eq!(
                v,
                vec![Violation::UiToolkit {
                    krate: "x".into(),
                    kind: kind.into()
                }]
            );
        }
    }

    #[test]
    fn ui_toolkit_in_ui_feature_ui_and_bin_passes() {
        for kind in ["ui", "feature-ui", "bin"] {
            let mut p = feat("x", "g", &[], &[]);
            p.kind = Some(kind.into());
            p.group = (kind == "feature-ui").then(|| "g".into());
            p.uses_ui_toolkit = true;
            assert_eq!(check(&[p]), vec![]);
        }
    }

    #[test]
    fn matrix_matches_the_standard() {
        use Kind::*;
        let table: [(Kind, &[Kind]); 10] = [
            (Core, &[Core]),
            (Engine, &[Core, Engine]),
            (Integration, &[Core, Engine]),
            (Os, &[Core]),
            (Feature, &[Core, Engine, Integration, Os, Feature]),
            (App, &[Core, Engine, Integration, Os, Feature]),
            (Ffi, &[Core, App]),
            (Ui, &[Core, App, Ui]),
            (FeatureUi, &[Core, App, Ui]),
            (
                Bin,
                &[
                    Core,
                    Engine,
                    Integration,
                    Os,
                    Feature,
                    App,
                    Ffi,
                    Ui,
                    FeatureUi,
                    Bin,
                    Tool,
                ],
            ),
        ];
        for (kind, allowed) in table {
            assert_eq!(kind.allowed_deps(), allowed, "{kind:?}");
        }
        assert_eq!(Tool.allowed_deps(), Bin.allowed_deps());
    }

    #[test]
    fn violations_name_every_crate_and_are_sorted() {
        let pkgs = vec![
            pkg("zeta", "widget"),
            Package {
                name: "alpha".into(),
                ..Package::default()
            },
        ];
        let v = check(&pkgs);
        assert_eq!(v.len(), 2);
        assert!(v[0].to_string().contains("alpha"));
        assert!(v[1].to_string().contains("zeta"));
    }
}
