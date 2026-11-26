//! Text in the HTML import (rule U9): what the report echoes is made safe by the one helper (`meldframe-text`, display
//! profile), and what the tree keeps of the document, its text, tags and attributes, is file content and is left as
//! the user wrote it.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::Child;

const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";

fn stray(name: &str) -> String {
    let p = run(&format!("<p>a</p></{name}>"));
    let e = p
        .report
        .entries
        .iter()
        .find(|e| e.code == "stray_end_tag")
        .unwrap_or_else(|| panic!("{:?}", p.report.entries));
    e.name.clone()
}

#[test]
fn a_name_in_the_report_loses_what_the_old_set_kept() {
    // The report used to keep each of these (the set it had was narrower than the one of the other layers).
    for c in [
        '\u{2028}',
        '\u{2029}',
        '\u{061C}',
        '\u{180B}',
        '\u{2800}',
        '\u{3164}',
        '\u{FFA0}',
        '\u{E0100}',
        '\u{2066}',
    ] {
        assert_eq!(stray(&format!("a{c}b")), "ab", "U+{:04X}", c as u32);
    }
}

#[test]
fn a_joiner_emoji_in_a_name_reaches_the_report_whole_and_a_bidi_control_does_not() {
    // A tag name starts with an ASCII letter; what follows can be anything.
    assert_eq!(stray(&format!("x{FAMILY}\u{202E}x")), format!("x{FAMILY}x"));
    assert_eq!(stray("a\u{200D}b\u{FE0F}c"), "abc");
}

#[test]
fn a_name_of_four_byte_characters_is_cut_at_the_cap_between_characters() {
    // The tree cuts a name at 64 bytes (`MAX_NAME_BYTES`), before the report's own cap of 80 characters: 16 pictures.
    let name = stray(&format!("x{}", "\u{1F600}".repeat(199)));
    assert_eq!(name.len(), 61, "{name:?}");
    assert!(name[1..].chars().all(|c| c == '\u{1F600}'), "{name:?}");
    // A family cut at that bound ends in a joiner that joins nothing (57 letters, a picture, then the joiner fills
    // the 64th byte): the report does not show it.
    let name = stray(&format!("{}{}", "a".repeat(57), FAMILY));
    assert_eq!(name.chars().count(), 58, "{name:?}");
    assert!(name.ends_with('\u{1F468}'), "{name:?}");
}

#[test]
fn the_text_tags_and_attributes_of_the_document_are_kept_as_written() {
    // File content round-trips: hidden characters in it are the user's, and the display layer cleans them when it
    // shows them (the app layer, part 3 of the work on rule U9).
    let text = format!("a\u{202E}b\u{200B}c\u{2028}d{FAMILY}\u{061C}");
    let p = run(&format!(
        "<p t\u{202E}x=\"v\u{200B}\">{text}&#x202E;&#x2028;</p>"
    ));
    let el = p.tree.element(first_element(&p.tree));
    assert_eq!(el.attrs[0].name, "t\u{202E}x");
    assert_eq!(el.attrs[0].value, "v\u{200B}");
    assert_eq!(
        el.children,
        vec![Child::Text(format!("{text}\u{202E}\u{2028}"))]
    );
}
