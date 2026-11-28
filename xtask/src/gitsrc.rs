//! Git sources in manifests (E8.0.4, ADR-0027). The Rune workspace takes Rune by git revision, and
//! `cargo deny` allows that source only there. Two rules keep it honest in every `Cargo.toml` of
//! the repository: a dependency or patch that names a `git` source also names the `rev` it is pinned
//! at, as a full 40-character commit id (not a branch, a tag, a short id or nothing), and all lines
//! that name one git URL, however it is spelled, name one `rev`, so a bump that misses a line fails
//! here and not as two copies of a crate in the build. The Rune repository may be named only in
//! `frontends/rune/` (a second guard: the main workspace never depends on it).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use crate::scan;
use crate::workspaces::RUNE_DIR;

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
    /// (file, place, git url, rev): a `rev` that is not a full 40-character commit id.
    ShortRev(String, String, String, String),
    /// (file, place): the Rune repository named outside `frontends/rune/`.
    RuneOutside(String, String),
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
            Self::ShortRev(file, place, url, rev) => write!(
                f,
                "{file}: {place} pins {url} at `{rev}`; `rev =` must be a full 40-character commit id"
            ),
            Self::RuneOutside(file, place) => write!(
                f,
                "{file}: {place} names the Rune repository; only {RUNE_DIR}/ may (ADR-0027)"
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

/// One spelling of a git URL: lower case, `https`, no trailing `/` and no `.git`, so that
/// `http://GitHub.com/A/R.git/` and `https://github.com/a/r` are the same source here.
fn normal_url(url: &str) -> String {
    let lower = url.trim().to_ascii_lowercase();
    let url = lower.trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    }
}

/// A full commit id: 40 hexadecimal digits.
fn is_full_rev(rev: &str) -> bool {
    rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The Rune repository, as `normal_url` writes it.
const RUNE_URL: &str = "https://github.com/anomalyco/rune";

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
                let place = format!("{place}.{name}");
                if url == RUNE_URL && !m.path.starts_with(&format!("{RUNE_DIR}/")) {
                    out.push(Violation::RuneOutside(m.path.clone(), place.clone()));
                }
                match spec.get("rev").and_then(toml::Value::as_str).map(str::trim) {
                    Some(rev) if is_full_rev(rev) => {
                        revs.entry(url)
                            .or_default()
                            .insert(rev.to_ascii_lowercase());
                    }
                    Some(rev) if !rev.is_empty() => out.push(Violation::ShortRev(
                        m.path.clone(),
                        place,
                        url,
                        rev.to_string(),
                    )),
                    _ => out.push(Violation::NoRev(m.path.clone(), place, url)),
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

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    const PINNED: &str = r#"
[workspace.dependencies]
rune = { git = "https://example.test/rune", rev = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
[patch.crates-io]
winit = { git = "https://example.test/rune", rev = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
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
            "{PINNED}[dev-dependencies]\nx = {{ git = \"https://example.test/rune.git/\", rev = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\" }}\n"
        );
        let v = check(&manifest(&two));
        assert_eq!(
            v,
            vec![Violation::RevsDiffer(
                "https://example.test/rune".into(),
                vec![A.into(), B.into()]
            )]
        );
        let other = format!(
            "{PINNED}[dev-dependencies]\nx = {{ git = \"https://other.test/x\", rev = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\" }}\n"
        );
        assert_eq!(check(&manifest(&other)), vec![]);
    }

    #[test]
    fn revisions_are_compared_across_manifests() {
        let a = Manifest {
            path: "a/Cargo.toml".into(),
            text: "[dependencies]\nr = { git = \"https://e.test/r\", rev = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" }\n".into(),
        };
        let b = Manifest {
            path: "b/Cargo.toml".into(),
            text: "[dependencies]\nr = { git = \"https://e.test/r\", rev = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\" }\n".into(),
        };
        assert_eq!(check(&[a, b]).len(), 1);
    }

    #[test]
    fn a_rev_that_is_not_a_full_commit_id_fails() {
        for rev in [
            "main",
            "048528dc35f0",
            "v1.0",
            &A[..39],
            &format!("{A}0"),
            &"g".repeat(40),
        ] {
            let text =
                format!("[dependencies]\nr = {{ git = \"https://e.test/r\", rev = \"{rev}\" }}\n");
            let v = check(&manifest(&text));
            assert_eq!(v.len(), 1, "{rev}: {v:?}");
            assert!(matches!(v[0], Violation::ShortRev(..)), "{rev}");
            assert!(v[0].to_string().contains("full 40-character"), "{rev}");
        }
        // Upper case digits are a full id, and the same id in either case is one revision.
        let upper = format!(
            "[dependencies]\nr = {{ git = \"https://e.test/r\", rev = \"{}\" }}\n",
            A.to_uppercase()
        );
        let lower =
            format!("[dev-dependencies]\nr = {{ git = \"https://e.test/r\", rev = \"{A}\" }}\n");
        assert_eq!(check(&manifest(&format!("{upper}{lower}"))), vec![]);
    }

    #[test]
    fn two_spellings_of_one_url_are_one_source() {
        let text = format!(
            "[dependencies]\nr = {{ git = \"https://github.com/AnomalyCo/Rune.git\", rev = \"{A}\" }}\n\
             [dev-dependencies]\nr = {{ git = \"{plain}://github.com/anomalyco/rune/\", rev = \"{B}\" }}\n",
            plain = "http"
        );
        let v = check(&manifest(&text));
        assert_eq!(
            v,
            vec![Violation::RevsDiffer(
                "https://github.com/anomalyco/rune".into(),
                vec![A.into(), B.into()]
            )]
        );
        assert_eq!(
            normal_url("HTTP://Example.TEST/X.git/"),
            "https://example.test/x"
        );
    }

    #[test]
    fn the_rune_repository_may_be_named_only_under_frontends_rune() {
        let text = format!(
            "[dependencies]\nr = {{ git = \"https://github.com/AnomalyCo/Rune\", rev = \"{A}\" }}\n"
        );
        let outside = Manifest {
            path: "crates/ui/x/Cargo.toml".into(),
            text: text.clone(),
        };
        let v = check(&[outside]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(matches!(v[0], Violation::RuneOutside(..)));
        assert!(v[0].to_string().contains("only frontends/rune/ may"));
        let inside = Manifest {
            path: "frontends/rune/crates/x/Cargo.toml".into(),
            text,
        };
        assert_eq!(check(&[inside]), vec![]);
        // A repository whose name only starts like Rune's is another source.
        let other = Manifest {
            path: "crates/ui/x/Cargo.toml".into(),
            text: format!(
                "[dependencies]\nr = {{ git = \"https://github.com/anomalyco/rune-docs\", rev = \"{A}\" }}\n"
            ),
        };
        assert_eq!(check(&[other]), vec![]);
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
            std::fs::create_dir_all(full.parent().expect("a parent")).expect("a directory");
            std::fs::write(full, text).expect("a file");
        }
        let mut paths: Vec<String> = collect(&dir).into_iter().map(|m| m.path).collect();
        std::fs::remove_dir_all(&dir).expect("the tree");
        paths.sort();
        // The Rune build output is skipped like any other target directory.
        assert_eq!(paths, ["Cargo.toml", "frontends/rune/crates/a/Cargo.toml"]);
    }
}
