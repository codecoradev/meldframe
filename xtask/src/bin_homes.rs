//! Where a `bin` crate may live (decision 47, issue 317). The matrix lets `bin` depend on every
//! kind, so a crate labelled `bin` is outside L2 and outside `portable-check` (L5). That is right
//! for the adapters that link axum or sqlx, which cannot be built for `wasm32`, and wrong for any
//! other crate that would take the label to get out of the matrix. This check ties the label to
//! the directory: a `bin` crate is an app, an adapter under `crates/integrations/`, or a shell of
//! the Rune front end, and nothing else.

use std::fmt;

use crate::rules::Package;

/// The directories, relative to the repository root, whose crates may be declared `bin`.
pub const BIN_HOMES: [&str; 4] = [
    "apps/",
    "crates/integrations/",
    "frontends/rune/apps/",
    "frontends/rune/tools/",
];

/// A crate declared `bin` outside the directories of `BIN_HOMES`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Violation {
    pub krate: String,
    pub dir: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: a bin crate may only live under {}; this one is in `{}` (a crate that is not an app \
             or an adapter takes the kind of what it is: bin is outside the matrix and portable-check)",
            self.krate,
            BIN_HOMES.join(", "),
            self.dir
        )
    }
}

/// Every `bin` crate that is not under one of `BIN_HOMES`, sorted.
pub fn check(packages: &[Package]) -> Vec<Violation> {
    let mut out: Vec<Violation> = packages
        .iter()
        .filter(|p| p.kind.as_deref() == Some("bin"))
        .filter(|p| !BIN_HOMES.iter().any(|home| p.dir.starts_with(home)))
        .map(|p| Violation {
            krate: p.name.clone(),
            dir: p.dir.clone(),
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, kind: &str, dir: &str) -> Package {
        Package {
            name: name.into(),
            kind: Some(kind.into()),
            dir: dir.into(),
            ..Package::default()
        }
    }

    fn bad(name: &str, dir: &str) -> Violation {
        Violation {
            krate: name.into(),
            dir: dir.into(),
        }
    }

    #[test]
    fn bin_crates_in_the_apps_the_adapters_and_the_rune_shells_pass() {
        let pkgs = [
            pkg("server", "bin", "apps/server"),
            pkg("http", "bin", "crates/integrations/meldframe-http"),
            pkg("desktop", "bin", "frontends/rune/apps/desktop"),
            pkg("measure", "bin", "frontends/rune/tools/measure"),
        ];
        assert_eq!(check(&pkgs), vec![]);
    }

    #[test]
    fn a_bin_crate_in_core_features_or_the_app_layer_fails() {
        let pkgs = [
            pkg("a", "bin", "crates/core/a"),
            pkg("b", "bin", "crates/features/export/b"),
            pkg("c", "bin", "crates/app/c"),
            pkg("d", "bin", "packages/d"),
        ];
        assert_eq!(
            check(&pkgs),
            vec![
                bad("a", "crates/core/a"),
                bad("b", "crates/features/export/b"),
                bad("c", "crates/app/c"),
                bad("d", "packages/d"),
            ]
        );
    }

    #[test]
    fn a_directory_that_only_starts_like_a_home_fails() {
        let pkgs = [
            pkg("a", "bin", "apps-extra/a"),
            pkg("b", "bin", "crates/integrations-old/b"),
            pkg("c", "bin", "apps"),
        ];
        assert_eq!(check(&pkgs).len(), 3);
    }

    #[test]
    fn other_kinds_may_live_anywhere_and_a_missing_kind_is_left_to_the_kind_rules() {
        let mut none = pkg("n", "core", "elsewhere/n");
        none.kind = None;
        let pkgs = [
            pkg("t", "tool", "crates/core/t"),
            pkg("i", "integration", "elsewhere/i"),
            pkg("w", "widget", "elsewhere/w"),
            none,
        ];
        assert_eq!(check(&pkgs), vec![]);
    }

    #[test]
    fn the_message_names_the_crate_and_the_directory() {
        let text = bad("x", "crates/core/x").to_string();
        assert!(
            text.starts_with("x: a bin crate may only live under"),
            "{text}"
        );
        assert!(text.contains("`crates/core/x`"), "{text}");
    }
}
