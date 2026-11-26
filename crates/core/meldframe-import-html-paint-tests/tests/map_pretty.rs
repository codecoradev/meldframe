//! The rules of the mapping read on indented, pretty-printed HTML (the form a real page has: white space between
//! tags), and what a box holds that is not a box of its own: inline elements, the body, controls. Each test names the
//! rule it keeps.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod map_common;
use html_import::map::{ImportError, Options, import};
use map_common::*;
use meldframe_protocol::{Bind, LayoutMode, NodeKind};

#[test]
fn indentation_between_boxes_does_not_stop_a_wrapper_from_collapsing() {
    let ns = import_nodes(
        "<div>\n  <div>\n    <section style=\"width:100px\">\n      <p>x</p>\n    </section>\n  </div>\n</div>\n",
    );
    assert_eq!(
        names(&ns),
        ["Section", "x"],
        "the two bare wrappers make no node"
    );
}

#[test]
fn a_sized_box_that_holds_only_indentation_is_a_rect_and_a_round_one_an_ellipse() {
    let i = run(
        "<div style=\"width:300px\">\n  <div id=r style=\"width:20px;height:10px\">\n  </div>\n  <div id=c style=\"width:20px;height:20px;border-radius:50%;background:#000000\">\n  </div>\n  <div id=f style=\"width:20px;height:10px;background:#ff0000\">\n  </div>\n</div>\n",
    );
    let ns = nodes(&i);
    assert_eq!(named(&ns, "r").kind, NodeKind::Rect);
    assert_eq!(named(&ns, "c").kind, NodeKind::Ellipse);
    let f = named(&ns, "f");
    assert_eq!((f.kind, fill(f)), (NodeKind::Rect, "#ff0000"));
    assert!(i.skipped.is_empty(), "{:?}", i.skipped);
}

#[test]
fn an_indented_paragraph_is_one_text_and_the_indentation_is_not_in_it() {
    let ns = import_nodes(
        "<div style=\"width:300px\">\n  <p style=\"margin:0\">\n    Hello\n    world\n  </p>\n</div>\n",
    );
    assert_eq!(ns.len(), 2);
    assert_eq!(ns[1].text, "Hello world");
}

#[test]
fn the_opacity_of_a_text_is_kept() {
    let ns =
        import_nodes("<div style=\"width:300px\"><p style=\"margin:0;opacity:.4\">x</p></div>");
    assert_eq!(ns[1].opacity, Bind::Lit(0.4));
}

#[test]
fn an_inline_element_that_decorates_is_listed_on_the_box_that_holds_it() {
    let i = run(
        "<div style=\"width:300px\">\n  <p id=p style=\"margin:0\">\n    a <span style=\"background:#ffff00;padding:2px\">b</span>\n    <a style=\"text-decoration:underline\">c</a>\n    <em style=\"font-style:italic\">d</em>\n  </p>\n</div>\n",
    );
    let l = lost(&i, "p");
    for want in [
        "background-color",
        "padding-top",
        "text-decoration",
        "font-style",
    ] {
        assert!(l.contains(&want.to_string()), "{want} in {l:?}");
    }
    assert_eq!(nodes(&i)[1].text, "a b c d");
}

#[test]
fn the_body_is_a_flow_and_its_refused_declarations_are_listed() {
    let i = run_css(
        "<h1>a</h1><p>b</p>",
        "body{margin:0;display:flex;gap:7px;justify-content:center;transform:rotate(1deg)}",
    );
    let root = &nodes(&i)[0];
    assert_eq!((root.layout, root.gap), (LayoutMode::Row, 7.0));
    assert_eq!(format!("{:?}", root.justify), "Center");
    assert!(lost(&i, "Imported HTML").contains(&"transform".to_string()));
}

#[test]
fn a_refused_declaration_of_the_body_is_listed_on_a_single_root_too() {
    let i = run_css(
        "<div style=\"width:100px\"><p>x</p></div>",
        "body{margin:0;letter-spacing:2px}",
    );
    assert_eq!(
        nodes(&i).len(),
        2,
        "the body says nothing about the box, so the div is the root"
    );
    assert!(lost(&i, &i.nodes[0].name).contains(&"letter-spacing".to_string()));
}

#[test]
fn the_margin_of_the_body_is_one_rule_for_one_root_and_for_several() {
    let one = import(
        "<div style=\"width:100px\"><p>x</p></div>",
        "",
        &Options::default(),
    )
    .unwrap();
    let two = import("<p>a</p><p>b</p>", "", &Options::default()).unwrap();
    for i in [&one, &two] {
        let root = &nodes(i)[0];
        assert_eq!(
            (root.pad().left, root.name.as_str()),
            (8.0, "Imported HTML"),
            "the browser's 8 px of the body"
        );
    }
    assert_eq!(nodes(&one).len(), 3, "the page, the div and its text");
    let flat = nodes(&run("<div style=\"width:100px\"><p>x</p></div>"));
    assert_eq!(flat.len(), 2, "with no margin the div is the root");
}

#[test]
fn align_self_other_than_stretch_is_refused() {
    let i = run(
        "<div style=\"display:flex;width:200px;height:50px\"><div id=b style=\"align-self:center;width:10px;height:10px\"></div><div id=c style=\"align-self:stretch;width:10px\"></div></div>",
    );
    assert!(lost(&i, "b").contains(&"align-self".to_string()));
    assert!(!lost(&i, "c").contains(&"align-self".to_string()));
}

#[test]
fn a_hidden_input_is_not_drawn_and_a_password_value_is_never_copied() {
    let i = run(
        "<div style=\"width:300px\">\n  <input id=h type=\"HIDDEN\" value=\"secret\">\n  <input id=p type=\"password\" value=\"hunter2\" placeholder=\"Password\">\n</div>\n",
    );
    let ns = nodes(&i);
    assert!(
        ns.iter()
            .all(|n| !n.text.contains("secret") && !n.text.contains("hunter2")),
        "{ns:?}"
    );
    assert!(ns.iter().all(|n| n.name != "h"));
    assert!(ns.iter().any(|n| n.text == "Password"));
    assert!(!format!("{:?}", i.report).contains("hunter2"));
}

#[test]
fn a_select_reads_the_options_inside_an_optgroup() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><select><optgroup label=\"g\"><option>One</option><option selected>Two</option></optgroup></select></div>",
    );
    assert!(ns.iter().any(|n| n.text == "Two"), "{ns:?}");
}

#[test]
fn a_default_margin_or_padding_of_a_wrapper_keeps_it() {
    let ns = import_nodes("<div style=\"width:300px\"><ul><li>One</li></ul></div>");
    let ul = named(&ns, "List");
    assert_eq!(
        ul.pad().left,
        40.0,
        "the padding of the list is not dropped"
    );
    assert_eq!(ns.len(), 3);
    let q = import_nodes("<div style=\"width:300px\"><blockquote><p>x</p></blockquote></div>");
    let b = named(&q, "Quote");
    assert_eq!((b.mar_l, b.mar_r), (40.0, 40.0));
}

#[test]
fn an_image_lists_a_percent_width_and_its_padding_as_refused() {
    let i = run(
        "<div style=\"width:300px\"><img id=a src=\"x.png\" style=\"width:100%;height:20px;padding:4px\"><img id=b src=\"y.png\" width=\"50%\" height=\"20\"></div>",
    );
    let (a, b) = (lost(&i, "a"), lost(&i, "b"));
    assert!(
        a.contains(&"width".to_string()) && a.contains(&"padding-top".to_string()),
        "{a:?}"
    );
    assert!(b.contains(&"width".to_string()), "{b:?}");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_very_long_text_in_a_huge_box_is_measured_in_bounded_time() {
    // The stub measurer's wrap is quadratic in the length of a line: 250 KB of short words in a box of 99,999,999 px
    // is a line that long. The measure of a long text is one line, so the call takes milliseconds, not minutes.
    let t = "a ".repeat(125_000);
    let html =
        format!("<div style=\"width:300px\"><p style=\"margin:0;width:99999999px\">{t}</p></div>");
    let start = std::time::Instant::now();
    let i = import(&html, "body{margin:0}", &Options::default());
    assert!(start.elapsed().as_secs() < 5, "took {:?}", start.elapsed());
    // The text is over the limit of the file, which is the gate's to refuse, not the importer's.
    let i = i.unwrap();
    assert!(nodes(&i)[1].text.len() > 20_000);
}

#[test]
fn the_walk_stops_at_the_first_node_past_the_cap_and_says_so() {
    let boxes: String = (0..2_000)
        .map(|_| "<div style=\"width:5px;height:5px\"></div>")
        .collect();
    let e = import(
        &format!("<div style=\"width:300px\">{boxes}</div>"),
        "body{margin:0}",
        &Options::default(),
    )
    .unwrap_err();
    match e {
        // The walk stops after the first node past the cap, and the box that holds it counts too.
        ImportError::TooMany { nodes, max } => {
            assert!((101..=102).contains(&nodes) && max == 100, "{e}");
        }
        other => panic!("{other:?}"),
    }
    assert!(e.to_string().contains("at least 10"), "{e}");
}
