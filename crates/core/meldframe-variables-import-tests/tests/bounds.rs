//! The bounds of a report, and the order in which a variable is refused: the report is bounded in bytes as well as in
//! lines, what the text puts in it is cut and cleaned, and a variable that points at one refused later is told so.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;

/// Every string a report carries, with its entry name.
fn strings(i: &variables_import::Import) -> Vec<&str> {
    i.report
        .entries
        .iter()
        .flat_map(|e| [e.name.as_str(), e.message.as_str(), e.code])
        .collect()
}

fn bytes(i: &variables_import::Import) -> usize {
    strings(i).iter().map(|s| s.len()).sum()
}

#[test]
fn a_report_is_bounded_in_bytes_whatever_the_names_in_the_text() {
    // The first name is 100 KB long and every later one collides with it: no message may carry the name whole.
    let mut t = format!(":root {{ --a{}b: #fff;", "_".repeat(100_000));
    for k in 1..300 {
        t.push_str(&format!("--a{}b: #000;", "-".repeat(k)));
    }
    t.push('}');
    let i = run(&t);
    assert!(i.report.entries.len() <= variables_import::MAX_ENTRIES);
    assert!(i.report.omitted > 0);
    assert!(
        bytes(&i) <= variables_import::MAX_REPORT_BYTES + 2_000,
        "{} bytes",
        bytes(&i)
    );
}

#[test]
fn the_notes_come_first_and_count_against_the_cap() {
    let mut t = String::from(":root { --x: #111; --y: var(--x); } .dark { --x: #eee; }");
    for k in 0..400 {
        t.push_str(&format!(":root {{ --c{k}: calc(1px + 1px); }}"));
    }
    let i = run(&t);
    assert!(i.report.entries.len() <= variables_import::MAX_ENTRIES);
    assert_eq!(i.report.entries[0].kind, variables_import::EntryKind::Note);
    assert!(
        i.report
            .entries
            .iter()
            .any(|e| e.code == "dark_mode_source")
    );
    assert!(i.report.omitted >= 200, "{}", i.report.omitted);
}

#[test]
fn control_and_bidi_characters_are_not_echoed_to_the_reader_of_a_report_in_any_form() {
    let evil = "\u{202e}\u{200b}\u{1b}\u{7}x";
    let t = format!(
        ":root {{ --a: var(--b{evil}); --c: calc(1px) ; --d: 4px; }} .card{evil} {{ --e: 1 }} @import \"a{evil}\"; \
         :root {{ --nm{evil}: #fff; }}"
    );
    let i = run(&t);
    assert!(!i.report.entries.is_empty());
    for s in strings(&i) {
        assert!(
            !s.chars().any(|c| c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')),
            "{s:?}"
        );
    }
}

#[test]
fn a_property_that_points_at_one_killed_later_is_missing_its_target_not_in_a_cycle() {
    let i = run(":root { --a: var(--b); --b: var(--nope); --c: var(--a); --ok: #000; }");
    assert_eq!(i.report.variables, 1);
    for n in ["--a", "--b", "--c"] {
        assert_eq!(skipped(&i, n).unwrap().code, "alias_target_missing", "{n}");
    }
}

#[test]
fn no_format_character_variation_selector_or_tag_reaches_the_reader_of_a_report() {
    let mut chars: Vec<char> = Vec::new();
    for (a, b) in [
        (0xADu32, 0xAD),
        (0x600, 0x605),
        (0x61C, 0x61C),
        (0x6DD, 0x6DD),
        (0x70F, 0x70F),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x202A, 0x202E),
        (0x2060, 0x206F),
        (0xFE00, 0xFE0F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0x1BCA0, 0x1BCA3),
        (0x1D173, 0x1D17A),
        (0xE0000, 0xE007F),
        (0xE0100, 0xE01EF),
        (0x1, 0x1F),
    ] {
        chars.extend((a..=b).filter_map(char::from_u32));
    }
    for c in chars {
        let t = format!(
            ":root {{ --a: var(--b{c}x); --d{c}e: calc(1px); --f: {c}calc(1px); }} .card{c} {{ --g: 1 }}"
        );
        let i = run(&t);
        assert!(!i.report.entries.is_empty());
        for s in strings(&i) {
            assert!(!s.contains(c), "U+{:04X} in {s:?}", c as u32);
        }
    }
}

#[test]
fn the_rules_a_scan_counted_but_did_not_keep_are_in_the_omitted_count_of_the_report() {
    let i = run(&"a{}"
        .repeat(60_000)
        .chars()
        .take(200_000)
        .collect::<String>());
    assert!(i.report.entries.len() <= variables_import::MAX_ENTRIES);
    assert!(
        i.report.omitted >= 60_000 - variables_import::MAX_ENTRIES - 1_000,
        "{}",
        i.report.omitted
    );
}
