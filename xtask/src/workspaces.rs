//! The two Cargo workspaces (E8.0.4, ADR-0027): the main one and the Rune front end under
//! `frontends/rune/`. The checks read both, so every package here has a `dir` relative to the
//! repository root. The main workspace never depends on the Rune one (the Rune side may take path
//! dependencies into the main one), and a package name belongs to one workspace only.

use std::collections::{BTreeSet, HashSet};
use std::fmt;

use crate::rules::Package;

/// Where the Rune workspace lives, relative to the repository root.
pub const RUNE_DIR: &str = "frontends/rune";

/// True when the package is a member of the Rune workspace.
pub fn is_rune(p: &Package) -> bool {
    in_rune_dir(&p.dir)
}

fn in_rune_dir(dir: &str) -> bool {
    dir == RUNE_DIR
        || dir
            .strip_prefix(RUNE_DIR)
            .is_some_and(|r| r.starts_with('/'))
}

/// A broken rule between the two workspaces.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// A crate of the main workspace depends on a crate under `frontends/rune` (any dependency
    /// kind, dev included: a test edge would still make the main build need the Rune toolchain).
    MainDependsOnRune { krate: String, dep: String },
    /// A package name is used in both workspaces, so a name cannot say which one is meant.
    SameName { name: String },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MainDependsOnRune { krate, dep } => write!(
                f,
                "{krate}: a crate of the main workspace may not depend on {dep}, which is under \
                 {RUNE_DIR} (ADR-0027: no crate of the main workspace depends on the Rune workspace)"
            ),
            Self::SameName { name } => write!(
                f,
                "{name}: a package of this name is in the main workspace and in {RUNE_DIR}"
            ),
        }
    }
}

/// Check the packages of both workspaces against the two rules above.
pub fn check(packages: &[Package]) -> Vec<Violation> {
    let rune: Vec<&Package> = packages.iter().filter(|p| is_rune(p)).collect();
    let rune_names: HashSet<&str> = rune.iter().map(|p| p.name.as_str()).collect();
    let mut out = BTreeSet::new();
    for p in packages.iter().filter(|p| !is_rune(p)) {
        if rune_names.contains(p.name.as_str()) {
            out.insert(Violation::SameName {
                name: p.name.clone(),
            });
        }
        let by_path = p
            .path_dependencies
            .iter()
            .filter(|d| in_rune_dir(d))
            // A path dependency is named by its package when the package is known.
            .map(|d| {
                rune.iter()
                    .find(|r| r.dir == **d)
                    .map_or(d.as_str(), |r| r.name.as_str())
            });
        let by_name = p
            .dependencies
            .iter()
            .chain(&p.dev_dependencies)
            .map(String::as_str)
            .filter(|d| rune_names.contains(d));
        for dep in by_path.chain(by_name) {
            out.insert(Violation::MainDependsOnRune {
                krate: p.name.clone(),
                dep: dep.to_string(),
            });
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules;

    fn pkg(name: &str, kind: &str, dir: &str) -> Package {
        Package {
            name: name.into(),
            kind: Some(kind.into()),
            dir: dir.into(),
            ..Package::default()
        }
    }

    fn with_deps(mut p: Package, deps: &[&str], dev: &[&str], paths: &[&str]) -> Package {
        p.dependencies = deps.iter().map(|s| s.to_string()).collect();
        p.dev_dependencies = dev.iter().map(|s| s.to_string()).collect();
        p.path_dependencies = paths.iter().map(|s| s.to_string()).collect();
        p
    }

    #[test]
    fn a_main_crate_and_a_rune_crate_that_only_looks_into_main_pass() {
        let pkgs = vec![
            pkg("core-a", "core", "crates/core/core-a"),
            with_deps(
                pkg("rune-ui", "ui", "frontends/rune/crates/rune-ui"),
                &["core-a", "rune"],
                &[],
                &["crates/core/core-a"],
            ),
        ];
        assert_eq!(check(&pkgs), vec![]);
        assert_eq!(rules::check(&pkgs).len(), 0);
    }

    #[test]
    fn a_main_crate_that_depends_on_a_rune_crate_fails_by_path_name_and_dev_edge() {
        let rune = pkg("rune-ui", "ui", "frontends/rune/crates/rune-ui");
        for edge in [
            with_deps(
                pkg("a", "ui", "crates/ui/a"),
                &[],
                &[],
                &["frontends/rune/crates/rune-ui"],
            ),
            with_deps(pkg("a", "ui", "crates/ui/a"), &["rune-ui"], &[], &[]),
            with_deps(pkg("a", "ui", "crates/ui/a"), &[], &["rune-ui"], &[]),
        ] {
            let v = check(&[rune.clone(), edge]);
            assert_eq!(
                v,
                vec![Violation::MainDependsOnRune {
                    krate: "a".into(),
                    dep: "rune-ui".into()
                }]
            );
            assert!(v[0].to_string().contains("ADR-0027"));
        }
    }

    #[test]
    fn a_path_dependency_into_the_rune_directory_fails_even_for_an_unknown_package() {
        let a = with_deps(
            pkg("a", "ui", "crates/ui/a"),
            &[],
            &[],
            &["frontends/rune/tools/x"],
        );
        assert_eq!(
            check(&[a]),
            vec![Violation::MainDependsOnRune {
                krate: "a".into(),
                dep: "frontends/rune/tools/x".into()
            }]
        );
        // A directory that only starts with the same letters is not the Rune workspace.
        let b = with_deps(
            pkg("b", "ui", "crates/ui/b"),
            &[],
            &[],
            &["frontends/rune-notes/x"],
        );
        assert_eq!(check(&[b]), vec![]);
    }

    #[test]
    fn the_same_package_name_in_both_workspaces_fails() {
        let pkgs = vec![
            pkg("shared", "ui", "crates/ui/shared"),
            pkg("shared", "ui", "frontends/rune/crates/shared"),
        ];
        assert_eq!(
            check(&pkgs),
            vec![Violation::SameName {
                name: "shared".into()
            }]
        );
    }

    // deps-check on the Rune workspace: the matrix and the toolkit rule read its packages too.
    #[test]
    fn deps_check_fails_a_rune_crate_without_a_kind_and_a_core_crate_that_uses_rune() {
        let no_kind = Package {
            name: "rune-a".into(),
            dir: "frontends/rune/crates/rune-a".into(),
            ..Package::default()
        };
        let mut core = pkg("rune-b", "core", "frontends/rune/crates/rune-b");
        core.uses_ui_toolkit = true;
        let found = rules::check(&[no_kind, core]);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].to_string().contains("rune-a: missing"));
        assert!(found[1].to_string().contains("may not use a UI toolkit"));
    }

    #[test]
    fn rune_and_its_kit_are_ui_toolkits() {
        assert!(rules::is_ui_toolkit("rune"));
        assert!(rules::is_ui_toolkit("rune-kit"));
        assert!(!rules::is_ui_toolkit("meldframe-rune-pin"));
    }
}
