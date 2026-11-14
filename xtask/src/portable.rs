//! `portable-check` (L5): portable kinds compile for the planned target platforms.

use crate::plan::Step;
use crate::rules::{Kind, Package};

/// The web target; mobile and 32-bit targets join the list when the product plans them.
pub const TARGETS: &[&str] = &["wasm32-unknown-unknown"];

/// True for kinds that must stay portable (L5).
pub fn is_portable(kind: Kind) -> bool {
    use Kind::*;
    matches!(kind, Core | Engine | Integration | Os | Feature | App | Ffi)
}

/// One build per target over every portable crate. Crates with a missing or unknown kind are
/// left to `deps-check`.
pub fn plan(packages: &[Package]) -> Vec<Step> {
    let mut names: Vec<&str> = packages
        .iter()
        .filter(|p| {
            p.kind
                .as_deref()
                .and_then(Kind::parse)
                .is_some_and(is_portable)
        })
        .map(|p| p.name.as_str())
        .collect();
    names.sort_unstable();
    if names.is_empty() {
        return Vec::new();
    }
    TARGETS
        .iter()
        .map(|target| {
            let mut args = vec!["build", "--target", target];
            for n in &names {
                args.push("-p");
                args.push(n);
            }
            Step::new(format!("portable crates for {target}"), &args)
        })
        .collect()
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

    #[test]
    fn portable_kinds_are_core_engine_integration_os_feature_app_ffi() {
        for k in [
            "core",
            "engine",
            "integration",
            "os",
            "feature",
            "app",
            "ffi",
        ] {
            assert!(is_portable(Kind::parse(k).unwrap()), "{k}");
        }
        for k in ["ui", "feature-ui", "bin", "tool"] {
            assert!(!is_portable(Kind::parse(k).unwrap()), "{k}");
        }
    }

    #[test]
    fn plan_names_only_portable_crates_for_each_target() {
        let steps = plan(&[
            pkg("c", "core"),
            pkg("u", "ui"),
            pkg("f", "feature"),
            pkg("t", "tool"),
        ]);
        assert_eq!(steps.len(), TARGETS.len());
        assert_eq!(
            steps[0].args,
            [
                "build",
                "--target",
                "wasm32-unknown-unknown",
                "-p",
                "c",
                "-p",
                "f"
            ]
        );
    }

    #[test]
    fn crates_without_a_valid_kind_are_skipped() {
        let bad = Package {
            name: "x".into(),
            ..Package::default()
        };
        assert!(plan(&[bad, pkg("y", "widget")]).is_empty());
    }
}
