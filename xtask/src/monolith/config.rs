//! `xtask/monolith.toml`: the threshold, the shared-helper rules and the allow-lists of `mm-check`.

use std::collections::BTreeMap;

use super::Violation;

/// Path of the configuration, relative to the workspace root.
pub const CONFIG_FILE: &str = "xtask/monolith.toml";

/// A `[shared.<name>]` rule: a pattern that belongs in one home.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shared {
    /// File or directory (workspace-relative) where the pattern may be written.
    pub home: String,
    /// What to say to the author: `use meldframe_text::is_hidden_text`.
    pub advice: String,
    /// Token sequences, written as Rust source.
    pub ban: Vec<String>,
}

/// A group of copies that is allowed today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowCopy {
    /// `path::fn` of every member, sorted.
    pub fns: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Crates above this many lines (the measure of `size-check`) are monoliths.
    pub threshold: usize,
    pub shared: BTreeMap<String, Shared>,
    pub allow_copy: Vec<AllowCopy>,
    /// Rule name to file path to reason.
    pub allow_shared: BTreeMap<String, BTreeMap<String, String>>,
    /// Crate name to reason.
    pub allow_test_crate: BTreeMap<String, String>,
}

fn string(v: &toml::Value, what: &str) -> Result<String, String> {
    match v.as_str() {
        Some(s) if !s.trim().is_empty() => Ok(s.to_string()),
        _ => Err(format!("monolith.toml: {what} must be a non-empty string")),
    }
}

fn table<'a>(v: &'a toml::Value, what: &str) -> Result<&'a toml::Table, String> {
    v.as_table()
        .ok_or_else(|| format!("monolith.toml: {what} must be a table"))
}

fn reasons(v: &toml::Value, what: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (k, r) in table(v, what)? {
        out.insert(
            k.clone(),
            string(r, &format!("the reason of `{k}` in {what}"))?,
        );
    }
    Ok(out)
}

/// Parse the configuration. Every allow-list entry needs a reason, and the threshold must be set.
pub fn parse(text: &str) -> Result<Config, String> {
    parse_with(text, true)
}

/// Parse the configuration as it is on the base ref: the file may be older than the code that reads it, so a missing
/// threshold is not an error (the ratchet looks at the allow-lists only).
pub fn parse_base(text: &str) -> Result<Config, String> {
    parse_with(text, false)
}

fn parse_with(text: &str, need_threshold: bool) -> Result<Config, String> {
    let root: toml::Table = text.parse().map_err(|e| format!("monolith.toml: {e}"))?;
    let mut cfg = Config::default();
    let mut threshold = None;
    for (key, value) in &root {
        match key.as_str() {
            "monolith" => {
                let t = table(value, "[monolith]")?;
                threshold = t
                    .get("threshold")
                    .and_then(toml::Value::as_integer)
                    .and_then(|n| usize::try_from(n).ok());
            }
            "shared" => {
                for (name, v) in table(value, "[shared]")? {
                    let t = table(v, &format!("[shared.{name}]"))?;
                    let get = |k: &str| {
                        t.get(k)
                            .ok_or_else(|| format!("monolith.toml: [shared.{name}] has no `{k}`"))
                    };
                    let ban = get("ban")?
                        .as_array()
                        .ok_or_else(|| format!("monolith.toml: [shared.{name}] ban is a list"))?
                        .iter()
                        .map(|p| string(p, &format!("a pattern of [shared.{name}]")))
                        .collect::<Result<Vec<_>, _>>()?;
                    if ban.is_empty() {
                        return Err(format!("monolith.toml: [shared.{name}] bans nothing"));
                    }
                    cfg.shared.insert(
                        name.clone(),
                        Shared {
                            home: string(get("home")?, &format!("[shared.{name}] home"))?,
                            advice: string(get("use")?, &format!("[shared.{name}] use"))?,
                            ban,
                        },
                    );
                }
            }
            "allow_copy" => {
                let list = value
                    .as_array()
                    .ok_or("monolith.toml: allow_copy must be a list of [[allow_copy]] tables")?;
                for v in list {
                    let t = table(v, "[[allow_copy]]")?;
                    let mut fns: Vec<String> = t
                        .get("fns")
                        .and_then(toml::Value::as_array)
                        .ok_or("monolith.toml: [[allow_copy]] needs `fns`")?
                        .iter()
                        .map(|f| string(f, "a function of [[allow_copy]]"))
                        .collect::<Result<_, _>>()?;
                    if fns.len() < 2 || fns.iter().any(|f| !f.contains("::")) {
                        return Err(
                            "monolith.toml: [[allow_copy]] fns are two or more `path::fn`".into(),
                        );
                    }
                    fns.sort();
                    fns.dedup();
                    let reason = t
                        .get("reason")
                        .ok_or("monolith.toml: [[allow_copy]] needs a `reason`")?;
                    cfg.allow_copy.push(AllowCopy {
                        fns,
                        reason: string(reason, "[[allow_copy]] reason")?,
                    });
                }
            }
            "allow_shared" => {
                for (name, v) in table(value, "[allow_shared]")? {
                    cfg.allow_shared
                        .insert(name.clone(), reasons(v, &format!("[allow_shared.{name}]"))?);
                }
            }
            "allow_test_crate" => cfg.allow_test_crate = reasons(value, "[allow_test_crate]")?,
            other => return Err(format!("monolith.toml: unknown table [{other}]")),
        }
    }
    cfg.threshold = if need_threshold {
        threshold.ok_or("monolith.toml: [monolith] threshold must be a line count")?
    } else {
        threshold.unwrap_or(0)
    };
    if let Some(name) = cfg
        .allow_shared
        .keys()
        .find(|n| !cfg.shared.contains_key(*n))
    {
        return Err(format!(
            "monolith.toml: [allow_shared.{name}] has no [shared.{name}]"
        ));
    }
    Ok(cfg)
}

fn fn_name(id: &str) -> &str {
    id.rsplit("::").next().unwrap_or(id)
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A group of copies fits an entry of the base when each member can be matched to its own member
/// there: by `path::fn`, or, for a file that moved, by the function name. So a group may lose a
/// member (progress) or move to other paths, but may not gain one.
fn fits(now: &[String], base: &[String]) -> bool {
    if now.len() > base.len() {
        return false;
    }
    let mut free: Vec<&String> = base.iter().collect();
    let mut moved = Vec::new();
    for m in now {
        match free.iter().position(|b| *b == m) {
            Some(i) => {
                free.remove(i);
            }
            None => moved.push(m),
        }
    }
    moved.into_iter().all(|m| {
        free.iter()
            .position(|b| fn_name(b) == fn_name(m))
            .map(|i| free.remove(i))
            .is_some()
    })
}

fn raised(entry: String) -> Violation {
    Violation::entry(
        "allow-list",
        format!("new entry `{entry}`; entries only go down (fix the code instead)"),
    )
}

/// Compare with the lists on the base branch: nothing may be added. An entry may shrink, and a
/// file that moved keeps its entry:
/// - a group in `allow_copy` fits one entry of the base (see `fits`), and one base entry covers one
///   group;
/// - a file in `allow_shared` matches a base entry of the same rule by path, or else by file name,
///   and one base entry covers one file;
/// - a crate in `allow_test_crate` must be on the base by name.
pub fn check_not_raised(base: &Config, now: &Config) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut free: Vec<&AllowCopy> = base.allow_copy.iter().collect();
    let mut groups: Vec<&AllowCopy> = now.allow_copy.iter().collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.fns.len()));
    for g in groups {
        match free.iter().position(|b| fits(&g.fns, &b.fns)) {
            Some(i) => {
                free.remove(i);
            }
            None => out.push(raised(format!("allow_copy {}", g.fns.join(", ")))),
        }
    }
    for (rule, files) in &now.allow_shared {
        let old = base.allow_shared.get(rule);
        let mut free: Vec<&String> = old.into_iter().flat_map(|o| o.keys()).collect();
        let mut moved = Vec::new();
        for f in files.keys() {
            match free.iter().position(|b| *b == f) {
                Some(i) => {
                    free.remove(i);
                }
                None => moved.push(f),
            }
        }
        for f in moved {
            match free.iter().position(|b| file_name(b) == file_name(f)) {
                Some(i) => {
                    free.remove(i);
                }
                None => out.push(raised(format!("allow_shared.{rule} {f}"))),
            }
        }
    }
    out.extend(
        now.allow_test_crate
            .keys()
            .filter(|c| !base.allow_test_crate.contains_key(*c))
            .map(|c| raised(format!("allow_test_crate {c}"))),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[monolith]
threshold = 2500

[shared.hidden_text]
home = "crates/core/meldframe-text"
use = "meldframe_text::is_hidden_text"
ban = ["0x200B..=0x200F"]

[allow_shared.hidden_text]
"crates/x/src/a.rs" = "moves with #1"

[allow_test_crate]
"meldframe-a-tests" = "moves into meldframe-a"

[[allow_copy]]
fns = ["b.rs::f", "a.rs::g"]
reason = "audit D8"
"#;

    #[test]
    fn reads_every_table() {
        let c = parse(SAMPLE).unwrap();
        assert_eq!(c.threshold, 2500);
        assert_eq!(c.shared["hidden_text"].home, "crates/core/meldframe-text");
        assert_eq!(c.shared["hidden_text"].ban, ["0x200B..=0x200F"]);
        assert_eq!(c.allow_copy[0].fns, ["a.rs::g", "b.rs::f"]);
        assert_eq!(c.allow_shared["hidden_text"].len(), 1);
        assert_eq!(
            c.allow_test_crate["meldframe-a-tests"],
            "moves into meldframe-a"
        );
        assert_eq!(c.allow_copy.len(), 1);
    }

    #[test]
    fn an_entry_without_a_reason_is_refused() {
        let no_copy_reason = SAMPLE.replace("reason = \"audit D8\"", "");
        assert!(parse(&no_copy_reason).is_err());
        let blank = SAMPLE.replace("\"moves into meldframe-a\"", "\" \"");
        assert!(parse(&blank).is_err());
    }

    #[test]
    fn a_base_file_from_before_the_threshold_still_reads() {
        let old = "[allow_test_crate]\n\"a\" = \"r\"\n";
        assert!(
            parse(old).is_err(),
            "the current file must name its threshold"
        );
        let base = parse_base(old).unwrap();
        assert_eq!(base.allow_test_crate.len(), 1);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(parse("not toml [").is_err());
        assert!(parse("[monolith]\n").is_err());
        assert!(parse("[monolith]\nthreshold = 1\n[nope]\n").is_err());
        let orphan = "[monolith]\nthreshold = 1\n[allow_shared.x]\n\"a\" = \"r\"\n";
        assert!(parse(orphan).is_err());
        let one = SAMPLE.replace("fns = [\"b.rs::f\", \"a.rs::g\"]", "fns = [\"b.rs::f\"]");
        assert!(parse(&one).is_err());
    }

    #[test]
    fn a_new_entry_against_the_base_fails_and_a_removed_one_passes() {
        let now = parse(SAMPLE).unwrap();
        let mut base = now.clone();
        assert!(check_not_raised(&base, &now).is_empty());
        base.allow_test_crate.clear();
        let v = check_not_raised(&base, &now);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("allow_test_crate meldframe-a-tests"));
        let mut fewer = now.clone();
        fewer.allow_copy.clear();
        assert!(check_not_raised(&now, &fewer).is_empty());
    }

    fn copies(groups: &[&[&str]]) -> Config {
        Config {
            threshold: 1,
            allow_copy: groups
                .iter()
                .map(|g| {
                    let mut fns: Vec<String> = g.iter().map(|s| s.to_string()).collect();
                    fns.sort();
                    AllowCopy {
                        fns,
                        reason: "r".into(),
                    }
                })
                .collect(),
            ..Config::default()
        }
    }

    #[test]
    fn a_group_that_shrinks_is_progress() {
        let base = copies(&[&["a.rs::f", "b.rs::f", "c.rs::f"]]);
        let now = copies(&[&["a.rs::f", "c.rs::f"]]);
        assert!(check_not_raised(&base, &now).is_empty());
    }

    #[test]
    fn a_group_that_gains_a_member_or_a_new_group_fails() {
        let base = copies(&[&["a.rs::f", "b.rs::f"]]);
        let grew = copies(&[&["a.rs::f", "b.rs::f", "c.rs::f"]]);
        assert_eq!(check_not_raised(&base, &grew).len(), 1);
        let other = copies(&[&["a.rs::f", "b.rs::f"], &["x.rs::g", "y.rs::g"]]);
        assert_eq!(check_not_raised(&base, &other).len(), 1);
    }

    #[test]
    fn two_groups_do_not_both_hide_behind_one_base_entry() {
        let base = copies(&[&["a.rs::f", "b.rs::f", "c.rs::f"]]);
        let now = copies(&[&["a.rs::f", "b.rs::f"], &["a.rs::f", "c.rs::f"]]);
        assert_eq!(check_not_raised(&base, &now).len(), 1);
    }

    #[test]
    fn a_moved_file_keeps_its_functions_and_its_entry() {
        let base = copies(&[&["old/a.rs::f", "old/b.rs::f"]]);
        let moved = copies(&[&["new/mod_a/x.rs::f", "new/mod_b/y.rs::f"]]);
        assert!(check_not_raised(&base, &moved).is_empty());
        let renamed_fn = copies(&[&["new/x.rs::g", "new/y.rs::g"]]);
        assert_eq!(check_not_raised(&base, &renamed_fn).len(), 1);
    }

    #[test]
    fn a_moved_file_keeps_its_shared_entry_but_one_entry_covers_one_file() {
        let with = |files: &[&str]| {
            let mut c = Config::default();
            c.allow_shared
                .entry("hex".into())
                .or_default()
                .extend(files.iter().map(|f| (f.to_string(), "r".to_string())));
            c
        };
        let base = with(&["old/check/validate.rs"]);
        assert!(check_not_raised(&base, &with(&["new/app/validate.rs"])).is_empty());
        let two = with(&["new/a/validate.rs", "new/b/validate.rs"]);
        assert_eq!(check_not_raised(&base, &two).len(), 1);
        assert_eq!(check_not_raised(&base, &with(&["new/other.rs"])).len(), 1);
    }
}
