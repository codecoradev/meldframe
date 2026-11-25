//! `label` and `identifier`: a name made safe to show, and an identifier made safe to keep as a key.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_text::{MAX_COMBINING_RUN, MAX_LABEL, identifier, label};

const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";

/// The characters of the golden table (issue 319): the braille blank, the Hangul filler, the Khmer vowel, the zero
/// width space, the soft hyphen and the bidirectional override. Each is removed from a name and from an identifier.
const HIDDEN: [char; 6] = [
    '\u{2800}', '\u{3164}', '\u{17B4}', '\u{200B}', '\u{00AD}', '\u{202E}',
];

#[test]
fn each_hidden_character_of_the_golden_table_goes_from_a_label_and_an_identifier() {
    for c in HIDDEN {
        let s = format!("a{c}b");
        assert_eq!(label(&s, MAX_LABEL), "ab", "label U+{:04X}", c as u32);
        assert_eq!(
            identifier(&s, 128, 128),
            "ab",
            "identifier U+{:04X}",
            c as u32
        );
    }
}

#[test]
fn a_run_of_white_space_is_one_space_in_a_label_and_the_ends_are_trimmed() {
    assert_eq!(label("a\u{00A0}\u{00A0}b", MAX_LABEL), "a b");
    assert_eq!(label("  a \t\u{3000} b\u{00A0} ", MAX_LABEL), "a b");
    assert_eq!(label("\u{00A0}\u{00A0}", MAX_LABEL), "");
}

#[test]
fn a_tab_or_a_line_break_is_a_gap_in_a_label_and_a_removed_character_in_an_identifier() {
    assert_eq!(
        label("two\nlines\tand\r\nmore", MAX_LABEL),
        "two lines and more"
    );
    assert_eq!(identifier("a\nb\tc", 128, 128), "abc");
    // Other controls are removed, not turned into a gap.
    assert_eq!(label("a\u{0}b\u{7F}c", MAX_LABEL), "abc");
}

#[test]
fn an_identifier_keeps_its_white_space_because_a_valid_one_must_come_back_as_it_was() {
    assert_eq!(
        identifier("a\u{00A0}\u{00A0}b", 128, 128),
        "a\u{00A0}\u{00A0}b"
    );
    assert_eq!(identifier(" agent:x ", 128, 128), " agent:x ");
}

#[test]
fn a_family_emoji_stays_whole_in_a_label_and_falls_apart_in_an_identifier() {
    assert_eq!(label(FAMILY, MAX_LABEL), FAMILY);
    assert_eq!(
        identifier(FAMILY, 128, 128),
        "\u{1F468}\u{1F469}\u{1F467}\u{1F466}"
    );
}

#[test]
fn a_text_that_is_only_hidden_is_empty() {
    assert_eq!(label("\u{200B}\u{202E}\u{2800}", MAX_LABEL), "");
    assert_eq!(identifier("\u{200B}\u{202E}", 128, 128), "");
}

#[test]
fn stacked_marks_are_cut_to_the_run_of_a_note_after_the_hidden_text_between_them_is_gone() {
    let marks = "\u{0301}".repeat(MAX_COMBINING_RUN + 3);
    let shown = label(&format!("e{marks}"), MAX_LABEL);
    assert_eq!(shown.chars().count(), 1 + MAX_COMBINING_RUN);
    let between = "e\u{0301}\u{200B}\u{0301}\u{200B}\u{0301}\u{0301}\u{0301}\u{0301}";
    assert_eq!(
        label(between, MAX_LABEL).chars().count(),
        1 + MAX_COMBINING_RUN
    );
}

#[test]
fn a_label_is_cut_in_characters_not_bytes_and_never_inside_a_character() {
    let long = "\u{1F600}".repeat(MAX_LABEL + 10);
    let shown = label(&long, MAX_LABEL);
    assert_eq!(shown.chars().count(), MAX_LABEL);
    assert_eq!(shown.len(), 4 * MAX_LABEL);
    assert_eq!(label(&long, 3), "\u{1F600}".repeat(3));
}

#[test]
fn a_cut_leaves_no_joiner_that_joins_nothing_and_no_space_at_the_end() {
    // Cut right after the first joiner of a family: the joiner has nothing to join.
    assert_eq!(label(FAMILY, 2), "\u{1F468}");
    assert_eq!(label("ab cd", 3), "ab");
}

#[test]
fn an_identifier_is_cut_to_both_bounds_between_characters() {
    let long = "\u{00E9}".repeat(128);
    let shown = identifier(&long, 128, 128);
    assert_eq!(shown, "\u{00E9}".repeat(64));
    assert_eq!(shown.len(), 128);
    assert_eq!(identifier("\u{1F600}\u{1F600}", 100, 5), "\u{1F600}");
}

#[test]
fn a_clean_text_is_returned_as_it_is() {
    assert_eq!(label("Card / title", MAX_LABEL), "Card / title");
    assert_eq!(identifier("agent:claude", 128, 128), "agent:claude");
}
