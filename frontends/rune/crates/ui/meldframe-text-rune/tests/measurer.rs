//! The Rune measurer behind the layout core's `TextMeasurer` seam (E8.1.7, E8.2.3): a text is sized by Rune's own text
//! engine, and an answer is always a size (`meldframe_layout::is_size`), whatever the node holds.

use meldframe_layout::{MAX_MEASURE, TextMeasurer, is_size};
use meldframe_protocol::Node;
use meldframe_text_rune::RuneMeasurer;

/// A text node of the given text, font size and weight, as the layout reads it from a file.
fn text(text: &str, size: f64, weight: u16) -> Node {
    let json = serde_json::json!({
        "id": "t", "type": "text", "text": text, "fontWeight": weight,
    });
    let mut node: Node = serde_json::from_value(json).expect("a text node");
    // Set after the parse: JSON cannot hold the sizes that a bad host or file may still hand over.
    node.font_size = size;
    node
}

/// The bundled face only, so that the numbers do not depend on the machine's fonts.
fn measurer() -> RuneMeasurer {
    RuneMeasurer::bundled_fonts_only()
}

#[test]
fn a_known_string_has_a_positive_width_and_the_height_of_a_line() {
    let m = measurer();
    let (w, h) = m.measure(&text("Hello", 16.0, 400), None);
    assert!(w > 0.0 && h > 0.0, "{w} x {h}");
    assert!(is_size((w, h)));
}

#[test]
fn the_width_grows_with_the_length_of_the_text() {
    let m = measurer();
    let mut last = 0.0;
    for n in [1, 2, 4, 8, 16, 32] {
        let (w, _) = m.measure(&text(&"n".repeat(n), 16.0, 400), None);
        assert!(w > last, "{n} characters: {w} is not wider than {last}");
        last = w;
    }
}

#[test]
fn the_width_and_the_height_are_whole_pixels_rounded_up_and_the_exact_one_is_below() {
    let m = measurer();
    let node = text("Subheading text goes here", 14.0, 400);
    let (w, h) = m.measure(&node, None);
    assert_eq!((w.fract(), h.fract()), (0.0, 0.0));
    let (ew, eh) = m.exact(&node, None);
    assert!(ew <= w && w - ew < 1.0, "exact {ew}, rounded {w}");
    assert!(eh <= h && h - eh < 1.0, "exact {eh}, rounded {h}");
}

#[test]
fn a_wrap_width_gives_more_lines_and_no_line_wider_than_it() {
    let m = measurer();
    let node = text(
        "Design tools should produce layouts that a browser can reproduce exactly",
        16.0,
        400,
    );
    let (one_w, one_h) = m.measure(&node, None);
    let (wrapped_w, wrapped_h) = m.measure(&node, Some(160.0));
    assert!(
        wrapped_h >= 2.0 * one_h,
        "{wrapped_h} is not two lines of {one_h}"
    );
    assert!(wrapped_w <= 160.0 && wrapped_w < one_w, "{wrapped_w}");
    let (_, narrower_h) = m.measure(&node, Some(80.0));
    assert!(narrower_h > wrapped_h, "{narrower_h} against {wrapped_h}");
}

#[test]
fn a_word_wider_than_the_wrap_width_stays_whole_on_its_own_line() {
    let m = measurer();
    let word = text("Unbreakable", 16.0, 400);
    let (w, h) = m.measure(&word, None);
    assert_eq!(m.measure(&word, Some(20.0)), (w, h));
}

#[test]
fn an_empty_text_measures_as_one_space() {
    let m = measurer();
    assert_eq!(
        m.measure(&text("", 16.0, 400), None),
        m.measure(&text(" ", 16.0, 400), None)
    );
}

#[test]
fn the_weight_reaches_the_width() {
    let m = measurer();
    let regular = m
        .measure(&text("Weight reaches the width", 16.0, 400), None)
        .0;
    let semibold = m
        .measure(&text("Weight reaches the width", 16.0, 600), None)
        .0;
    assert!(semibold > regular, "{semibold} is not wider than {regular}");
}

#[test]
fn a_node_that_gives_no_size_gets_a_size_and_never_a_number_that_is_not_one() {
    let m = measurer();
    for size in [f64::NAN, f64::INFINITY, -12.0, 0.0, 1.0e12] {
        let answer = m.measure(&text("odd size", size, 400), None);
        assert!(is_size(answer), "font size {size}: {answer:?}");
    }
    for wrap in [f64::NAN, f64::NEG_INFINITY, -5.0, f64::INFINITY, 1.0e12] {
        let answer = m.measure(&text("odd wrap", 16.0, 400), Some(wrap));
        assert!(is_size(answer), "wrap {wrap}: {answer:?}");
    }
    let huge = m.measure(&text(&"w".repeat(2000), 4000.0, 400), None);
    assert!(is_size(huge) && huge.0 <= MAX_MEASURE, "{huge:?}");
}

/// The height of one line the export pins as `line-height`: the measured height of a single `M` (contract section 6, rule 2).
fn pinned_line(m: &RuneMeasurer, size: f64, weight: u16) -> f64 {
    m.measure(&text("M", size, weight), None).1
}

#[test]
fn a_text_of_n_lines_is_n_times_the_pinned_line_height_for_any_size() {
    let m = measurer();
    for size in [
        8.0, 12.0, 13.0, 13.5, 14.3, 16.0, 17.0, 29.0, 41.0, 72.0, 96.0,
    ] {
        let line = pinned_line(&m, size, 400);
        assert_eq!(line.fract(), 0.0, "the pin is whole pixels at {size}");
        let three = m.measure(&text("one\ntwo\nthree", size, 400), None).1;
        assert_eq!(three, 3.0 * line, "three explicit lines at {size}");
        let wrapped = text(
            "Design tools should produce layouts a browser can reproduce",
            size,
            400,
        );
        for wrap in [60.0, 120.0, 240.0] {
            let h = m.measure(&wrapped, Some(wrap)).1;
            assert_eq!(
                h % line,
                0.0,
                "{h} is not whole lines of {line} at {size}, wrap {wrap}"
            );
            assert!(h >= line);
        }
    }
}
