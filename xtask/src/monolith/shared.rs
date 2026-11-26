//! MM4: shared helpers. A pattern that a `[shared.<name>]` rule bans must be written in the
//! rule's home only; anywhere else the author is told to use the helper.

use std::collections::BTreeSet;

use super::tokens::{find, tokenize};
use super::{Config, CrateSrc, Violation, is_test_path, product_tokens};

/// True when `path` is the home itself or inside it.
fn inside(path: &str, home: &str) -> bool {
    path == home
        || path
            .strip_prefix(home)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub fn check(crates: &[CrateSrc], cfg: &Config) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut hit: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (name, rule) in &cfg.shared {
        let patterns: Vec<_> = rule.ban.iter().map(|p| (p, tokenize(p))).collect();
        let allowed = cfg.allow_shared.get(name);
        for k in crates {
            for f in k
                .files
                .iter()
                .filter(|f| !is_test_path(&f.path) && !inside(&f.path, &rule.home))
            {
                let toks = product_tokens(f);
                let found = patterns
                    .iter()
                    .find_map(|(p, pat)| find(&toks, pat).first().map(|line| (*p, *line)));
                let Some((pattern, line)) = found else {
                    continue;
                };
                if allowed.is_some_and(|a| a.contains_key(&f.path)) {
                    hit.insert((name.as_str(), f.path.as_str()));
                    continue;
                }
                out.push(Violation::new(
                    &k.name,
                    &f.path,
                    line,
                    "MM4",
                    format!(
                        "{name}: `{pattern}` is written here, outside {}; {}",
                        rule.home, rule.advice
                    ),
                ));
            }
        }
    }
    for (name, files) in &cfg.allow_shared {
        out.extend(files.keys().filter(|f| !hit.contains(&(name.as_str(), f.as_str()))).map(|f| {
            Violation::entry(
                "MM4",
                format!("allow_shared.{name} entry `{f}` matches no banned pattern; it is not needed, remove it"),
            )
        }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monolith::config::Shared;
    use crate::monolith::fixtures::krate;

    fn cfg(allow: &[(&str, &str)]) -> Config {
        let mut c = Config {
            threshold: 2500,
            ..Config::default()
        };
        c.shared.insert(
            "hex".into(),
            Shared {
                home: "crates/home".into(),
                advice: "use home::is_hex".into(),
                ban: vec!["matches!(h.len(), 3 | 4 | 6 | 8)".into()],
            },
        );
        for (rule, path) in allow {
            c.allow_shared
                .entry((*rule).into())
                .or_default()
                .insert((*path).into(), "reason".into());
        }
        c
    }

    const BAD: &str = "fn ok(h: &str) -> bool { matches!(h.len(), 3|4 | 6 | 8) }\n";

    #[test]
    fn a_banned_pattern_outside_its_home_fails_with_the_advice() {
        let v = check(
            &[krate("a", &[("src/lib.rs", &format!("\n{BAD}"))])],
            &cfg(&[]),
        );
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].rule, "MM4");
        assert_eq!(v[0].line, 2);
        assert!(v[0].message.contains("use home::is_hex"), "{}", v[0]);
    }

    #[test]
    fn the_same_pattern_in_the_home_passes() {
        let v = check(&[krate("home", &[("src/lib.rs", BAD)])], &cfg(&[]));
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn test_code_and_comments_and_strings_do_not_count() {
        let src = format!(
            "// {BAD}\nfn s() -> &'static str {{ \"matches!(h.len(), 3 | 4 | 6 | 8)\" }}\n\
             #[cfg(test)]\nmod t {{ {BAD} }}\n"
        );
        let a = krate("a", &[("src/lib.rs", &src), ("tests/t.rs", BAD)]);
        assert!(check(&[a], &cfg(&[])).is_empty());
    }

    /// The rules of `xtask/monolith.toml` as they are, without the allow-lists.
    fn real_rules() -> Config {
        let mut c = crate::monolith::parse(include_str!("../../monolith.toml")).unwrap();
        c.allow_shared.clear();
        c
    }

    #[test]
    fn every_rule_of_the_configuration_finds_a_violation_in_a_made_up_crate() {
        let cases = [
            // The hidden characters, in the spelling of either case, as a range, a single value or a
            // control-only check.
            (
                "hidden_text",
                "fn f(c: char) -> bool { matches!(c as u32, 0x200b..=0x200f) }",
            ),
            ("hidden_text", "fn f(c: char) -> bool { c == '\\u{feff}' }"),
            (
                "hidden_text",
                "fn f(c: u32) -> bool { matches!(c, 0x2028..=0x202e) }",
            ),
            (
                "hidden_text",
                "fn f(c: u32) -> bool { matches!(c, 0x2060..=0x206F | 0x41) }",
            ),
            ("hidden_text", "fn f(c: u32) -> bool { c == 0x00AD }"),
            (
                "hidden_text",
                "fn f(s: &str) -> bool { s.chars().any(|ch| ch.is_control() || ch == 'x') }",
            ),
            (
                "hidden_text",
                "fn f(s: &str) -> bool { s.chars().any(char::is_control) }",
            ),
            (
                "hex_colour",
                "fn f(hex: &str) -> bool { matches!(hex.len(), 3 | 4 | 6 | 8) }",
            ),
            (
                "asset_id",
                "fn f(s: &str) -> bool { s.bytes().all(|x| matches!(x, b'0'..=b'9' | b'a'..=b'f')) }",
            ),
            (
                "css_number",
                "fn f(s: &str) -> bool { s.bytes().all(|x| matches!(x, b'.' | b'-' | b'+')) }",
            ),
            (
                "css_number",
                "fn f(s: &str) -> bool { s.bytes().all(|x| matches!(x, b'.' | b'+' | b'-')) }",
            ),
        ];
        for (rule, code) in cases {
            let k = krate("made-up", &[("src/lib.rs", code)]);
            let v = check(&[k], &real_rules());
            let hit = v.iter().any(|v| v.message.starts_with(&format!("{rule}:")));
            assert!(hit, "{rule} did not find {code}: {v:?}");
        }
    }

    #[test]
    fn the_rules_of_the_configuration_leave_ordinary_code_and_their_homes_alone() {
        let clean = "fn f(c: char) -> bool { c.is_alphabetic() && c as u32 > 0x7F }";
        let k = krate("made-up", &[("src/lib.rs", clean)]);
        assert!(check(&[k], &real_rules()).is_empty());
        let home = krate(
            "core/meldframe-text",
            &[(
                "src/charset.rs",
                "fn f(c: char) -> bool { c.is_control() || c == '\\u{FEFF}' }",
            )],
        );
        assert!(check(&[home], &real_rules()).is_empty());
    }

    #[test]
    fn an_allowed_file_passes_and_a_stale_entry_fails() {
        let a = krate("a", &[("src/lib.rs", BAD)]);
        let allowed = cfg(&[("hex", "crates/a/src/lib.rs")]);
        assert!(check(&[a], &allowed).is_empty());
        let clean = krate("a", &[("src/lib.rs", "fn f() {}\n")]);
        let v = check(&[clean], &allowed);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("not needed"), "{}", v[0]);
    }
}
