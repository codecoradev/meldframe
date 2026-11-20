//! `size-check` (D1): 500 lines per file; per crate an ideal band of 1,500 to 2,500 lines and a
//! hard ceiling of 4,000 (owner decision, 20 Nov 2026). Only the ceiling fails the check; a crate
//! above 2,500 is reported as a candidate for a planned split. An allow-list records what is over
//! a ceiling today and an entry must equal the current size, so it can only go down.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use crate::rules::Package;
use crate::scan;

pub const FILE_LIMIT: usize = 500;
/// Hard ceiling of a crate: above it the check fails, and at it the split comes before the next
/// addition.
pub const CRATE_LIMIT: usize = 4000;
/// Bottom of the ideal band of a crate; only the note's text uses it.
pub const CRATE_IDEAL_LOW: usize = 1500;
/// Top of the ideal band (1,500 to 2,500): above it a crate is a candidate for a planned split.
/// Not a failure.
pub const CRATE_SOFT: usize = 2500;
/// Source extensions that count towards the ceilings.
pub const EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "mjs", "css", "scss", "svelte", "html",
];
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

/// Crates above the soft value, largest first (ties by name): candidates for a planned split.
pub fn split_candidates(crates: &[Sized]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = crates
        .iter()
        .map(|(name, files)| (name.clone(), files.iter().map(|f| f.1).sum()))
        .filter(|(_, total)| *total > CRATE_SOFT)
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// A line count with a comma between thousands: 4000 is `4,000`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The note printed after the result line: `None` when no crate is above the soft value.
pub fn split_note(candidates: &[(String, usize)]) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let n = candidates.len();
    let mut note = format!(
        "size-check: note: {n} crate{} above {soft} lines (ideal band {low} to {soft}; hard \
         ceiling {hard}), candidates for a planned split:",
        if n == 1 { "" } else { "s" },
        low = thousands(CRATE_IDEAL_LOW),
        soft = thousands(CRATE_SOFT),
        hard = thousands(CRATE_LIMIT),
    );
    for (name, lines) in candidates {
        note.push_str(&format!("\n  {name}: {lines} lines"));
    }
    Some(note)
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

/// The value of `--base` in the arguments of `size-check`: `None` without the flag, an error when
/// the flag has no value (last argument, or followed by another flag).
pub fn base_arg(args: &[String]) -> Result<Option<&str>, String> {
    let value = match args
        .iter()
        .position(|a| a == "--base" || a.starts_with("--base="))
    {
        None => return Ok(None),
        Some(i) => match args[i].strip_prefix("--base=") {
            Some(v) => Some(v),
            None => args.get(i + 1).map(String::as_str),
        },
    };
    match value {
        Some(v) if !v.starts_with("--") && !v.is_empty() => Ok(Some(v)),
        _ => Err("size-check: --base needs a git ref, e.g. --base origin/develop".into()),
    }
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

    /// A crate of `n` files of 400 lines each, so only the crate total can be over a limit.
    fn crate_of(name: &str, n: usize) -> Sized {
        let files: Vec<(String, usize)> = (0..n).map(|i| (format!("{name}/{i}.rs"), 400)).collect();
        (name.into(), files)
    }

    /// 11 files of 400 lines: 4,400 lines, over the 4,000 crate ceiling only.
    fn big() -> Sized {
        crate_of("a", 11)
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
    fn crate_over_4000_without_entry_fails() {
        assert_eq!(
            check(&[big()], &Allow::default()),
            vec![OverLimit("a".into(), 4400, 4000)]
        );
    }

    #[test]
    fn crate_between_the_ideal_band_and_the_ceiling_passes() {
        // 2,800 lines: above the 2,500 soft value, under the 4,000 hard ceiling.
        assert_eq!(check(&[crate_of("a", 7)], &Allow::default()), vec![]);
        // 4,000 lines exactly is within the ceiling.
        assert_eq!(check(&[crate_of("a", 10)], &Allow::default()), vec![]);
    }

    #[test]
    fn thousands_are_grouped_with_a_comma() {
        assert_eq!(thousands(500), "500");
        assert_eq!(thousands(1500), "1,500");
        assert_eq!(thousands(4000), "4,000");
        assert_eq!(thousands(12_345), "12,345");
    }

    #[test]
    fn limits_are_the_owner_decision_of_20_nov_2026() {
        assert_eq!(
            (FILE_LIMIT, CRATE_IDEAL_LOW, CRATE_SOFT, CRATE_LIMIT),
            (500, 1500, 2500, 4000)
        );
    }

    #[test]
    fn split_candidates_list_crates_above_2500_largest_first() {
        let sizes = [
            crate_of("exact", 6), // 2,400
            crate_of("b", 7),     // 2,800
            crate_of("c", 10),    // 4,000
            crate_of("d", 7),     // 2,800
            krate("e", &[("e/x.rs", 500)]),
        ];
        assert_eq!(
            split_candidates(&sizes),
            vec![
                ("c".to_string(), 4000),
                ("b".to_string(), 2800),
                ("d".to_string(), 2800)
            ]
        );
        // Exactly 2,500 is inside the band, not a candidate.
        let edge = krate("edge", &[("edge/a.rs", 500); 5]);
        assert_eq!(split_candidates(&[edge]), vec![]);
    }

    /// A crate of 500-line files plus one line, so its total is `500 * full + 1`.
    fn one_over(name: &str, full: usize) -> Sized {
        let mut files: Vec<(String, usize)> =
            (0..full).map(|i| (format!("{name}/{i}.rs"), 500)).collect();
        files.push((format!("{name}/last.rs"), 1));
        (name.into(), files)
    }

    #[test]
    fn a_crate_of_2501_lines_is_a_split_candidate_but_not_a_violation() {
        let c = one_over("a", 5);
        assert_eq!(
            split_candidates(std::slice::from_ref(&c)),
            vec![("a".to_string(), 2501)]
        );
        assert_eq!(check(&[c], &Allow::default()), vec![]);
    }

    #[test]
    fn a_crate_of_4001_lines_fails_without_an_entry() {
        assert_eq!(
            check(&[one_over("a", 8)], &Allow::default()),
            vec![OverLimit("a".into(), 4001, 4000)]
        );
    }

    #[test]
    fn split_note_is_silent_without_candidates() {
        assert_eq!(split_note(&[]), None);
    }

    #[test]
    fn split_note_names_each_candidate_and_is_not_a_violation() {
        let note = split_note(&[("c".into(), 4000), ("b".into(), 2800)]).unwrap();
        assert!(note.starts_with("size-check: note: 2 crates above 2,500 lines"));
        assert!(note.contains("ideal band 1,500 to 2,500; hard ceiling 4,000"));
        assert!(note.contains("  c: 4000 lines"));
        assert!(note.contains("  b: 2800 lines"));
        assert!(note.contains("planned split"));
        let one = split_note(&[("c".into(), 2600)]).unwrap();
        assert!(one.starts_with("size-check: note: 1 crate above 2,500 lines"));
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
        // An entry only makes sense above the 4,000 ceiling: it equals the size, or it is wrong.
        assert_eq!(check(&[big()], &allow(&[], &[("a", 4400)])), vec![]);
        assert_eq!(check(&[big()], &allow(&[], &[("a", 4500)])).len(), 1);
    }

    #[test]
    fn a_crate_entry_for_a_crate_under_the_ceiling_is_stale() {
        // meldframe-web was 2,089 under the old 2,000 ceiling; it is within 4,000 now.
        let c = crate_of("a", 6);
        assert_eq!(
            check(&[c], &allow(&[], &[("a", 2400)])),
            vec![Stale("a".into(), 2400)]
        );
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
    fn web_client_sources_count_towards_the_ceilings() {
        for ext in ["svelte", "mjs", "scss", "html", "ts", "css"] {
            assert!(EXTENSIONS.contains(&ext), "{ext} is not measured");
        }
    }

    #[test]
    fn base_needs_a_value() {
        let a = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(base_arg(&a(&["size-check"])), Ok(None));
        assert_eq!(
            base_arg(&a(&["size-check", "--base", "origin/develop"])),
            Ok(Some("origin/develop"))
        );
        assert!(base_arg(&a(&["size-check", "--base"])).is_err());
        assert!(base_arg(&a(&["size-check", "--base", "--other"])).is_err());
        assert!(base_arg(&a(&["size-check", "--base", ""])).is_err());
        assert_eq!(base_arg(&a(&["x", "--base=o/d"])), Ok(Some("o/d")));
        assert!(base_arg(&a(&["x", "--base="])).is_err());
    }

    #[test]
    fn allow_list_parses_both_tables_and_rejects_bad_values() {
        let a = parse_allow("[files]\n\"a/x.rs\" = 600\n[crates]\na = 4100\n").unwrap();
        assert_eq!(a, allow(&[("a/x.rs", 600)], &[("a", 4100)]));
        assert_eq!(parse_allow("").unwrap(), Allow::default());
        assert!(parse_allow("[files]\n\"a\" = \"big\"\n").is_err());
        assert!(parse_allow("[files\n").is_err());
        assert!(parse_allow("[sizes]\nx = 1\n").is_err());
    }
}
