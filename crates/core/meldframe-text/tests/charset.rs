//! The set of hidden characters, written out here apart from the code, and every scalar value compared with it.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_text::{first_hidden, is_combining_mark, is_hidden_text, mark_run_exceeds};

/// The ranges, besides the controls: the set of the activity panel (ADR-0021) and the format characters that only
/// the variables import held (0600 to 0605, 06DD, 070F, 0890 and 0891, 08E2, 110BD, 110CD, 13430 to 1343F).
const HIDDEN: [(u32, u32); 30] = [
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180F),
    (0x200B, 0x200F),
    (0x2028, 0x202E),
    (0x2060, 0x206F),
    (0x2800, 0x2800),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFEFF, 0xFEFF),
    (0xFFA0, 0xFFA0),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0000, 0xE007F),
    (0xE0100, 0xE01EF),
    (0x0000, 0x001F),
    (0x007F, 0x009F),
    // The last entry repeats a member on purpose: the table is a list, not a partition.
    (0x200B, 0x200B),
];

fn expected(c: char) -> bool {
    HIDDEN.iter().any(|(a, b)| (*a..=*b).contains(&(c as u32)))
}

#[test]
fn every_scalar_value_agrees_with_the_set() {
    for c in (0..=0x10FFFF).filter_map(char::from_u32) {
        assert_eq!(is_hidden_text(c), expected(c), "U+{:04X}", c as u32);
    }
}

#[test]
fn the_set_holds_every_character_the_older_predicates_held() {
    // The older predicates, each as it was written: nothing any of them refused is let through now.
    let old_invisible = |c: char| {
        matches!(c as u32,
            0x200B..=0x200F | 0x202A..=0x202E | 0x2028 | 0x2029 | 0x2066..=0x2069 | 0xFEFF)
    };
    let old_hidden = |c: char| {
        c.is_control()
            || matches!(c as u32,
                0xAD | 0x34F | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0xFE00..=0xFE0F
                | 0xFEFF | 0xE0000..=0xE007F)
    };
    let old_import = |c: char| {
        c.is_control()
            || matches!(c as u32,
                0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x890..=0x891 | 0x8E2 | 0x180E | 0x200B..=0x200F
                | 0x202A..=0x202E | 0x2060..=0x206F | 0xFE00..=0xFE0F | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD
                | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A | 0xE0000..=0xE007F
                | 0xE0100..=0xE01EF)
    };
    for c in (0..=0x10FFFF).filter_map(char::from_u32) {
        if old_invisible(c) || old_hidden(c) || old_import(c) {
            assert!(is_hidden_text(c), "U+{:04X}", c as u32);
        }
    }
}

#[test]
fn what_is_not_hidden_stays_text() {
    for c in [
        ' ',
        'a',
        'Z',
        '0',
        'é',
        '\u{00A0}',
        '\u{00AC}',
        '\u{00AE}',
        '\u{2010}',
        '\u{2070}',
        '\u{FE10}',
        '\u{0430}',
        '中',
        '\u{1F600}',
        '\u{2801}',
        '\u{3165}',
        '\u{FFA1}',
        '\u{E0080}',
        '\u{E00FF}',
        '\u{0606}',
        '\u{13440}',
    ] {
        assert!(!is_hidden_text(c), "U+{:04X}", c as u32);
    }
}

#[test]
fn the_line_break_is_a_control_unless_the_rule_keeps_it() {
    assert!(is_hidden_text('\n'));
    assert_eq!(first_hidden("a\nb", false), Some('\n'));
    assert_eq!(first_hidden("a\nb", true), None);
    // Keeping the break keeps nothing else.
    assert_eq!(first_hidden("a\n\u{202E}b", true), Some('\u{202E}'));
    assert_eq!(first_hidden("a\u{2028}b", true), Some('\u{2028}'));
}

#[test]
fn combining_marks_are_the_five_blocks_and_nothing_else() {
    let blocks = [
        (0x0300, 0x036F),
        (0x1AB0, 0x1AFF),
        (0x1DC0, 0x1DFF),
        (0x20D0, 0x20FF),
        (0xFE20, 0xFE2F),
    ];
    for c in (0..=0x10FFFF).filter_map(char::from_u32) {
        let want = blocks.iter().any(|(a, b)| (*a..=*b).contains(&(c as u32)));
        assert_eq!(is_combining_mark(c), want, "U+{:04X}", c as u32);
    }
}

#[test]
fn a_run_of_marks_is_counted_in_a_row() {
    let marks = |n: usize| "a".to_owned() + &"\u{0301}".repeat(n);
    assert!(!mark_run_exceeds(&marks(4), 4));
    assert!(mark_run_exceeds(&marks(5), 4));
    // A character between two runs breaks the run.
    let split = marks(3) + "b" + &"\u{0301}".repeat(3);
    assert!(!mark_run_exceeds(&split, 4));
}
