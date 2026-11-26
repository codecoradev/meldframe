//! `display_text`: the one helper of rule U9 for a screen that has no app state (the page that says the file did not
//! open shows what the server or the browser said). It is `meldframe_text::label` across the boundary: the same display
//! profile and the same cut as every name in `AppState`, so a front end cleans nothing itself.

use meldframe_wasm::display_text;

#[test]
fn a_hidden_character_is_removed_and_a_plain_text_is_not_changed() {
    assert_eq!(display_text("Not found", 200), "Not found");
    for c in [
        "\u{2800}", "\u{3164}", "\u{17B4}", "\u{200B}", "\u{00AD}", "\u{202E}",
    ] {
        assert_eq!(display_text(&format!("a{c}b"), 200), "ab", "{c:?}");
    }
    assert_eq!(display_text("a\u{00A0}\u{00A0}b", 200), "a b");
}

#[test]
fn a_line_break_is_a_space_and_the_ends_are_trimmed() {
    assert_eq!(
        display_text("  line one\nline two\r\n ", 200),
        "line one line two"
    );
}

#[test]
fn the_text_is_cut_at_the_given_number_of_characters_and_a_joiner_is_not_left_dangling() {
    assert_eq!(display_text(&"x".repeat(300), 200), "x".repeat(200));
    // A family emoji is kept whole, and a cut inside it leaves no joiner.
    let family = "\u{1F468}\u{200D}\u{1F469}";
    assert_eq!(display_text(family, 200), family);
    assert_eq!(display_text(family, 2), "\u{1F468}");
    assert_eq!(display_text("anything", 0), "");
}
