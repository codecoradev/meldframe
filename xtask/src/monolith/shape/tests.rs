use super::*;
use crate::monolith::fixtures::krate;

fn cfg() -> Config {
    Config {
        threshold: 0,
        ..Config::default()
    }
}

const LIB: &str = "//! A crate.\n\nmod a;\npub mod b;\npub use a::A;\n";
const DOC: &str = "//! Does one thing.\n";

fn rules(v: &[Violation]) -> Vec<&'static str> {
    v.iter().map(|v| v.rule).collect()
}

#[test]
fn a_root_with_only_docs_uses_mods_and_attributes_passes() {
    let lib = "//! Docs.\n#![forbid(unsafe_code)]\nuse std::fmt;\n#[cfg(feature = \"x\")]\npub mod a;\n\
               pub(crate) mod b;\npub use a::{A, B};\nextern crate alloc;\n";
    let k = krate(
        "m",
        &[("src/lib.rs", lib), ("src/a.rs", DOC), ("src/b.rs", DOC)],
    );
    assert!(check(&[k], &cfg()).is_empty());
}

#[test]
fn an_item_in_the_root_fails_whatever_it_is() {
    for item in [
        "fn f() {}",
        "pub struct S;",
        "const C: u8 = 1;",
        "impl X {}",
        "mod inline { }",
        "macro_rules! m { () => {} }",
    ] {
        let lib = format!("//! Docs.\nmod a;\n{item}\n");
        let k = krate("m", &[("src/lib.rs", &lib), ("src/a.rs", DOC)]);
        let v = check(&[k], &cfg());
        assert_eq!(rules(&v), ["MM1"], "{item}: {v:?}");
        assert_eq!(v[0].line, 3, "{item}");
    }
}

#[test]
fn test_code_in_the_root_is_not_an_item() {
    let lib = "//! Docs.\nmod a;\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n";
    let k = krate("m", &[("src/lib.rs", lib), ("src/a.rs", DOC)]);
    assert!(check(&[k], &cfg()).is_empty());
}

#[test]
fn a_root_of_more_than_150_lines_fails() {
    let long = format!("//! Docs.\nmod a;\n{}", "// padding\n".repeat(149));
    let k = krate("m", &[("src/lib.rs", &long), ("src/a.rs", DOC)]);
    assert_eq!(rules(&check(std::slice::from_ref(&k), &cfg())), ["MM1"]);
    let fits = format!("//! Docs.\nmod a;\n{}", "// padding\n".repeat(148));
    let k = krate("m", &[("src/lib.rs", &fits), ("src/a.rs", DOC)]);
    assert!(check(&[k], &cfg()).is_empty());
}

#[test]
fn a_crate_at_or_below_the_threshold_and_a_crate_without_lib_rs_are_not_read() {
    let k = krate("m", &[("src/lib.rs", "fn f() {}\n")]);
    let high = Config {
        threshold: 100,
        ..Config::default()
    };
    assert!(check(std::slice::from_ref(&k), &high).is_empty());
    let bin = krate("m", &[("src/main.rs", "fn main() {}\n")]);
    assert!(check(&[bin], &cfg()).is_empty());
}

#[test]
fn a_module_file_over_500_lines_fails() {
    let big = format!("{DOC}{}", "// line\n".repeat(500));
    let k = krate(
        "m",
        &[("src/lib.rs", LIB), ("src/a.rs", &big), ("src/b.rs", DOC)],
    );
    let v = check(&[k], &cfg());
    assert_eq!(rules(&v), ["MM2"], "{v:?}");
    assert!(v[0].path.ends_with("src/a.rs"));
}

#[test]
fn a_directory_module_needs_two_files_and_a_root_that_obeys_mm1() {
    let one = krate(
        "m",
        &[("src/lib.rs", "//! D.\nmod a;\n"), ("src/a/mod.rs", DOC)],
    );
    let v = check(&[one], &cfg());
    assert_eq!(rules(&v), ["MM2"], "{v:?}");
    assert!(v[0].message.contains("at least 2 files"), "{}", v[0]);

    let two = krate(
        "m",
        &[
            ("src/lib.rs", "//! D.\nmod a;\n"),
            ("src/a/mod.rs", "//! Does one thing.\nmod x;\n"),
            ("src/a/x.rs", "fn x() {}\n"),
        ],
    );
    assert!(check(&[two], &cfg()).is_empty());

    let fat_root = krate(
        "m",
        &[
            ("src/lib.rs", "//! D.\nmod a;\n"),
            ("src/a/mod.rs", "//! Does one thing.\nmod x;\nfn f() {}\n"),
            ("src/a/x.rs", "fn x() {}\n"),
        ],
    );
    let v = check(&[fat_root], &cfg());
    assert_eq!(rules(&v), ["MM2"], "{v:?}");
    assert!(v[0].message.contains("root"), "{}", v[0]);

    let named = krate(
        "m",
        &[
            ("src/lib.rs", "//! D.\nmod a;\n"),
            ("src/a.rs", "//! Does one thing.\nmod x;\n"),
            ("src/a/x.rs", "fn x() {}\n"),
        ],
    );
    assert!(check(&[named], &cfg()).is_empty());
}

#[test]
fn a_module_without_a_one_sentence_doc_fails() {
    for (text, ok) in [
        (
            "//! Does one thing.\n//!\n//! More, in another paragraph. It may have many.\n",
            true,
        ),
        ("//! Does one thing,\n//! over two lines.\n", true),
        ("\n//! Does one thing.\n", true),
        ("fn a() {}\n", false),
        ("// Does one thing.\n", false),
        ("//! Does one thing. Then another.\n", false),
        ("//! Does one thing\n", false),
        ("//!\n", false),
    ] {
        let k = krate(
            "m",
            &[("src/lib.rs", "//! D.\nmod a;\n"), ("src/a.rs", text)],
        );
        let v = check(&[k], &cfg());
        assert_eq!(v.is_empty(), ok, "{text:?}: {v:?}");
        if !ok {
            assert_eq!(rules(&v), ["MM2"]);
        }
    }
}

#[test]
fn more_than_40_modules_or_a_module_without_a_file_fails() {
    let mods: String = (0..41).map(|i| format!("mod m{i};\n")).collect();
    let lib = format!("//! D.\n{mods}");
    let files: Vec<(String, &str)> = (0..41).map(|i| (format!("src/m{i}.rs"), DOC)).collect();
    let mut all: Vec<(&str, &str)> = vec![("src/lib.rs", &lib)];
    all.extend(files.iter().map(|(p, t)| (p.as_str(), *t)));
    let v = check(&[krate("m", &all)], &cfg());
    assert_eq!(rules(&v), ["MM2"], "{v:?}");
    assert!(v[0].message.contains("41"), "{}", v[0]);

    let missing = krate("m", &[("src/lib.rs", "//! D.\nmod gone;\n")]);
    let v = check(&[missing], &cfg());
    assert_eq!(rules(&v), ["MM2"]);
    assert!(v[0].message.contains("gone"), "{}", v[0]);
}

#[test]
fn list_prints_each_module_with_its_sentence() {
    let k = krate(
        "m",
        &[
            ("src/lib.rs", "//! D.\nmod a;\nmod b;\n"),
            ("src/a.rs", "//! Reads the thing.\n"),
            ("src/b.rs", "fn b() {}\n"),
        ],
    );
    let lines = list(&[k], &cfg());
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].starts_with("m::a: Reads the thing."),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("m::b: (no one-sentence doc)"),
        "{}",
        lines[1]
    );
    let none = krate("n", &[("src/lib.rs", "fn f() {}\n")]);
    let high = Config {
        threshold: 100,
        ..Config::default()
    };
    assert!(list(&[none], &high).is_empty());
}
