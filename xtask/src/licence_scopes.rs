//! `licences --check` over both workspaces (E8.0.4, ADR-0027). Each workspace has its own
//! `deny.toml` and so its own allow list: the main one stays as it is, and the Rune workspace
//! lists what its lock reaches (Rune's own graph has licences the product does not use). A crate is
//! judged by the list of the workspace that reaches it.

use xtask_licences as licences;

/// One workspace: its name for messages, its `cargo metadata` output (with dependencies) and its
/// `deny.toml`.
pub struct Scope<'a> {
    pub name: &'a str,
    pub metadata: &'a str,
    pub deny_toml: &'a str,
}

/// What the check found over all scopes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Violations, each prefixed with its workspace when there is more than one scope.
    pub violations: Vec<String>,
    /// One line per third-party crate (`workspace name version licence`), for `--list`.
    pub listing: Vec<String>,
    /// Third-party crates read in each scope, in order.
    pub counts: Vec<usize>,
}

/// Judge every scope against its own allow list.
pub fn check(scopes: &[Scope]) -> Result<Outcome, String> {
    let mut out = Outcome::default();
    for scope in scopes {
        let allowed =
            licences::parse_allow(scope.deny_toml).map_err(|e| format!("{}: {e}", scope.name))?;
        let deps =
            licences::parse_deps(scope.metadata).map_err(|e| format!("{}: {e}", scope.name))?;
        out.counts.push(deps.len());
        out.listing.extend(deps.iter().map(|d| {
            let licence = d.licence.as_deref().unwrap_or("none declared");
            format!("{}: {} {} {licence}", scope.name, d.name, d.version)
        }));
        let prefix = if scopes.len() > 1 {
            format!("{}: ", scope.name)
        } else {
            String::new()
        };
        out.violations.extend(
            licences::check(&deps, &allowed)
                .iter()
                .map(|v| format!("{prefix}{v}")),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crate with a source, so it counts as third-party, and a licence.
    fn metadata(licence: &str) -> String {
        format!(
            r#"{{"packages":[
              {{"name":"spellbook","version":"0.3.0","license":"{licence}","source":"registry+x"}},
              {{"name":"ours","version":"0.0.0","license":null,"source":null}}
            ]}}"#
        )
    }

    const MAIN: &str = "[licenses]\nallow = [\"MIT\"]\n";
    const RUNE: &str = "[licenses]\nallow = [\"MIT\", \"MPL-2.0\"]\n";

    #[test]
    fn a_crate_is_judged_by_the_list_of_the_workspace_that_reaches_it() {
        let meta = metadata("MPL-2.0");
        let only_main = check(&[Scope {
            name: "main",
            metadata: &meta,
            deny_toml: MAIN,
        }])
        .unwrap();
        assert_eq!(only_main.violations.len(), 1);
        assert!(only_main.violations[0].contains("`MPL-2.0` is not in the allow-list"));

        let rune_list = check(&[Scope {
            name: "frontends/rune",
            metadata: &meta,
            deny_toml: RUNE,
        }])
        .unwrap();
        assert_eq!(rune_list.violations, Vec::<String>::new());
    }

    #[test]
    fn with_two_scopes_a_violation_names_its_workspace_and_every_crate_is_listed() {
        let ok = metadata("MIT");
        let bad = metadata("GPL-3.0-only");
        let out = check(&[
            Scope {
                name: "main",
                metadata: &ok,
                deny_toml: MAIN,
            },
            Scope {
                name: "frontends/rune",
                metadata: &bad,
                deny_toml: RUNE,
            },
        ])
        .unwrap();
        assert_eq!(out.counts, [1, 1]);
        assert_eq!(out.violations.len(), 1);
        assert!(out.violations[0].starts_with("frontends/rune: spellbook 0.3.0"));
        assert_eq!(
            out.listing,
            [
                "main: spellbook 0.3.0 MIT",
                "frontends/rune: spellbook 0.3.0 GPL-3.0-only"
            ]
        );
    }

    #[test]
    fn a_deny_toml_without_an_allow_list_is_an_error_naming_the_workspace() {
        let meta = metadata("MIT");
        let err = check(&[Scope {
            name: "frontends/rune",
            metadata: &meta,
            deny_toml: "",
        }])
        .unwrap_err();
        assert!(err.starts_with("frontends/rune: "), "{err}");
    }
}
