//! Text (`docs/import-html.md` section 3.5): the style of a text node, how spaces collapse, and the rule of U9:
//! the text of a node is file content and comes back as written, while a name the importer makes up is cleaned.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod map_common;
use map_common::*;
use meldframe_protocol::{NodeKind, SizeMode, TextAlign};

fn text(style: &str, body: &str) -> meldframe_protocol::Node {
    let ns = import_nodes(&format!(
        "<div style=\"width:300px;{style}\"><p style=\"margin:0\">{body}</p></div>"
    ));
    ns.into_iter()
        .find(|n| n.kind == NodeKind::Text)
        .expect("a text node")
}

#[test]
fn spaces_and_newlines_collapse_unless_the_white_space_keeps_them() {
    assert_eq!(text("", "a   b\n\n c").text, "a b c");
    assert_eq!(text("white-space:pre", "a  b\n c").text, "a  b\n c");
    assert_eq!(text("white-space:pre-wrap", "a  b").text, "a  b");
    assert_eq!(text("white-space:pre-line", "a  b\n  c").text, "a b\nc");
    assert_eq!(text("white-space:nowrap", "a   b").text, "a b");
}

#[test]
fn nowrap_and_pre_make_the_width_hug_and_wrapping_text_fills() {
    assert_eq!(text("white-space:nowrap", "a b").sw, SizeMode::Hug);
    assert_eq!(text("white-space:pre", "a b").sw, SizeMode::Hug);
    assert_eq!(text("", "a b").sw, SizeMode::Fill);
    assert_eq!(text("", "a b").sh, SizeMode::Hug);
}

#[test]
fn a_text_with_its_own_width_is_fixed_and_wraps() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><p style=\"margin:0;width:120px\">one two three four</p></div>",
    );
    let t = &ns[1];
    assert_eq!((t.sw, t.w), (SizeMode::Fixed, 120.0));
    assert!(
        t.h > 0.0,
        "the stored height is an estimate from the stub measure"
    );
}

#[test]
fn text_transform_is_applied_to_the_characters() {
    assert_eq!(text("text-transform:uppercase", "ab c").text, "AB C");
    assert_eq!(text("text-transform:lowercase", "AB").text, "ab");
    assert_eq!(text("text-transform:capitalize", "ab cd").text, "Ab Cd");
}

#[test]
fn font_weight_rounds_to_a_step_of_a_hundred_and_says_so() {
    assert_eq!(text("font-weight:700", "x").font_weight, 700);
    assert_eq!(text("font-weight:bold", "x").font_weight, 700);
    assert_eq!(text("font-weight:normal", "x").font_weight, 400);
    let i = run("<div style=\"width:300px;font-weight:450\"><p style=\"margin:0\">x</p></div>");
    assert_eq!(nodes(&i)[1].font_weight, 500);
    assert!(approx(&i, "x").contains(&"font-weight".to_string()));
}

#[test]
fn text_align_start_end_and_justify_are_the_nearest_of_three() {
    let a = |v: &str| text(&format!("text-align:{v}"), "x").text_align;
    assert_eq!(
        (a("left"), a("center"), a("right")),
        (TextAlign::Left, TextAlign::Center, TextAlign::Right)
    );
    assert_eq!(
        (a("start"), a("end"), a("justify")),
        (TextAlign::Left, TextAlign::Right, TextAlign::Left)
    );
}

#[test]
fn the_colour_is_inherited_and_a_text_takes_it_as_its_fill() {
    let t = text("color:#abcdef80", "x");
    assert_eq!(fill(&t), "#abcdef80");
}

#[test]
fn the_font_family_and_line_height_are_refused_and_inter_is_not() {
    let i = run(
        "<div style=\"width:300px\"><p id=p style=\"margin:0;font-family:Georgia;line-height:1.4\">x</p></div>",
    );
    let l = lost(&i, "p");
    assert!(
        l.contains(&"font-family".to_string()) && l.contains(&"line-height".to_string()),
        "{l:?}"
    );
    let j = run(
        "<div style=\"width:300px;font-family:'Inter Variable'\"><p style=\"margin:0\">x</p></div>",
    );
    assert_eq!(j.unsupported_count(), 0, "{:?}", j.nodes);
}

#[test]
fn the_text_of_a_node_is_file_content_and_is_kept_as_written() {
    // U+202E (right-to-left override) and U+200B (zero-width space) are in the text of the file.
    let t = text("", "a\u{202e}b\u{200b}c");
    assert_eq!(t.text, "a\u{202e}b\u{200b}c");
}

#[test]
fn a_name_the_importer_makes_up_is_cleaned_of_hidden_characters() {
    let i = run(
        "<div style=\"width:300px\"><div data-name=\"Na\u{202e}me\u{200b}\" style=\"height:5px\"></div><p style=\"margin:0\">He\u{202e}llo\u{200b} there</p></div>",
    );
    let ns = nodes(&i);
    assert!(ns.iter().any(|n| n.name == "Name"), "{:?}", names(&ns));
    assert!(
        ns.iter().any(|n| n.name == "Hello there"),
        "{:?}",
        names(&ns)
    );
    for n in &i.nodes {
        assert!(
            meldframe_text::first_hidden(&n.name, false).is_none(),
            "{:?}",
            n.name
        );
        assert!(meldframe_text::first_hidden(&n.source, false).is_none());
    }
}

#[test]
fn an_emoji_name_keeps_its_joiner() {
    let family = "\u{1f468}\u{200d}\u{1f469}";
    let ns = import_nodes(&format!(
        "<div style=\"width:30px\"><div data-name=\"{family}\" style=\"width:10px;height:5px\"></div></div>"
    ));
    assert_eq!(ns[1].name, family);
}

#[test]
fn a_name_is_cut_to_the_limit_of_the_model() {
    let long = "n".repeat(400);
    let ns = import_nodes(&format!(
        "<div style=\"width:30px\"><div data-name=\"{long}\" style=\"width:10px;height:5px\"></div></div>"
    ));
    assert_eq!(ns[1].name.chars().count(), 256);
}

#[test]
fn character_references_in_text_and_names_are_read() {
    let ns = import_nodes(
        "<div data-name=\"A &amp; B\" style=\"width:300px\"><p style=\"margin:0\">x &lt; y &#38; z</p></div>",
    );
    assert_eq!(ns[0].name, "A & B");
    assert_eq!(ns[1].text, "x < y & z");
}
