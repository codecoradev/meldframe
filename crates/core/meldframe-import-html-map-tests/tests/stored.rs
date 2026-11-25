//! The sizes the plan stores for a node that hugs or fills: the layout core works them out, with the text measurer the
//! caller gives (`docs/import-html.md` section 3.1, the `w` and `h` row).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::map::{Options, plan_with};
use meldframe_layout::{StubMeasurer, TextMeasurer};
use meldframe_protocol::{Node, SizeMode};

#[test]
fn a_hugging_text_is_as_large_as_its_line() {
    // The stub measurer: 0.5 of the size for each character, and a line of 1.2 times the size.
    let list = map(
        "<div data-name=Box><p data-name=T style='white-space:nowrap;font-size:20px;margin:0'>abcd</p><p>x</p></div>",
    );
    let t = named(&list, "T");
    assert_eq!((t.sw, t.sh), (SizeMode::Hug, SizeMode::Hug));
    assert_eq!((t.w, t.h), (40.0, 24.0));
}

#[test]
fn a_hugging_frame_is_as_large_as_what_is_in_it_and_its_padding() {
    let list = map(
        "<div data-name=Box style='display:flex;width:fit-content;gap:6px;padding:5px;font-size:20px'>\
         <p data-name=A style='margin:0'>ab</p><p data-name=B style='margin:0'>cd</p></div>",
    );
    let b = named(&list, "Box");
    assert_eq!((b.sw, b.sh), (SizeMode::Hug, SizeMode::Hug));
    assert_eq!((b.w, b.h), (20.0 + 6.0 + 20.0 + 10.0, 24.0 + 10.0));
}

#[test]
fn a_fill_takes_what_its_parent_gives_it_the_content_box() {
    let list = map(
        "<div data-name=Box style='display:flex;flex-direction:column;width:200px;padding:10px;font-size:20px'>\
         <div data-name=Fill><p style='margin:0'>a</p><p style='margin:0'>b</p></div><p style='margin:0'>c</p></div>",
    );
    let f = named(&list, "Fill");
    assert_eq!((f.sw, f.w), (SizeMode::Fill, 200.0));
}

#[test]
fn a_fixed_size_stays_what_the_page_said() {
    let list = map(
        "<div data-name=Box style='width:300px;height:100px'><p style='font-size:50px'>wide wide wide</p><p>x</p></div>",
    );
    let b = named(&list, "Box");
    assert_eq!((b.w, b.h), (300.0, 100.0));
}

#[test]
fn a_text_that_wraps_is_as_tall_as_its_lines() {
    // 30 px wide at 20 px: three characters of 10 px a line, so "aa bb cc" is three lines of 24 px.
    let list = map(
        "<div data-name=Box style='width:30px'><p data-name=T style='font-size:20px;margin:0'>aa bb cc</p><p>x</p></div>",
    );
    let t = named(&list, "T");
    assert_eq!((t.sw, t.sh, t.h), (SizeMode::Fill, SizeMode::Hug, 72.0));
}

/// Three pixels a character and ten a line, whatever the text.
struct Fixed;

impl TextMeasurer for Fixed {
    fn measure(&self, node: &Node, _wrap_w: Option<f64>) -> (f64, f64) {
        (node.text.chars().count() as f64 * 3.0, 10.0)
    }
}

#[test]
fn the_measurer_is_the_callers() {
    let html = "<div data-name=Box><p data-name=T style='white-space:nowrap;margin:0'>abcd</p><p>x</p></div>";
    let size = |m: &dyn TextMeasurer| {
        let p = plan_with(html, RESET, &Options::default(), m).unwrap();
        let list = nodes(&p);
        let t = named(&list, "T");
        (t.w, t.h)
    };
    assert_eq!(size(&Fixed), (12.0, 10.0));
    assert_eq!(size(&StubMeasurer), (32.0, 20.0));
}

#[test]
fn a_stored_size_never_passes_the_bound_of_a_size() {
    let long = "a".repeat(30_000);
    let list = map(&format!(
        "<div data-name=Box><p data-name=T style='white-space:nowrap;margin:0'>{long}</p><p>x</p></div>"
    ));
    assert_eq!(named(&list, "T").w, 100_000.0);
}
