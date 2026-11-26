//! Inline elements and the text of a box: which style a text takes (the box's, or that of the one inline element
//! that holds all of it), what an inline child of a flex container is, and two small rules found on the way (a grid
//! has its row gap, the browser's own lengths are not listed as conversions).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod map_common;
use map_common::*;
use meldframe_protocol::{LayoutMode, NodeKind, SizeMode};

fn texts(ns: &[meldframe_protocol::Node]) -> Vec<&meldframe_protocol::Node> {
    ns.iter().filter(|n| n.kind == NodeKind::Text).collect()
}

#[test]
fn the_text_of_a_box_that_holds_one_inline_element_takes_its_style() {
    let i = run(
        "<ul style=\"margin:0;padding:0\">\n  <li id=item>\n    <a href=\"/x\" style=\"color:#336699;font-size:14px;font-weight:600\">Pricing</a>\n  </li>\n</ul>\n",
    );
    let ns = nodes(&i);
    let t = texts(&ns)[0];
    assert_eq!(
        (fill(t), t.font_size, t.font_weight),
        ("#336699", 14.0, 600)
    );
    assert_eq!(t.text, "Pricing");
    assert!(approx(&i, "item").is_empty(), "{:?}", info(&i, "item"));
}

#[test]
fn a_padded_box_with_one_styled_inline_element_gives_its_text_the_style_of_that_element() {
    let i = run(
        "<ul style=\"margin:0;padding:0\"><li id=item style=\"padding:4px;background:#eeeeee\"><a style=\"color:#336699;font-weight:700\">Pricing</a></li></ul>",
    );
    let ns = nodes(&i);
    let li = named(&ns, "item");
    assert_eq!(
        li.kind,
        NodeKind::Frame,
        "padded: a frame with a text in it"
    );
    let t = texts(&ns)[0];
    assert_eq!((fill(t), t.font_weight), ("#336699", 700));
    assert!(approx(&i, "item").is_empty(), "{:?}", info(&i, "item"));
}

#[test]
fn a_button_with_a_bold_span_has_a_bold_label_and_nothing_is_listed() {
    let i = run(
        "<div style=\"width:300px\"><button id=b style=\"padding:8px\"><span style=\"font-weight:700;color:#ff0000\">Go</span></button></div>",
    );
    let ns = nodes(&i);
    let t = texts(&ns)[0];
    assert_eq!(
        (t.text.as_str(), fill(t), t.font_weight),
        ("Go", "#ff0000", 700)
    );
    assert!(approx(&i, "b").is_empty(), "{:?}", info(&i, "b"));
}

#[test]
fn what_an_inline_element_of_the_chain_declares_is_listed_on_the_box() {
    let i = run(
        "<ul style=\"margin:0;padding:0\"><li id=item><a style=\"text-decoration:underline\"><b style=\"background:#ffff00\">x</b></a></li></ul>",
    );
    let l = lost(&i, "item");
    assert!(
        l.contains(&"text-decoration".to_string()) && l.contains(&"background-color".to_string()),
        "{l:?}"
    );
}

#[test]
fn an_inline_child_of_a_flex_container_is_an_item_with_its_own_style() {
    let ns = import_nodes(
        "<div style=\"display:flex;width:300px;gap:8px\"><a href=\"/\" style=\"font-size:18px;font-weight:700\">Acme</a><span style=\"font-size:12px\">beta</span></div>",
    );
    let ts = texts(&ns);
    assert_eq!(
        ts.iter()
            .map(|t| (t.text.as_str(), t.font_size, t.font_weight))
            .collect::<Vec<_>>(),
        [("Acme", 18.0, 700), ("beta", 12.0, 400)]
    );
    assert!(
        ts.iter().all(|t| t.sw == SizeMode::Hug),
        "an item of a row is as wide as its content"
    );
}

#[test]
fn an_inline_child_of_a_column_flex_container_stretches_as_a_browser_does() {
    // Both are flex items, blockified: the `a` as well as the inline-block stretch across a column.
    let ns = import_nodes(
        "<div style=\"display:flex;flex-direction:column;width:300px\"><a href=\"/\" style=\"font-size:18px\">Acme</a><span style=\"display:inline-block\">beta</span></div>",
    );
    let ts = texts(&ns);
    assert_eq!(ts.len(), 2);
    assert!(
        ts.iter().all(|t| t.sw == SizeMode::Fill),
        "{:?}",
        ts.iter().map(|t| (&t.text, t.sw)).collect::<Vec<_>>()
    );
}

#[test]
fn a_grid_is_a_column_with_the_row_gap_between_its_children() {
    let ns = import_nodes(
        "<div id=g style=\"display:grid;grid-template-columns:1fr 1fr;row-gap:12px;width:300px\"><div style=\"height:5px\"></div><div style=\"height:5px\"></div></div>",
    );
    let g = named(&ns, "g");
    assert_eq!((g.layout, g.gap), (LayoutMode::Column, 12.0));
}

#[test]
fn a_length_of_the_browsers_own_defaults_is_not_listed_as_a_conversion() {
    let i = run("<div style=\"width:300px\"><p id=p>x</p></div>");
    assert!(approx(&i, "p").is_empty(), "{:?}", info(&i, "p"));
    assert_eq!(nodes(&i)[1].mar_t, 16.0);
}

#[test]
fn a_hidden_element_in_a_box_that_holds_one_inline_element_is_skipped_once() {
    let i = run(
        "<ul style=\"margin:0;padding:0\"><li><a><span style=\"display:none\">h</span><div style=\"width:5px;height:5px\"></div></a></li></ul>",
    );
    assert_eq!(i.skipped.len(), 1, "{:?}", i.skipped);
}

#[test]
fn an_inline_block_child_of_a_grid_container_stretches_like_an_item_of_a_flex_one() {
    let ns = import_nodes(
        "<div style=\"display:grid;width:300px\"><span style=\"display:inline-block\">beta</span></div>",
    );
    let ts = texts(&ns);
    assert_eq!(ts.len(), 1);
    assert_eq!(ts[0].sw, SizeMode::Fill, "{:?}", ts[0]);
}

#[test]
fn a_discarded_owner_attempt_counts_what_the_readers_said_once() {
    // The `a` holds a box, so the owner attempt is thrown away and the box is read as the `li`'s own content: the
    // refusal of the inline element's inset shadow is one in the report, as it is when no attempt is made.
    let one = run(
        "<ul style=\"margin:0;padding:0\"><li><a style=\"box-shadow:inset 0 0 2px red\"><div style=\"width:5px;height:5px\"></div></a></li></ul>",
    );
    let other = run(
        "<ul style=\"margin:0;padding:0\"><li><b>x</b><a style=\"box-shadow:inset 0 0 2px red\"><div style=\"width:5px;height:5px\"></div></a></li></ul>",
    );
    assert_eq!(
        one.report.counts.get("shadow_inset"),
        Some(&1),
        "{:?}",
        one.report.counts
    );
    assert_eq!(other.report.counts.get("shadow_inset"), Some(&1));
}

#[test]
fn a_hidden_sibling_does_not_stop_the_one_inline_element_from_owning_the_text() {
    let i = run(
        "<ul style=\"margin:0;padding:0\"><li id=item><span style=\"display:none\">h</span><a style=\"color:#336699\">Pricing</a></li></ul>",
    );
    let ns = nodes(&i);
    assert_eq!(
        (fill(texts(&ns)[0]), texts(&ns)[0].text.as_str()),
        ("#336699", "Pricing")
    );
    assert_eq!(i.skipped.len(), 1, "{:?}", i.skipped);
}
