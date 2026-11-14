//! The text measurer seam: the deterministic stub that tests and non-browser consumers use.
//! Expected numbers are worked out by hand from the stub's rule: character width is half the
//! font size, line height is the font size times 1.2 rounded up.

use meldframe_core::{Rect, StubMeasurer, TextMeasurer};
use meldframe_protocol::Node;

fn text_node(text: &str, font_size: f64) -> Node {
    serde_json::from_value(serde_json::json!({
        "id": "t",
        "type": "text",
        "text": text,
        "fontSize": font_size,
    }))
    .expect("a valid text node")
}

#[test]
fn single_line_width_is_chars_times_half_font_size_rounded_up() {
    // "hello" at 16 px: 5 chars * 8 = 40; line height ceil(19.2) = 20.
    let n = text_node("hello", 16.0);
    assert_eq!(StubMeasurer.measure(&n, None), (40.0, 20.0));
    // "abc" at 15 px: 3 * 7.5 = 22.5, rounded up to 23; line height 18.
    let n = text_node("abc", 15.0);
    assert_eq!(StubMeasurer.measure(&n, None), (23.0, 18.0));
}

#[test]
fn empty_text_measures_as_one_space() {
    // One space at 10 px on one line: width 5, height 12.
    let n = text_node("", 10.0);
    assert_eq!(StubMeasurer.measure(&n, None), (5.0, 12.0));
    // Wrapped, the space is a break, so the line is empty: width 0, still one line high.
    assert_eq!(StubMeasurer.measure(&n, Some(100.0)), (0.0, 12.0));
}

#[test]
fn characters_are_counted_not_bytes() {
    // Two chars of three bytes each at 10 px: width 10.
    let n = text_node("\u{4e2d}\u{6587}", 10.0);
    assert_eq!(StubMeasurer.measure(&n, None), (10.0, 12.0));
}

#[test]
fn wrapping_breaks_at_spaces_when_the_next_word_does_not_fit() {
    // At 10 px each char is 5 px. "aa bb" is 25 px and fits in 30; adding " cc" makes 40 and does not.
    let n = text_node("aa bb cc", 10.0);
    assert_eq!(StubMeasurer.measure(&n, Some(30.0)), (25.0, 24.0));
    // A wide enough box keeps one line.
    assert_eq!(StubMeasurer.measure(&n, Some(40.0)), (40.0, 12.0));
}

#[test]
fn a_word_wider_than_the_wrap_width_stays_whole_on_its_own_line() {
    // "abcdefgh" is 40 px and cannot break; "ij" follows on a second line.
    let n = text_node("abcdefgh ij", 10.0);
    assert_eq!(StubMeasurer.measure(&n, Some(20.0)), (40.0, 24.0));
}

#[test]
fn explicit_newlines_are_not_line_breaks_in_the_stub() {
    // The stub only breaks at spaces; the newline counts as one character.
    let n = text_node("ab\ncd", 10.0);
    assert_eq!(StubMeasurer.measure(&n, None), (25.0, 12.0));
}

#[test]
fn rect_round_trips_through_json() {
    let r = Rect {
        x: 1.5,
        y: -2.0,
        w: 30.0,
        h: 4.25,
    };
    let json = serde_json::to_string(&r).unwrap();
    assert_eq!(json, r#"{"x":1.5,"y":-2.0,"w":30.0,"h":4.25}"#);
    assert_eq!(serde_json::from_str::<Rect>(&json).unwrap(), r);
}
