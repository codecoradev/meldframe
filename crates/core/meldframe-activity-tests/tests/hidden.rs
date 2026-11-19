//! The hidden-text rule (ADR-0021): `is_hidden_text` is a stricter set than `meldframe_mcp_defs::is_invisible`, named
//! in full. The set is written out here, apart from the code, and every scalar value is compared with it.

use meldframe_activity::{is_combining_mark, is_hidden_text};

/// The ranges of the ADR, besides the controls.
const HIDDEN: [(u32, u32); 21] = [
    (0x00AD, 0x00AD),
    (0x034F, 0x034F),
    (0x061C, 0x061C),
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
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0000, 0xE007F),
    (0xE0100, 0xE01EF),
    (0x0000, 0x001F),
    (0x007F, 0x009F),
];

fn expected(c: char) -> bool {
    HIDDEN.iter().any(|(a, b)| (*a..=*b).contains(&(c as u32)))
}

#[test]
fn every_scalar_value_agrees_with_the_set_of_the_adr() {
    for c in (0..=0x10FFFF).filter_map(char::from_u32) {
        assert_eq!(is_hidden_text(c), expected(c), "U+{:04X}", c as u32);
    }
}

#[test]
fn every_member_of_the_set_is_hidden_one_by_one() {
    let mut n = 0;
    for (a, b) in HIDDEN {
        for u in a..=b {
            if let Some(c) = char::from_u32(u) {
                assert!(is_hidden_text(c), "U+{u:04X}");
                n += 1;
            }
        }
    }
    assert!(n > 500, "the set has its members: {n}");
}

#[test]
fn what_is_invisible_covers_is_hidden_too() {
    // The old set is a subset: nothing a file name refuses is let through in a note.
    for c in (0..=0x10FFFF).filter_map(char::from_u32) {
        if meldframe_mcp_defs::is_invisible(c) {
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
    ] {
        assert!(!is_hidden_text(c), "U+{:04X}", c as u32);
    }
}

#[test]
fn the_line_break_is_a_control_for_this_rule() {
    // A note allows `\n` by its own rule; a node name in a summary gets no such exception.
    assert!(is_hidden_text('\n'));
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
