//! The work of the builder is linear in the text, however the text is built to make a recovery rule look far back: a
//! paragraph that is never closed under a thousand spans, an end tag that nothing opened, a list that is never ended.
//! The work is counted by the builder (frames of the open stack it looked at or popped), so the check does not
//! depend on a clock and runs under WASM. Two things are asserted for each text: the work is within a bound for
//! each byte of text (the window of 64 frames a rule looks at), and it grows no faster than the text does.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::parse;

fn work(html: &str) -> usize {
    parse(html, &roomy()).unwrap().report.work
}

fn linear(name: &str, make: impl Fn(usize) -> String) {
    let (a, b) = (make(10_000), make(20_000));
    let (wa, wb) = (work(&a), work(&b));
    assert!(
        wb <= (WINDOW + 4) * b.len(),
        "{name}: {wb} frames for {} bytes",
        b.len()
    );
    // Twice the text is at most twice the work, with a little for the start.
    assert!(wb * 100 <= wa * 205, "{name}: {wa} then {wb}");
}

#[test]
fn paragraphs_that_are_never_closed() {
    linear("p", |n| "<p>x".repeat(n));
}

#[test]
fn a_paragraph_under_a_table_under_a_run_of_spans_and_then_blocks_that_look_for_it() {
    linear("p-table-span-div", |n| {
        format!("<p><table>{}{}", "<span>".repeat(n), "<div>".repeat(n))
    });
}

#[test]
fn end_tags_that_nothing_opened_over_a_deep_stack() {
    linear("stray", |n| {
        format!("{}{}", "<b>".repeat(n), "</i>".repeat(n))
    });
}

#[test]
fn end_tags_that_close_the_wrong_element_each_time() {
    linear("zigzag", |n| {
        format!("{}{}", "<b><i>".repeat(n), "</b></i>".repeat(n))
    });
    linear("never-matching", |n| {
        format!("{}{}", "<a>".repeat(n), "</b>".repeat(n))
    });
}

#[test]
fn lists_rows_and_cells_that_are_never_ended() {
    linear("li", |n| format!("<ul>{}", "<li>x".repeat(n)));
    linear("nested li", |n| "<ul><li>".repeat(n));
    linear("td", |n| format!("<table><tr>{}", "<td>x".repeat(n)));
    linear("tr", |n| format!("<table>{}", "<tr><td>x".repeat(n)));
    linear("dt dd", |n| format!("<dl>{}", "<dt>x<dd>y".repeat(n)));
    linear("option", |n| format!("<select>{}", "<option>x".repeat(n)));
    linear("headings", |n| "<h1>x<h2>y".repeat(n));
}

#[test]
fn things_that_are_dropped_in_a_row() {
    linear("comments", |n| format!("<p>{}", "<!---->".repeat(n)));
    linear("bogus comments", |n| format!("<p>{}", "<!x>".repeat(n)));
    linear("scripts", |n| {
        format!("<p>{}", "<script>x</script>".repeat(n / 4))
    });
    linear("references", |n| {
        format!("<p>{}", "&amp;&#65;&bogus;&".repeat(n / 4))
    });
    linear("less-thans", |n| format!("<p>{}", "<".repeat(n)));
    linear("attributes", |n| format!("<p {}>", "a=1 ".repeat(n)));
}

#[test]
fn the_same_text_gives_the_same_work() {
    let html = "<p>a<b>c</i><ul><li>x<li>y</ul>".repeat(500);
    assert_eq!(work(&html), work(&html));
}
