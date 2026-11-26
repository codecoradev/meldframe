//! Text in an import (rule U9): the report echoes text of the style sheet, made safe by the one helper
//! (`meldframe-text`, display profile), and what becomes a name or an id of the file is built from ASCII only.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_text::{first_hidden, plain};

const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";

/// Every string a report carries.
fn strings(i: &variables_import::Import) -> Vec<&str> {
    i.report
        .entries
        .iter()
        .flat_map(|e| [e.name.as_str(), e.message.as_str(), e.code])
        .collect()
}

#[test]
fn a_zero_width_joiner_emoji_in_a_name_reaches_the_report_whole_and_a_bidi_control_does_not() {
    // The text of a style sheet is shown to a person, so it takes the display profile: a family stays a family.
    let t = format!(":root {{ --{FAMILY}\u{202E}x: calc(1px); }}");
    let i = run(&t);
    let e = i
        .report
        .entries
        .iter()
        .find(|e| e.code == "computed_value")
        .unwrap_or_else(|| panic!("{:?}", i.report.entries));
    assert_eq!(e.name, format!("--{FAMILY}x"));
    for s in strings(&i) {
        assert!(!s.contains('\u{202E}'), "{s:?}");
        assert_eq!(first_hidden(&s.replace('\u{200D}', ""), false), None);
    }
}

#[test]
fn a_joiner_that_joins_nothing_and_the_arabic_letter_mark_do_not_reach_the_report() {
    let t = ":root { --a\u{200D}b\u{61C}c: calc(1px); }";
    let i = run(t);
    let e = i
        .report
        .entries
        .iter()
        .find(|e| e.name.starts_with("--a"))
        .unwrap_or_else(|| panic!("{:?}", i.report.entries));
    assert_eq!(e.name, "--abc");
}

#[test]
fn the_line_and_paragraph_separators_do_not_reach_the_report() {
    // They are white space, so a name collapses them to a space; in no string of the report do they stay.
    let t = ":root { --a\u{2028}b: calc(1px); } .card\u{2029}x { --c: 1 } @foo\u{2028}bar;";
    let i = run(t);
    assert!(!i.report.entries.is_empty());
    for s in strings(&i) {
        assert!(!s.contains(['\u{2028}', '\u{2029}']), "{s:?}");
    }
}

#[test]
fn a_name_of_four_byte_characters_is_cut_at_the_cap_between_characters() {
    let t = format!(":root {{ --{}: calc(1px); }}", "\u{1F600}".repeat(300));
    let i = run(&t);
    let e = i
        .report
        .entries
        .iter()
        .find(|e| e.name.starts_with("--\u{1F600}"))
        .unwrap_or_else(|| panic!("{:?}", i.report.entries));
    // 80 characters: 77 of the text and the ellipsis; every one of the text is whole.
    assert_eq!(e.name.chars().count(), 80);
    assert!(e.name.ends_with("..."));
    assert!(
        e.name[2..e.name.len() - 3]
            .chars()
            .all(|c| c == '\u{1F600}')
    );
}

#[test]
fn a_family_cut_at_the_cap_does_not_end_in_a_joiner() {
    // 77 characters of the text are kept before the ellipsis: `--`, 73 letters, a picture and the joiner after it.
    let head = "a".repeat(73);
    let t = format!(":root {{ --{head}{FAMILY}{FAMILY}: calc(1px); }}");
    let i = run(&t);
    let e = i
        .report
        .entries
        .iter()
        .find(|e| e.name.starts_with("--aaa"))
        .unwrap_or_else(|| panic!("{:?}", i.report.entries));
    assert!(e.name.ends_with("..."), "{:?}", e.name);
    assert!(!e.name.ends_with("\u{200D}..."), "{:?}", e.name);
    assert!(e.name.ends_with("\u{1F468}..."), "{:?}", e.name);
}

#[test]
fn what_becomes_a_name_or_an_id_of_the_file_is_ascii_whatever_the_text_holds() {
    // Collections, modes, variables and their ids are built from ASCII letters and digits and the strict profile of
    // the text helper accepts every one of them.
    let t = format!(
        ":root {{ --{FAMILY}-brand: #fff; --a\u{202E}b: #000; --c\u{200B}d: #111; }} \
         @theme {{ --color-x\u{FE0F}y: #222; --\u{2764}\u{FE0F}: #333; }}"
    );
    let v = vars(&t);
    assert!(!v.variables.is_empty());
    let mut names: Vec<&str> = Vec::new();
    for c in &v.collections {
        names.push(&c.id);
        names.push(&c.name);
        for m in &c.modes {
            names.push(&m.id);
            names.push(&m.name);
        }
    }
    for x in &v.variables {
        names.push(&x.id);
        names.push(&x.name);
    }
    for n in names {
        assert!(n.is_ascii(), "{n:?}");
        assert_eq!(plain(n, 128, 128), Ok(()), "{n:?}");
    }
}
