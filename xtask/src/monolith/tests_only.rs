//! MM6: tests live in the crate they test. A crate named `*-tests`, `*-props` or `*-scenarios`
//! that has no source code of its own is a way to keep tests out of the line count of the crate
//! under test; it fails unless `[allow_test_crate]` lists it.

use std::collections::BTreeSet;

use super::{Config, CrateSrc, Violation, is_test_path, product_tokens};

/// A crate with at most this many lines of code under `src/` holds no source of its own.
pub const MAX_CODE_LINES: usize = 2;

const SUFFIXES: &[&str] = &["-tests", "-props", "-scenarios"];

/// Lines under `src/` that hold a token outside test code.
fn code_lines(k: &CrateSrc) -> usize {
    let src = format!("{}/src/", k.dir);
    k.files
        .iter()
        .filter(|f| f.path.starts_with(&src) && !is_test_path(&f.path))
        .flat_map(|f| {
            product_tokens(f)
                .into_iter()
                .map(|t| (f.path.clone(), t.line))
        })
        .collect::<BTreeSet<_>>()
        .len()
}

pub fn check(crates: &[CrateSrc], cfg: &Config) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut listed = BTreeSet::new();
    for k in crates {
        let named = SUFFIXES.iter().any(|s| k.name.ends_with(s));
        if !named || code_lines(k) > MAX_CODE_LINES {
            continue;
        }
        if cfg.allow_test_crate.contains_key(&k.name) {
            listed.insert(k.name.as_str());
            continue;
        }
        out.push(Violation::new(
            &k.name,
            &k.dir,
            0,
            "MM6",
            "a crate that only holds tests; move them to the tests/ directory of the crate they \
             test, or list it in [allow_test_crate] with a reason"
                .into(),
        ));
    }
    out.extend(
        cfg.allow_test_crate
            .keys()
            .filter(|n| !listed.contains(n.as_str()))
            .map(|n| {
                Violation::entry(
                    "MM6",
                    format!("allow_test_crate entry `{n}` is not a test-only crate; it is not needed, remove it"),
                )
            }),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monolith::fixtures::krate;

    fn cfg(allow: &[&str]) -> Config {
        Config {
            threshold: 2500,
            allow_test_crate: allow
                .iter()
                .map(|n| (n.to_string(), "reason".into()))
                .collect(),
            ..Config::default()
        }
    }

    const CODE: &str = "pub fn a() -> u8 {\n    1\n}\n";

    #[test]
    fn a_tests_crate_without_source_fails() {
        for name in ["x-tests", "x-props", "x-scenarios"] {
            let none = krate(name, &[("tests/t.rs", CODE)]);
            let v = check(&[none], &cfg(&[]));
            assert_eq!(v.len(), 1, "{name}: {v:?}");
            assert_eq!(v[0].rule, "MM6");
            assert_eq!(v[0].krate, name);
        }
    }

    #[test]
    fn docs_and_test_code_in_src_are_not_source() {
        let src = "//! Tests of x.\n\n#[cfg(test)]\nmod t {\n    fn a() {}\n    fn b() {}\n    fn c() {}\n}\n";
        let v = check(&[krate("x-tests", &[("src/lib.rs", src)])], &cfg(&[]));
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn two_lines_of_code_are_still_a_test_crate() {
        let v = check(
            &[krate(
                "x-tests",
                &[("src/lib.rs", "pub mod a;\npub mod b;\n")],
            )],
            &cfg(&[]),
        );
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn a_tests_crate_with_source_passes() {
        let k = krate("x-harness-tests", &[("src/lib.rs", CODE)]);
        assert!(check(&[k], &cfg(&[])).is_empty());
    }

    #[test]
    fn another_name_passes_even_without_source() {
        let k = krate("x-harness", &[("tests/t.rs", CODE)]);
        assert!(check(&[k], &cfg(&[])).is_empty());
    }

    #[test]
    fn a_listed_crate_passes_and_an_entry_that_matches_nothing_fails() {
        let k = krate("x-tests", &[("tests/t.rs", CODE)]);
        assert!(check(std::slice::from_ref(&k), &cfg(&["x-tests"])).is_empty());
        let v = check(&[k], &cfg(&["x-tests", "gone-tests"]));
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("gone-tests"), "{}", v[0]);
        let with_code = krate("y-tests", &[("src/lib.rs", CODE)]);
        let v = check(&[with_code], &cfg(&["y-tests"]));
        assert_eq!(v.len(), 1);
    }
}
