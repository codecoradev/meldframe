//! `size-check` (D1): 500 lines per file, 2,000 per crate. An allow-list records what is over
//! today and an entry must equal the current size, so it can only go down.

use std::collections::BTreeMap;
use std::fmt;

pub const FILE_LIMIT: usize = 500;
pub const CRATE_LIMIT: usize = 2000;

/// Parsed `xtask/size-allow.toml`: path or crate name to the line count allowed today.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allow {
    pub files: BTreeMap<String, usize>,
    pub crates: BTreeMap<String, usize>,
}

/// A source file and its line count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub path: String,
    pub lines: usize,
}

/// A crate with its source files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sized {
    pub name: String,
    pub files: Vec<Source>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// Over the ceiling with no allow-list entry.
    OverLimit {
        what: String,
        lines: usize,
        limit: usize,
    },
    /// Larger than its allow-list entry.
    Grew {
        what: String,
        lines: usize,
        allowed: usize,
    },
    /// Entry is higher than the current size: lower it to the size.
    EntryNotLowered {
        what: String,
        lines: usize,
        allowed: usize,
    },
    /// Entry for something now within the ceiling or gone: remove it.
    StaleEntry { what: String, allowed: usize },
    /// Compared with the base list: an entry was added or raised.
    EntryRaised {
        what: String,
        old: Option<usize>,
        new: usize,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::OverLimit { what, lines, limit } => {
                write!(f, "{what}: {lines} lines, over the {limit} ceiling")
            }
            Violation::Grew {
                what,
                lines,
                allowed,
            } => {
                write!(
                    f,
                    "{what}: {lines} lines, grew past its allow-list entry {allowed}"
                )
            }
            Violation::EntryNotLowered {
                what,
                lines,
                allowed,
            } => write!(
                f,
                "{what}: allow-list entry {allowed} is above the current {lines}; lower it"
            ),
            Violation::StaleEntry { what, allowed } => write!(
                f,
                "{what}: allow-list entry {allowed} is no longer needed; remove it"
            ),
            Violation::EntryRaised { what, old, new } => match old {
                Some(o) => write!(f, "{what}: allow-list entry raised from {o} to {new}"),
                None => write!(
                    f,
                    "{what}: new allow-list entry ({new}); entries only go down"
                ),
            },
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
        let entries = value
            .as_table()
            .ok_or("size allow-list: expected a table")?;
        for (key, v) in entries {
            let n = v
                .as_integer()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| format!("size allow-list: `{key}` must be a line count"))?;
            target.insert(key.clone(), n);
        }
    }
    Ok(allow)
}

/// One measured item against its ceiling and optional entry.
fn judge(
    what: &str,
    lines: Option<usize>,
    limit: usize,
    entry: Option<usize>,
) -> Option<Violation> {
    let what = what.to_string();
    match (lines, entry) {
        (Some(l), None) if l > limit => Some(Violation::OverLimit {
            what,
            lines: l,
            limit,
        }),
        (Some(l), Some(e)) if l > limit && l > e => Some(Violation::Grew {
            what,
            lines: l,
            allowed: e,
        }),
        (Some(l), Some(e)) if l > limit && l < e => Some(Violation::EntryNotLowered {
            what,
            lines: l,
            allowed: e,
        }),
        (_, Some(e)) if lines.is_none_or(|l| l <= limit) => {
            Some(Violation::StaleEntry { what, allowed: e })
        }
        _ => None,
    }
}

/// Check every crate and file against the ceilings and the allow-list. An entry must equal the
/// current size, so growing or leaving slack both fail and the list can only shrink.
pub fn check(crates: &[Sized], allow: &Allow) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut seen_files = Vec::new();
    for c in crates {
        let total: usize = c.files.iter().map(|f| f.lines).sum();
        out.extend(judge(
            &c.name,
            Some(total),
            CRATE_LIMIT,
            allow.crates.get(&c.name).copied(),
        ));
        for f in &c.files {
            seen_files.push(f.path.as_str());
            out.extend(judge(
                &f.path,
                Some(f.lines),
                FILE_LIMIT,
                allow.files.get(&f.path).copied(),
            ));
        }
    }
    let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
    let missing = allow
        .files
        .iter()
        .filter(|(k, _)| !seen_files.contains(&k.as_str()))
        .chain(
            allow
                .crates
                .iter()
                .filter(|(k, _)| !names.contains(&k.as_str())),
        );
    out.extend(missing.map(|(k, e)| Violation::StaleEntry {
        what: k.clone(),
        allowed: *e,
    }));
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
                out.push(Violation::EntryRaised {
                    what: k.clone(),
                    old,
                    new: n,
                });
            }
        }
    }
    out.sort();
    out
}

/// Source extensions that count towards the ceilings.
pub const EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "css"];

/// Path of the allow-list, relative to the workspace root.
pub const ALLOW_FILE: &str = "xtask/size-allow.toml";

/// Measure every package's source files.
pub fn measure(root: &std::path::Path, packages: &[crate::rules::Package]) -> Vec<Sized> {
    packages
        .iter()
        .map(|p| Sized {
            name: p.name.clone(),
            files: crate::scan::files(root, &p.dir, EXTENSIONS)
                .into_iter()
                .map(|path| {
                    let lines = crate::scan::read(root, &path).lines().count();
                    Source { path, lines }
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, files: &[(&str, usize)]) -> Sized {
        Sized {
            name: name.into(),
            files: files
                .iter()
                .map(|(p, l)| Source {
                    path: (*p).into(),
                    lines: *l,
                })
                .collect(),
        }
    }

    fn allow(files: &[(&str, usize)], crates: &[(&str, usize)]) -> Allow {
        Allow {
            files: files.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
            crates: crates.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
        }
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
            vec![Violation::OverLimit {
                what: "a/x.rs".into(),
                lines: 501,
                limit: 500
            }]
        );
    }

    #[test]
    fn crate_over_2000_without_entry_fails() {
        let files: Vec<(String, usize)> = (0..5).map(|i| (format!("a/{i}.rs"), 410)).collect();
        let refs: Vec<(&str, usize)> = files.iter().map(|(p, l)| (p.as_str(), *l)).collect();
        let v = check(&[krate("a", &refs)], &Allow::default());
        assert_eq!(
            v,
            vec![Violation::OverLimit {
                what: "a".into(),
                lines: 2050,
                limit: 2000
            }]
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
            vec![Violation::Grew {
                what: "a/x.rs".into(),
                lines: 610,
                allowed: 600
            }]
        );
    }

    #[test]
    fn an_entry_above_the_current_size_fails_so_it_must_be_lowered() {
        let c = krate("a", &[("a/x.rs", 550)]);
        assert_eq!(
            check(&[c], &allow(&[("a/x.rs", 600)], &[])),
            vec![Violation::EntryNotLowered {
                what: "a/x.rs".into(),
                lines: 550,
                allowed: 600
            }]
        );
    }

    #[test]
    fn entry_for_a_file_back_under_the_limit_or_missing_is_stale() {
        let c = krate("a", &[("a/x.rs", 400)]);
        let v = check(
            &[c],
            &allow(&[("a/x.rs", 600), ("a/gone.rs", 700)], &[("a", 3000)]),
        );
        assert_eq!(v.len(), 3);
        assert!(v.iter().all(|v| matches!(v, Violation::StaleEntry { .. })));
    }

    #[test]
    fn crate_entries_follow_the_same_rules() {
        let files: Vec<(String, usize)> = (0..5).map(|i| (format!("a/{i}.rs"), 410)).collect();
        let refs: Vec<(&str, usize)> = files.iter().map(|(p, l)| (p.as_str(), *l)).collect();
        let c = krate("a", &refs);
        assert_eq!(
            check(std::slice::from_ref(&c), &allow(&[], &[("a", 2050)])),
            vec![]
        );
        assert_eq!(check(&[c], &allow(&[], &[("a", 2100)])).len(), 1);
    }

    #[test]
    fn raising_an_entry_against_the_base_list_fails() {
        let base = allow(&[("a/x.rs", 600)], &[]);
        let raised = allow(&[("a/x.rs", 650)], &[]);
        assert_eq!(
            check_not_raised(&base, &raised),
            vec![Violation::EntryRaised {
                what: "a/x.rs".into(),
                old: Some(600),
                new: 650
            }]
        );
    }

    #[test]
    fn adding_an_entry_against_the_base_list_fails() {
        let v = check_not_raised(&Allow::default(), &allow(&[("a/new.rs", 501)], &[]));
        assert_eq!(
            v,
            vec![Violation::EntryRaised {
                what: "a/new.rs".into(),
                old: None,
                new: 501
            }]
        );
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
