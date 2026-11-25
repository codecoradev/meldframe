//! The two profiles of `sanitize` and `clip`: what a node name keeps and what an identifier loses.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_text::{Profile, clip, sanitize, sanitize_clip};
use std::borrow::Cow;

const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
const FLAG: &str = "\u{1F1EB}\u{1F1F7}";
const HEART: &str = "\u{2764}\u{FE0F}";
const KEYCAP: &str = "1\u{FE0F}\u{20E3}";

fn display(s: &str) -> String {
    sanitize(s, Profile::Display).into_owned()
}

fn strict(s: &str) -> String {
    sanitize(s, Profile::Strict).into_owned()
}

#[test]
fn a_family_emoji_survives_the_display_profile_and_not_the_strict_one() {
    let name = format!("Team {FAMILY}");
    assert_eq!(display(&name), name);
    assert_eq!(strict(&name), "Team \u{1F468}\u{1F469}\u{1F467}\u{1F466}");
}

#[test]
fn a_flag_has_nothing_hidden_in_it_and_is_whole_in_both() {
    let name = format!("Made in {FLAG}");
    assert_eq!(display(&name), name);
    assert_eq!(strict(&name), name);
}

#[test]
fn a_heart_keeps_its_selector_in_display_and_a_keycap_too() {
    let name = format!("Love {HEART} step {KEYCAP}");
    assert_eq!(display(&name), name);
    assert_eq!(strict(&name), "Love \u{2764} step 1\u{20E3}");
}

#[test]
fn a_joiner_or_selector_that_joins_nothing_goes_in_display_too() {
    // After a letter, before a letter, doubled, at the ends, after a character that is no pictograph.
    assert_eq!(display("a\u{200D}b"), "ab");
    assert_eq!(display("a\u{FE0F}"), "a");
    assert_eq!(display("\u{200D}\u{1F600}"), "\u{1F600}");
    assert_eq!(display("\u{1F600}\u{200D}"), "\u{1F600}");
    assert_eq!(
        display("\u{1F600}\u{200D}\u{200D}\u{1F600}"),
        "\u{1F600}\u{200D}\u{1F600}"
    );
    // A selector 16 after a digit with no keycap behind it is not an emoji.
    assert_eq!(display("1\u{FE0F}"), "1");
    // Another variation selector is never kept.
    assert_eq!(display("\u{2764}\u{FE0E}"), "\u{2764}");
    assert_eq!(display("\u{1F600}\u{E0100}"), "\u{1F600}");
}

#[test]
fn bidirectional_controls_line_separators_and_tags_go_in_both_profiles() {
    let hidden = [
        '\u{202E}',
        '\u{202D}',
        '\u{202A}',
        '\u{202C}',
        '\u{2066}',
        '\u{2067}',
        '\u{2068}',
        '\u{2069}',
        '\u{200E}',
        '\u{200F}',
        '\u{061C}',
        '\u{2028}',
        '\u{2029}',
        '\u{E0041}',
        '\u{E007F}',
        '\u{200B}',
        '\u{200C}',
        '\u{FEFF}',
        '\u{00AD}',
        '\u{0000}',
        '\u{0085}',
        '\n',
        '\t',
    ];
    for c in hidden {
        let text = format!("a{c}b");
        assert_eq!(display(&text), "ab", "display U+{:04X}", c as u32);
        assert_eq!(strict(&text), "ab", "strict U+{:04X}", c as u32);
    }
    // Next to an emoji too: the tag characters of a hidden message do not ride on a pictograph.
    assert_eq!(display("\u{1F600}\u{E0068}\u{E0069}"), "\u{1F600}");
    assert_eq!(display("\u{1F600}\u{202E}\u{1F600}"), "\u{1F600}\u{1F600}");
}

#[test]
fn a_text_with_nothing_to_remove_is_not_copied() {
    for text in ["", "plain", "Frame \u{4E2D}\u{6587}", HEART, FAMILY] {
        assert!(
            matches!(sanitize(text, Profile::Display), Cow::Borrowed(_)),
            "{text}"
        );
    }
    assert!(matches!(
        sanitize("a\u{202E}", Profile::Display),
        Cow::Owned(_)
    ));
    assert!(matches!(sanitize(HEART, Profile::Strict), Cow::Owned(_)));
}

#[test]
fn clip_cuts_between_characters_in_both_counts() {
    assert_eq!(clip("hello", 3, usize::MAX), "hel");
    assert_eq!(clip("hello", usize::MAX, 3), "hel");
    assert_eq!(clip("hello", 10, 10), "hello");
    assert_eq!(clip("", 3, 3), "");
    // Four-byte characters at the cap: 3 characters are 12 bytes, and a byte cap of 10 keeps 2 of them.
    let text = "\u{1F600}".repeat(5);
    assert_eq!(clip(&text, 3, usize::MAX).chars().count(), 3);
    assert_eq!(clip(&text, usize::MAX, 10), "\u{1F600}\u{1F600}");
    assert_eq!(clip(&text, usize::MAX, 12).len(), 12);
    assert_eq!(clip(&text, usize::MAX, 3), "");
    assert_eq!(clip(&text, 0, usize::MAX), "");
    // A two-byte character is not split.
    assert_eq!(clip("\u{e9}\u{e9}", usize::MAX, 3), "\u{e9}");
}

#[test]
fn sanitize_clip_makes_safe_then_cuts_and_counts_both_ways() {
    // The hidden characters do not count toward the cap: they are gone before the cut.
    assert_eq!(
        sanitize_clip(
            "a\u{202E}b\u{200B}c\u{2028}d",
            Profile::Display,
            3,
            usize::MAX
        ),
        "abc"
    );
    // Four-byte characters at the cap: the character cap and the byte cap each hold.
    let text = "\u{1F600}".repeat(10);
    assert_eq!(
        sanitize_clip(&text, Profile::Display, 4, usize::MAX)
            .chars()
            .count(),
        4
    );
    assert_eq!(
        sanitize_clip(&text, Profile::Display, usize::MAX, 10),
        "\u{1F600}\u{1F600}"
    );
    assert_eq!(sanitize_clip("", Profile::Strict, 3, 3), "");
}

#[test]
fn a_cut_that_leaves_a_joiner_at_the_end_drops_it() {
    // The family cut after its first joiner would end with a joiner that joins nothing.
    let cut = sanitize_clip(FAMILY, Profile::Display, 2, usize::MAX);
    assert_eq!(cut, "\u{1F468}");
    // A cut after a whole member keeps what joined it.
    let cut = sanitize_clip(FAMILY, Profile::Display, 3, usize::MAX);
    assert_eq!(cut, "\u{1F468}\u{200D}\u{1F469}");
    // A keycap cut before its enclosing mark loses the selector that was waiting for it.
    assert_eq!(sanitize_clip(KEYCAP, Profile::Display, 2, usize::MAX), "1");
    // Nothing to cut: the same text.
    assert_eq!(sanitize_clip(FAMILY, Profile::Display, 20, 100), FAMILY);
}
