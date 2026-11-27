//! Git sources in manifests (E8.0.4, ADR-0027). The Rune workspace takes Rune by git revision, and
//! `cargo deny` allows that source only there. Two rules keep it honest in every `Cargo.toml` of
//! the repository: a dependency or patch that names a `git` source also names the `rev` it is pinned
//! at (not a branch, not a tag, not nothing), and all lines that name one git URL name one `rev`, so a
//! bump that misses a line fails here and not as two copies of a crate in the build.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use crate::scan;

/// One manifest: its path relative to the repository root and its text.
pub struct Manifest {
    pub path: String,
    pub text: String,
}

/// A broken git source rule.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// (file, place, git url): a git source with no `rev`.
    NoRev(String, String, String),
    /// (git url, the revisions): one URL, more than one `rev`.
    RevsDiffer(String, Vec<String>),
    /// (file, message): a manifest that does not parse cannot be read for sources.
    Unreadable(String, String),
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRev(file, place, url) => write!(
                f,
                "{file}: {place} names the git source {url} without `rev =`; pin it at a revision"
            ),
            Self::RevsDiffer(url, revs) => write!(
                f,
                "{url}: more than one revision in the manifests ({}); every line of one git \
                 source names the same `rev`",
                revs.join(", ")
            ),
            Self::Unreadable(file, e) => write!(f, "{file}: cannot read the manifest: {e}"),
        }
    }
}

const DEP_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

fn table(v: Option<&toml::Value>) -> Option<&toml::Table> {
    v.and_then(toml::Value::as_table)
}

fn add<'a>(out: &mut Vec<(String, &'a toml::Table)>, place: String, t: Option<&'a toml::Table>) {
    out.extend(t.map(|t| (place, t)));
}

/// The tables of a manifest that hold dependency specs, with the name of the place for messages.
fn spec_tables(doc: &toml::Table) -> Vec<(String, &toml::Table)> {
    let mut out = Vec::new();
    for name in DEP_TABLES {
        add(&mut out, name.to_string(), table(doc.get(name)));
    }
    let workspace = table(doc.get("workspace"));
    let shared = workspace.and_then(|w| table(w.get("dependencies")));
    add(&mut out, "workspace.dependencies".into(), shared);
    for (cfg, target) in table(doc.get("target")).into_iter().flatten() {
        for name in DEP_TABLES {
            let t = table(target.as_table().and_then(|t| t.get(name)));
            add(&mut out, format!("target.{cfg}.{name}"), t);
        }
    }
    for (registry, patches) in table(doc.get("patch")).into_iter().flatten() {
        add(&mut out, format!("patch.{registry}"), patches.as_table());
    }
    add(&mut out, "replace".into(), table(doc.get("replace")));
    out
}

/// The URL as cargo treats it for this purpose: no trailing `/` and no `.git`.
fn normal_url(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    url.strip_suffix(".git").unwrap_or(url).to_string()
}

/// Check every manifest.
pub fn check(manifests: &[Manifest]) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut revs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for m in manifests {
        let doc: toml::Table = match m.text.parse() {
            Ok(d) => d,
            Err(e) => {
                out.push(Violation::Unreadable(m.path.clone(), e.to_string()));
                continue;
            }
        };
        for (place, table) in spec_tables(&doc) {
            for (name, spec) in table {
                let Some(url) = spec.get("git").and_then(toml::Value::as_str) else {
                    continue;
                };
                let url = normal_url(url);
                match spec.get("rev").and_then(toml::Value::as_str) {
                    Some(rev) if !rev.trim().is_empty() => {
                        revs.entry(url).or_default().insert(rev.trim().to_string());
                    }
                    _ => out.push(Violation::NoRev(
                        m.path.clone(),
                        format!("{place}.{name}"),
                        url,
                    )),
                }
            }
        }
    }
    for (url, set) in revs {
        if set.len() > 1 {
            out.push(Violation::RevsDiffer(url, set.into_iter().collect()));
        }
    }
    out.sort();
    out
}

/// Every `Cargo.toml` under `root`, outside build output and hidden directories.
pub fn collect(root: &Path) -> Vec<Manifest> {
    scan::files(root, "", &["toml"])
        .into_iter()
        .filter(|p| p.rsplit('/').next() == Some("Cargo.toml"))
        .map(|path| Manifest {
            text: scan::read(root, &path),
            path,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(text: &str) -> Vec<Manifest> {
        vec![Manifest {
            path: "frontends/rune/Cargo.toml".into(),
            text: text.into(),
        }]
    }

    const PINNED: &str = r#"
[workspace.dependencies]
rune = { git = "https://example.test/rune", rev = "abc123" }
[patch.crates-io]
winit = { git = "https://example.test/rune", rev = "abc123" }
"#;

    #[test]
    fn a_git_source_pinned_at_one_revision_passes() {
        assert_eq!(check(&manifest(PINNED)), vec![]);
    }

    #[test]
    fn registry_and_path_dependencies_are_not_git_sources() {
        let text = "[dependencies]\nserde = \"1\"\nx = { path = \"../x\" }\n";
        assert_eq!(check(&manifest(text)), vec![]);
    }

    #[test]
    fn a_git_source_without_a_rev_fails_in_each_place_cargo_reads_one() {
        for text in [
            "[dependencies]\nr = { git = \"https://e.test/r\" }\n",
            "[dependencies]\nr = { git = \"https://e.test/r\", branch = \"dev\" }\n",
            "[dev-dependencies]\nr = { git = \"https://e.test/r\", tag = \"v1\" }\n",
            "[dependencies.r]\ngit = \"https://e.test/r\"\n",
            "[target.'cfg(unix)'.build-dependencies]\nr = { git = \"https://e.test/r\" }\n",
            "[workspace.dependencies]\nr = { git = \"https://e.test/r\" }\n",
            "[patch.crates-io]\nr = { git = \"https://e.test/r\" }\n",
            "[dependencies]\nr = { git = \"https://e.test/r\", rev = \"\" }\n",
        ] {
            let v = check(&manifest(text));
            assert_eq!(v.len(), 1, "{text}: {v:?}");
            assert!(matches!(v[0], Violation::NoRev(..)), "{text}");
            assert!(v[0].to_string().contains("without `rev =`"));
        }
    }

    #[test]
    fn two_revisions_of_one_url_fail_and_two_urls_do_not() {
        let two = format!(
            "{PINNED}[dev-dependencies]\nx = {{ git = \"https://example.test/rune.git/\", rev = \"def456\" }}\n"
        );
        let v = check(&manifest(&two));
        assert_eq!(
            v,
            vec![Violation::RevsDiffer(
                "https://example.test/rune".into(),
                vec!["abc123".into(), "def456".into()]
            )]
        );
        let other = format!(
            "{PINNED}[dev-dependencies]\nx = {{ git = \"https://other.test/x\", rev = \"def456\" }}\n"
        );
        assert_eq!(check(&manifest(&other)), vec![]);
    }

    #[test]
    fn revisions_are_compared_across_manifests() {
        let a = Manifest {
            path: "a/Cargo.toml".into(),
            text: "[dependencies]\nr = { git = \"https://e.test/r\", rev = \"1\" }\n".into(),
        };
        let b = Manifest {
            path: "b/Cargo.toml".into(),
            text: "[dependencies]\nr = { git = \"https://e.test/r\", rev = \"2\" }\n".into(),
        };
        assert_eq!(check(&[a, b]).len(), 1);
    }

    #[test]
    fn a_manifest_that_does_not_parse_fails() {
        let v = check(&manifest("[dependencies\n"));
        assert!(matches!(v[0], Violation::Unreadable(..)));
    }

    #[test]
    fn collect_finds_only_files_named_cargo_toml() {
        let dir = std::env::temp_dir().join(format!("xtask-gitsrc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in [
            ("Cargo.toml", "[workspace]\n"),
            ("frontends/rune/crates/a/Cargo.toml", "[package]\n"),
            ("deny.toml", "x = 1\n"),
            ("frontends/rune/target/debug/Cargo.toml", "[package]\n"),
        ] {
            let full = dir.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, text).unwrap();
        }
        let mut paths: Vec<String> = collect(&dir).into_iter().map(|m| m.path).collect();
        std::fs::remove_dir_all(&dir).unwrap();
        paths.sort();
        // The Rune build output is skipped like any other target directory.
        assert_eq!(paths, ["Cargo.toml", "frontends/rune/crates/a/Cargo.toml"]);
    }
}
