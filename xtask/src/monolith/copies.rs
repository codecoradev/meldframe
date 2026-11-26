//! MM3: no copies. Every function of 25 tokens or more outside test code is hashed by its tokens
//! (comments and the function's name left out); two files with the same hash fail.

use std::collections::{BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

use super::tokens::functions;
use super::{Config, CrateSrc, Violation, is_test_path, product_tokens};

/// Bodies shorter than this many tokens are not compared.
pub const MIN_TOKENS: usize = 25;

/// One function of a group of copies.
struct Member {
    krate: String,
    path: String,
    name: String,
    line: usize,
}

impl Member {
    fn id(&self) -> String {
        format!("{}::{}", self.path, self.name)
    }
}

/// Groups of functions with one body, spread over two files or more, as (token count, members).
fn groups(crates: &[CrateSrc]) -> Vec<(usize, Vec<Member>)> {
    let mut by_hash: HashMap<u64, (usize, Vec<Member>)> = HashMap::new();
    for k in crates {
        for f in k.files.iter().filter(|f| !is_test_path(&f.path)) {
            for func in functions(&product_tokens(f)) {
                if func.tokens.len() < MIN_TOKENS {
                    continue;
                }
                let mut h = DefaultHasher::new();
                func.tokens.hash(&mut h);
                let member = Member {
                    krate: k.name.clone(),
                    path: f.path.clone(),
                    name: func.name,
                    line: func.line,
                };
                by_hash
                    .entry(h.finish())
                    .or_insert_with(|| (func.tokens.len(), Vec::new()))
                    .1
                    .push(member);
            }
        }
    }
    let mut out: Vec<_> = by_hash
        .into_values()
        .filter(|(_, m)| m.iter().map(|x| &x.path).collect::<BTreeSet<_>>().len() > 1)
        .collect();
    for (_, members) in &mut out {
        members.sort_by(|a, b| (&a.path, &a.name).cmp(&(&b.path, &b.name)));
        members.dedup_by(|a, b| a.id() == b.id());
    }
    out
}

/// One entry of `[[allow_copy]]` covers one group, so two groups that have the same member ids
/// (the same function names in the same files, with different bodies) need two entries.
pub fn check(crates: &[CrateSrc], cfg: &Config) -> Vec<Violation> {
    let mut left: HashMap<&Vec<String>, usize> = HashMap::new();
    for a in &cfg.allow_copy {
        *left.entry(&a.fns).or_default() += 1;
    }
    let mut out = Vec::new();
    for (tokens, members) in groups(crates) {
        let key: Vec<String> = members.iter().map(Member::id).collect();
        match left.get_mut(&key) {
            Some(n) if *n > 0 => *n -= 1,
            _ => {
                let first = &members[0];
                out.push(Violation::new(
                    &first.krate,
                    &first.path,
                    first.line,
                    "MM3",
                    format!(
                        "the same function body ({tokens} tokens) is written in {}; keep one in a \
                         module both can use, or list the group in [[allow_copy]] with a reason",
                        key.join(", ")
                    ),
                ));
            }
        }
    }
    let mut unused: Vec<_> = left.into_iter().filter(|(_, n)| *n > 0).collect();
    unused.sort();
    for (fns, n) in unused {
        for _ in 0..n {
            out.push(Violation::entry(
                "MM3",
                format!(
                    "allow_copy entry [{}] matches no group of copies; it is not needed, remove it",
                    fns.join(", ")
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monolith::config::AllowCopy;
    use crate::monolith::fixtures::krate;

    /// A function of well over 25 tokens; `k` changes the body.
    fn long(name: &str, k: u32) -> String {
        format!(
            "pub fn {name}(x: u32) -> u32 {{ let a = x + {k}; let b = a * 2; let c = b - 1; \
             if c > 10 {{ c + a }} else {{ c * b }} }}\n"
        )
    }

    fn cfg(allow: &[&[&str]]) -> Config {
        Config {
            threshold: 2500,
            allow_copy: allow
                .iter()
                .map(|fns| AllowCopy {
                    fns: fns.iter().map(|s| s.to_string()).collect(),
                    reason: "test".into(),
                })
                .collect(),
            ..Config::default()
        }
    }

    #[test]
    fn the_same_body_in_two_crates_fails_whatever_the_name() {
        let a = krate("a", &[("src/lib.rs", &long("touches", 1))]);
        let b = krate("b", &[("src/x.rs", &long("other_name", 1))]);
        let v = check(&[a, b], &cfg(&[]));
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].rule, "MM3");
        assert!(
            v[0].message.contains("crates/b/src/x.rs::other_name"),
            "{}",
            v[0]
        );
        assert!(
            v[0].message.contains("crates/a/src/lib.rs::touches"),
            "{}",
            v[0]
        );
    }

    #[test]
    fn comments_and_layout_do_not_hide_a_copy() {
        let text = long("f", 1)
            .replace("let a", "// why\n    let a  ")
            .replace(';', ";\n");
        let a = krate("a", &[("src/lib.rs", &long("f", 1))]);
        let b = krate("b", &[("src/lib.rs", &text)]);
        assert_eq!(check(&[a, b], &cfg(&[])).len(), 1);
    }

    #[test]
    fn different_bodies_pass() {
        let a = krate("a", &[("src/lib.rs", &long("f", 1))]);
        let b = krate("b", &[("src/lib.rs", &long("f", 2))]);
        assert!(check(&[a, b], &cfg(&[])).is_empty());
    }

    #[test]
    fn two_copies_in_one_file_pass() {
        let text = format!("{}{}", long("f", 1), long("g", 1));
        assert!(check(&[krate("a", &[("src/lib.rs", &text)])], &cfg(&[])).is_empty());
    }

    #[test]
    fn a_short_body_is_not_compared() {
        let short = "pub fn f(x: u32) -> u32 { x + 1 }\n";
        let a = krate("a", &[("src/lib.rs", short)]);
        let b = krate("b", &[("src/lib.rs", short)]);
        assert!(check(&[a, b], &cfg(&[])).is_empty());
    }

    #[test]
    fn test_code_is_left_out() {
        let in_mod = format!("#[cfg(test)]\nmod tests {{ {} }}\n", long("f", 1));
        let a = krate(
            "a",
            &[("src/lib.rs", &in_mod), ("tests/t.rs", &long("f", 1))],
        );
        let b = krate("b", &[("src/lib.rs", &long("f", 1))]);
        assert!(check(&[a, b], &cfg(&[])).is_empty());
    }

    #[test]
    fn an_allowed_group_passes_and_a_group_that_grew_does_not() {
        let a = krate("a", &[("src/lib.rs", &long("f", 1))]);
        let b = krate("b", &[("src/lib.rs", &long("g", 1))]);
        let allow = [&["crates/a/src/lib.rs::f", "crates/b/src/lib.rs::g"][..]];
        assert!(check(&[a.clone(), b.clone()], &cfg(&allow)).is_empty());
        let c = krate("c", &[("src/lib.rs", &long("h", 1))]);
        let v = check(&[a, b, c], &cfg(&allow));
        assert!(
            v.iter()
                .any(|v| v.rule == "MM3" && v.krate != crate::monolith::CONFIG_FILE)
        );
        assert!(v.iter().any(|v| v.krate == crate::monolith::CONFIG_FILE));
    }

    /// Two files that each hold two functions called `f`, one pair with one body and one pair
    /// with another: two groups with the same member ids.
    fn two_groups_with_one_set_of_ids() -> Vec<CrateSrc> {
        let text = format!("{}{}", long("f", 1), long("f", 2));
        vec![
            krate("a", &[("src/lib.rs", &text)]),
            krate("b", &[("src/lib.rs", &text)]),
        ]
    }

    #[test]
    fn two_groups_with_the_same_ids_need_two_entries() {
        let pair = &["crates/a/src/lib.rs::f", "crates/b/src/lib.rs::f"][..];
        let crates = two_groups_with_one_set_of_ids();
        let v = check(&crates, &cfg(&[pair]));
        assert_eq!(v.len(), 1, "one entry covers one group: {v:?}");
        assert!(check(&crates, &cfg(&[pair, pair])).is_empty());
    }

    #[test]
    fn a_second_entry_with_the_same_ids_but_one_group_is_not_needed() {
        let pair = &["crates/a/src/lib.rs::f", "crates/b/src/lib.rs::f"][..];
        let crates = [
            krate("a", &[("src/lib.rs", &long("f", 1))]),
            krate("b", &[("src/lib.rs", &long("f", 1))]),
        ];
        let v = check(&crates, &cfg(&[pair, pair]));
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].message.contains("not needed"), "{}", v[0]);
    }

    #[test]
    fn an_entry_that_matches_no_copy_fails_as_not_needed() {
        let a = krate("a", &[("src/lib.rs", &long("f", 1))]);
        let allow = [&["crates/a/src/lib.rs::f", "crates/b/src/lib.rs::g"][..]];
        let v = check(&[a], &cfg(&allow));
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("not needed"), "{}", v[0]);
    }
}
