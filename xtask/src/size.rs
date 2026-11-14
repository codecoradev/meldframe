//! `size-check` (D1): 500 lines per file, 2,000 per crate. An allow-list records what is over
//! today and an entry must equal the current size, so it can only go down.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::rules::Package;
use crate::scan;

pub const FILE_LIMIT: usize = 500;
pub const CRATE_LIMIT: usize = 2000;
/// Source extensions that count towards the ceilings.
pub const EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "css"];
/// Path of the allow-list, relative to the workspace root.
pub const ALLOW_FILE: &str = "xtask/size-allow.toml";

/// Parsed `xtask/size-allow.toml`: path or crate name to the line count allowed today.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allow {
    pub files: BTreeMap<String, usize>,
    pub crates: BTreeMap<String, usize>,
}

/// A crate name with its source files as (path, line count).
pub type Sized = (String, Vec<(String, usize)>);

/// A broken ceiling or allow-list rule. The first field is the file path or crate name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// (what, lines, limit): over the ceiling with no allow-list entry.
    OverLimit(String, usize, usize),
    /// (what, lines, entry): larger than its allow-list entry.
    Grew(String, usize, usize),
    /// (what, lines, entry): entry above the current size; lower it.
    NotLowered(String, usize, usize),
    /// (what, entry): entry for something within the ceiling or gone; remove it.
    Stale(String, usize),
    /// (what, old, new): compared with the base list, an entry was added or raised.
    Raised(String, Option<usize>, usize),
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverLimit(w, l, max) => write!(f, "{w}: {l} lines, over the {max} ceiling"),
            Self::Grew(w, l, e) => write!(f, "{w}: {l} lines, grew past its allow-list entry {e}"),
            Self::NotLowered(w, l, e) => {
                write!(f, "{w}: allow-list entry {e} is above {l}; lower it")
            }
            Self::Stale(w, e) => write!(f, "{w}: allow-list entry {e} is not needed; remove it"),
            Self::Raised(w, Some(o), n) => {
                write!(f, "{w}: allow-list entry raised from {o} to {n}")
            }
            Self::Raised(w, None, n) => {
                write!(f, "{w}: new allow-list entry ({n}); entries only go down")
            }
        }
    }
}

/// Parse the allow-list file: tables `[files]` and `[crates]` of name = line count.
pub fn parse_allow(text: &str) -> Result<Allow, String> {
    let root: toml::Table = text.parse().map_err(|e| format!("size allow-list: {e}"))?;
    let mut allow = Allow::default();
    for (table, value) in &root {
        let target = match table.as_str() {
            "files" => &mut allow.files,
            "crates" => &mut allow.crates,
            other => return Err(format!("size allow-list: unknown table [{other}]")),
        };
        for (key, v) in value
            .as_table()
            .ok_or("size allow-list: expected a table")?
        {
            let n = v.as_integer().and_then(|n| usize::try_from(n).ok());
            target.insert(
                key.clone(),
                n.ok_or(format!("size allow-list: `{key}` must be a line count"))?,
            );
        }
    }
    Ok(allow)
}

/// One measured item against its ceiling and optional entry.
fn judge(what: &str, lines: usize, limit: usize, entry: Option<usize>) -> Option<Violation> {
    let w = what.to_string();
    match entry {
        None if lines > limit => Some(Violation::OverLimit(w, lines, limit)),
        Some(e) if lines <= limit => Some(Violation::Stale(w, e)),
        Some(e) if lines > e => Some(Violation::Grew(w, lines, e)),
        Some(e) if lines < e => Some(Violation::NotLowered(w, lines, e)),
        _ => None,
    }
}

/// Check every crate and file against the ceilings and the allow-list. An entry must equal the
/// current size, so growing or leaving slack both fail and the list can only shrink.
pub fn check(crates: &[Sized], allow: &Allow) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for (name, files) in crates {
        let total = files.iter().map(|f| f.1).sum();
        out.extend(judge(
            name,
            total,
            CRATE_LIMIT,
            allow.crates.get(name).copied(),
        ));
        for (path, lines) in files {
            seen.push(path.as_str());
            out.extend(judge(
                path,
                *lines,
                FILE_LIMIT,
                allow.files.get(path).copied(),
            ));
        }
    }
    let gone = |(k, e): (&String, &usize)| Violation::Stale(k.clone(), *e);
    out.extend(
        allow
            .files
            .iter()
            .filter(|(k, _)| !seen.contains(&k.as_str()))
            .map(gone),
    );
    out.extend(
        allow
            .crates
            .iter()
            .filter(|(k, _)| !crates.iter().any(|c| c.0 == **k))
            .map(gone),
    );
    out.sort();
    out
}

/// Compare with the allow-list on the base branch: nothing may be added or raised.
pub fn check_not_raised(base: &Allow, now: &Allow) -> Vec<Violation> {
    let mut out = Vec::new();
    for (now_map, base_map) in [(&now.files, &base.files), (&now.crates, &base.crates)] {
        for (k, &n) in now_map {
            let old = base_map.get(k).copied();
            if old.is_none_or(|o| n > o) {
                out.push(Violation::Raised(k.clone(), old, n));
            }
        }
    }
    out.sort();
    out
}

/// Measure every package's source files.
pub fn measure(root: &Path, packages: &[Package]) -> Vec<Sized> {
    let count = |path: String| {
        let lines = scan::read(root, &path).lines().count();
        (path, lines)
    };
    let files = |p: &Package| {
        scan::files(root, &p.dir, EXTENSIONS)
            .into_iter()
            .map(count)
            .collect()
    };
    packages
        .iter()
        .map(|p| (p.name.clone(), files(p)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use Violation::*;

    fn krate(name: &str, files: &[(&str, usize)]) -> Sized {
        (
            name.into(),
            files.iter().map(|(p, l)| ((*p).into(), *l)).collect(),
        )
    }

    fn allow(files: &[(&str, usize)], crates: &[(&str, usize)]) -> Allow {
        let map = |m: &[(&str, usize)]| m.iter().map(|(k, v)| ((*k).into(), *v)).collect();
        Allow {
            files: map(files),
            crates: map(crates),
        }
    }

    /// A crate of five 410-line files: 2,050 lines, over the crate ceiling only.
    fn big() -> Sized {
        let files: Vec<(String, usize)> = (0..5).map(|i| (format!("a/{i}.rs"), 410)).collect();
        ("a".into(), files)
    }

    #[test]
    fn within_limits_passes() {
        let c = krate("a", &[("a/x.rs", 500), ("a/y.rs", 10)]);
        assert_eq!(check(&[c], &Allow::default()), vec![]);
    }

    #[test]
    fn file_over_500_without_entry_fails() {
        let c = krate("a", &[("a/x.rs", 501)]);
        assert_eq!(
            check(&[c], &Allow::default()),
            vec![OverLimit("a/x.rs".into(), 501, 500)]
        );
    }

    #[test]
    fn crate_over_2000_without_entry_fails() {
        assert_eq!(
            check(&[big()], &Allow::default()),
            vec![OverLimit("a".into(), 2050, 2000)]
        );
    }

    #[test]
    fn entry_equal_to_size_is_accepted() {
        let c = krate("a", &[("a/x.rs", 600)]);
        assert_eq!(check(&[c], &allow(&[("a/x.rs", 600)], &[])), vec![]);
    }

    #[test]
    fn growing_past_the_entry_fails() {
        let c = krate("a", &[("a/x.rs", 610)]);
        assert_eq!(
            check(&[c], &allow(&[("a/x.rs", 600)], &[])),
            vec![Grew("a/x.rs".into(), 610, 600)]
        );
    }

    #[test]
    fn an_entry_above_the_current_size_fails_so_it_must_be_lowered() {
        let c = krate("a", &[("a/x.rs", 550)]);
        let v = check(&[c], &allow(&[("a/x.rs", 600)], &[]));
        assert_eq!(v, vec![NotLowered("a/x.rs".into(), 550, 600)]);
    }

    #[test]
    fn entry_for_a_file_back_under_the_limit_or_missing_is_stale() {
        let c = krate("a", &[("a/x.rs", 400)]);
        let v = check(
            &[c],
            &allow(&[("a/x.rs", 600), ("a/gone.rs", 700)], &[("a", 3000)]),
        );
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|v| matches!(v, Stale(..))));
    }

    #[test]
    fn crate_entries_follow_the_same_rules() {
        assert_eq!(check(&[big()], &allow(&[], &[("a", 2050)])), vec![]);
        assert_eq!(check(&[big()], &allow(&[], &[("a", 2100)])).len(), 1);
    }

    #[test]
    fn raising_an_entry_against_the_base_list_fails() {
        let v = check_not_raised(
            &allow(&[("a/x.rs", 600)], &[]),
            &allow(&[("a/x.rs", 650)], &[]),
        );
        assert_eq!(v, vec![Raised("a/x.rs".into(), Some(600), 650)]);
    }

    #[test]
    fn adding_an_entry_against_the_base_list_fails() {
        let v = check_not_raised(&Allow::default(), &allow(&[("a/new.rs", 501)], &[]));
        assert_eq!(v, vec![Raised("a/new.rs".into(), None, 501)]);
    }

    #[test]
    fn lowering_or_removing_entries_against_the_base_list_passes() {
        let base = allow(&[("a/x.rs", 600), ("a/y.rs", 700)], &[("a", 2500)]);
        let now = allow(&[("a/x.rs", 550)], &[("a", 2400)]);
        assert_eq!(check_not_raised(&base, &now), vec![]);
    }

    #[test]
    fn allow_list_parses_both_tables_and_rejects_bad_values() {
        let a = parse_allow("[files]\n\"a/x.rs\" = 600\n[crates]\na = 2100\n").unwrap();
        assert_eq!(a, allow(&[("a/x.rs", 600)], &[("a", 2100)]));
        assert_eq!(parse_allow("").unwrap(), Allow::default());
        assert!(parse_allow("[files]\n\"a\" = \"big\"\n").is_err());
        assert!(parse_allow("[files\n").is_err());
        assert!(parse_allow("[sizes]\nx = 1\n").is_err());
    }
}
