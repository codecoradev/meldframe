//! MM5: no cycles between the top-level modules of a monolith. An edge from module `a` to module
//! `b` is any `crate::b` path in a non-test file of `a`.

use std::collections::{BTreeMap, BTreeSet};

use super::shape::{modules, monoliths};
use super::tokens::{Tok, close_of};
use super::{Config, CrateSrc, Violation, is_test_path, product_tokens};

/// The first segment of every `crate::<segment>` path and `crate::{<segment>, ..}` list.
fn crate_paths(toks: &[Tok]) -> Vec<&str> {
    let is = |k: usize, s: &str| toks.get(k).is_some_and(|t| t.text == s);
    let mut out = Vec::new();
    for i in 0..toks.len() {
        if !(is(i, "crate") && is(i + 1, ":") && is(i + 2, ":")) {
            continue;
        }
        if is(i + 3, "{") {
            let close = close_of(toks, i + 3);
            let mut depth = 0;
            for (k, t) in toks.iter().enumerate().take(close).skip(i + 3) {
                match t.text.as_str() {
                    "{" => depth += 1,
                    "}" => depth -= 1,
                    _ => {}
                }
                if depth == 1 && k > i + 3 && (is(k - 1, "{") || is(k - 1, ",")) {
                    out.push(t.text.as_str());
                }
            }
        } else if let Some(t) = toks.get(i + 3) {
            out.push(t.text.as_str());
        }
    }
    out
}

/// The modules reachable from `from`.
fn reach<'a>(edges: &BTreeMap<&'a str, BTreeSet<&'a str>>, from: &'a str) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    let mut todo = vec![from];
    while let Some(m) = todo.pop() {
        for &n in edges.get(m).into_iter().flatten() {
            if seen.insert(n) {
                todo.push(n);
            }
        }
    }
    seen
}

pub fn check(crates: &[CrateSrc], cfg: &Config) -> Vec<Violation> {
    let mut out = Vec::new();
    for k in monoliths(crates, cfg) {
        let mods = modules(k);
        let names: BTreeSet<&str> = mods.iter().map(|m| m.name.as_str()).collect();
        let mut edges: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for m in &mods {
            for f in m.files.iter().filter(|f| !is_test_path(&f.path)) {
                let toks = product_tokens(f);
                for used in crate_paths(&toks) {
                    if let Some(&to) = names.get(used)
                        && to != m.name
                    {
                        edges.entry(m.name.as_str()).or_default().insert(to);
                    }
                }
            }
        }
        let mut cycles: BTreeSet<Vec<&str>> = BTreeSet::new();
        for &a in &names {
            let from_a = reach(&edges, a);
            let mut group: Vec<&str> = from_a
                .iter()
                .copied()
                .filter(|b| reach(&edges, b).contains(a))
                .collect();
            if !group.is_empty() {
                group.push(a);
                group.sort_unstable();
                group.dedup();
                cycles.insert(group);
            }
        }
        for group in cycles {
            out.push(Violation::new(
                &k.name,
                &format!("{}/src/lib.rs", k.dir),
                0,
                "MM5",
                format!(
                    "the modules {} use each other in a cycle; move what they share into a module \
                     of its own",
                    group.join(", ")
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monolith::fixtures::krate;

    fn cfg() -> Config {
        Config {
            threshold: 0,
            ..Config::default()
        }
    }

    const LIB: &str = "//! D.\nmod a;\nmod b;\nmod c;\n";

    fn crate_of(a: &str, b: &str, c: &str) -> CrateSrc {
        krate(
            "m",
            &[
                ("src/lib.rs", LIB),
                ("src/a.rs", a),
                ("src/b.rs", b),
                ("src/c.rs", c),
            ],
        )
    }

    #[test]
    fn two_modules_that_use_each_other_fail() {
        let k = crate_of("use crate::b::B;\n", "fn f() { crate::a::g(); }\n", "");
        let v = check(&[k], &cfg());
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].rule, "MM5");
        assert!(v[0].message.contains("a, b"), "{}", v[0]);
    }

    #[test]
    fn a_longer_cycle_fails_once() {
        let k = crate_of("use crate::b;\n", "use crate::c;\n", "use crate::a;\n");
        let v = check(&[k], &cfg());
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].message.contains("a, b, c"), "{}", v[0]);
    }

    #[test]
    fn a_use_list_counts() {
        let k = crate_of("use crate::{b::B, c};\n", "use crate::a;\n", "");
        assert_eq!(check(&[k], &cfg()).len(), 1);
    }

    #[test]
    fn a_chain_a_module_using_itself_and_test_code_pass() {
        let chain = crate_of("use crate::b;\n", "use crate::c;\n", "use crate::c::C;\n");
        assert!(check(&[chain], &cfg()).is_empty());
        let test_only = crate_of(
            "use crate::b;\n",
            "#[cfg(test)]\nmod t { use crate::a; }\n",
            "",
        );
        assert!(check(&[test_only], &cfg()).is_empty());
    }

    #[test]
    fn a_crate_below_the_threshold_is_not_read() {
        let k = crate_of("use crate::b;\n", "use crate::a;\n", "");
        let high = Config {
            threshold: 1000,
            ..Config::default()
        };
        assert!(check(&[k], &high).is_empty());
    }
}
