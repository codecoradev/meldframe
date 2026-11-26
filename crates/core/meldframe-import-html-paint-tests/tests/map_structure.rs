//! Structure (`docs/import-html.md` section 3.2): which elements make nodes, of what kind, in what order, with what
//! name and parent; what makes none.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod map_common;
use html_import::map::{Options, import};
use map_common::*;
use meldframe_protocol::{Align, BorderPosition, Justify, LayoutMode, NodeKind, SizeMode};

#[test]
fn one_element_is_the_root_frame_and_its_children_follow_in_source_order() {
    let ns = import_nodes(
        "<div id=card style=\"width:200px\"><p style=\"margin:0\">One</p><p style=\"margin:0\">Two</p></div>",
    );
    assert_eq!(names(&ns), ["card", "One", "Two"]);
    let root = &ns[0];
    assert_eq!(root.id, "imp-0");
    assert_eq!(root.parent_id, None);
    assert_eq!(
        (root.kind, root.layout),
        (NodeKind::Frame, LayoutMode::Column)
    );
    assert_eq!(
        (root.sw, root.w, root.sh),
        (SizeMode::Fixed, 200.0, SizeMode::Hug)
    );
    assert_eq!((root.padding, root.gap, root.clip), (0.0, 0.0, false));
    assert_eq!(fill(root), "none");
    let k = kids(&ns, "imp-0");
    assert_eq!(
        k.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(),
        ["One", "Two"]
    );
    assert!(
        k.iter()
            .all(|n| n.kind == NodeKind::Text && n.id.starts_with("imp-"))
    );
    assert!(
        ns.iter().all(|n| n.pos.is_empty()),
        "the server computes pos"
    );
}

#[test]
fn several_top_level_elements_share_a_root_frame_that_is_the_page() {
    let i = run("<h1 style=\"margin:0\">Title</h1><p style=\"margin:0\">Body</p>");
    let ns = nodes(&i);
    assert_eq!(ns.len(), 3);
    assert_eq!(ns[0].name, "Imported HTML");
    assert_eq!(ns[0].layout, LayoutMode::Column);
    assert_eq!(
        (ns[0].sw, ns[0].w, ns[0].sh),
        (SizeMode::Fixed, 1440.0, SizeMode::Hug)
    );
    assert_eq!(i.root, "imp-0");
}

#[test]
fn the_root_is_named_by_the_call_then_the_title_then_the_element_then_the_default() {
    let doc = "<html><head><title>My page</title></head><body><p>a</p><p>b</p></body></html>";
    let o = |name: Option<&str>| Options {
        name: name.map(Into::into),
        ..Options::default()
    };
    let name = |h: &str, o: &Options| nodes(&import(h, "", o).unwrap())[0].name.clone();
    assert_eq!(name(doc, &o(None)), "My page");
    assert_eq!(name(doc, &o(Some("Landing"))), "Landing");
    assert_eq!(
        name("<div data-name=\"Hero\"><p>a</p></div>", &o(None)),
        "Hero"
    );
    assert_eq!(name("<p>a</p><p>b</p>", &o(None)), "Imported HTML");
}

#[test]
fn the_page_margin_and_padding_go_on_the_root_frame() {
    let i = import("<p>a</p><p>b</p>", "body{padding:4px}", &Options::default()).unwrap();
    let root = &nodes(&i)[0];
    // The browser's 8 px margin of the body and the author's 4 px of padding.
    assert_eq!(root.pad().left, 12.0);
    let flat = nodes(&run("<p>a</p><p>b</p>"));
    assert_eq!(flat[0].pad().left, 0.0);
}

#[test]
fn the_body_background_paints_the_root() {
    let i = run_css("<p>a</p><p>b</p>", "body{margin:0;background:#112233}");
    assert_eq!(fill(&nodes(&i)[0]), "#112233");
}

#[test]
fn a_wrapper_that_says_nothing_makes_no_node() {
    let ns =
        import_nodes("<div><div><section style=\"width:100px\"><p>x</p></section></div></div>");
    // The page, two bare wrappers and the section: only the section and its text remain.
    assert_eq!(ns.len(), 2);
    assert_eq!(ns[0].name, "Section");
    assert_eq!(ns[0].w, 100.0);
    assert_eq!(ns[1].parent_id.as_deref(), Some("imp-0"));
}

#[test]
fn a_wrapper_with_a_declaration_of_its_own_stays() {
    let ns =
        import_nodes("<div style=\"padding:4px\"><div style=\"width:50px\"><p>x</p></div></div>");
    assert_eq!(ns.len(), 3);
}

#[test]
fn display_none_hidden_and_scripts_make_no_node_and_are_listed() {
    let i = run(
        "<div id=a style=\"width:10px\"><p style=\"display:none\">gone</p><p hidden>gone</p><script>alert(1)</script><style>p{color:red}</style><p>kept</p></div>",
    );
    let ns = nodes(&i);
    assert_eq!(names(&ns), ["a", "kept"]);
    let skipped: Vec<_> = i.skipped.iter().map(|s| s.code).collect();
    assert_eq!(skipped, ["display_none", "display_none"]);
    assert!(i.skipped[0].source.starts_with("p#"));
}

#[test]
fn a_paragraph_is_a_text_node_with_its_style_and_spaces_collapsed() {
    let ns = import_nodes(
        "<div id=a style=\"width:300px;color:#336699;font-size:20px;font-weight:600;text-align:center\"><p style=\"margin:0\">  Hello \n   world  </p></div>",
    );
    let t = &ns[1];
    assert_eq!(t.kind, NodeKind::Text);
    assert_eq!(t.text, "Hello world");
    assert_eq!(
        (t.font_size, t.font_weight, fill(t)),
        (20.0, 600, "#336699")
    );
    assert_eq!(format!("{:?}", t.text_align), "Center");
}

#[test]
fn a_br_is_a_line_break_in_the_text() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><p style=\"margin:0\">one<br>two <br/> three</p></div>",
    );
    assert_eq!(ns[1].text, "one\ntwo\nthree");
}

#[test]
fn an_element_with_a_background_and_text_is_a_frame_with_a_text_child() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><span style=\"display:block;background:#ff0000;padding:4px 8px\">Buy</span></div>",
    );
    assert_eq!(ns.len(), 3);
    let f = &ns[1];
    assert_eq!((f.kind, fill(f)), (NodeKind::Frame, "#ff0000"));
    assert_eq!((f.pad().top, f.pad().left), (4.0, 8.0));
    let t = &ns[2];
    assert_eq!(
        (t.kind, t.text.as_str(), t.parent_id.as_deref()),
        (NodeKind::Text, "Buy", Some(f.id.as_str()))
    );
}

#[test]
fn text_beside_a_block_is_its_own_text_node_in_order() {
    let ns = import_nodes(
        "<div style=\"width:300px\">before<div style=\"height:10px\"></div>after</div>",
    );
    let k = kids(&ns, "imp-0");
    assert_eq!(k.len(), 3);
    assert_eq!((k[0].kind, k[0].text.as_str()), (NodeKind::Text, "before"));
    assert_eq!(
        k[1].kind,
        NodeKind::Rect,
        "a hairline with a height and the width of the page"
    );
    assert_eq!((k[2].kind, k[2].text.as_str()), (NodeKind::Text, "after"));
}

#[test]
fn inline_children_of_another_style_join_the_text_and_the_first_difference_is_listed() {
    let i = run(
        "<div id=a style=\"width:300px\"><p id=p style=\"margin:0\">a <b>bold</b> c <span style=\"color:#ff0000\">red</span></p></div>",
    );
    let ns = nodes(&i);
    assert_eq!(ns[1].text, "a bold c red");
    let a = approx(&i, "p");
    assert_eq!(a, ["font-weight"], "{:?}", info(&i, "p"));
}

#[test]
fn an_image_is_a_rect_of_its_size_and_its_source_is_listed_not_fetched() {
    let i = run(
        "<div style=\"width:300px\"><img id=pic style=\"width:80px;height:40px\" src=\"https://x.test/a.png\"></div>",
    );
    let ns = nodes(&i);
    let r = named(&ns, "pic");
    assert_eq!(
        (r.kind, r.w, r.h, fill(r)),
        (NodeKind::Rect, 80.0, 40.0, "none")
    );
    assert_eq!(i.assets.len(), 1);
    assert_eq!(
        (i.assets[0].usage, i.assets[0].src.as_str()),
        ("image", "https://x.test/a.png")
    );
    assert_eq!(i.assets[0].node, r.id);
}

#[test]
fn an_svg_is_a_placeholder_that_says_its_content_was_not_carried() {
    let i = run(
        "<div style=\"width:300px\"><svg id=logo width=\"24\" height=\"24\"><path d=\"M0 0\"/></svg></div>",
    );
    let ns = nodes(&i);
    let r = named(&ns, "logo");
    assert_eq!((r.kind, r.w, r.h), (NodeKind::Rect, 24.0, 24.0));
    assert_eq!(lost(&i, "logo"), ["svg"]);
}

#[test]
fn a_button_is_a_centred_row_with_its_label() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><button id=b style=\"background:#000000;padding:8px\">Go</button></div>",
    );
    let b = named(&ns, "b");
    assert_eq!(
        (b.kind, b.layout, b.align, b.justify),
        (
            NodeKind::Frame,
            LayoutMode::Row,
            Align::Center,
            Justify::Center
        )
    );
    let k = kids(&ns, &b.id);
    assert_eq!((k.len(), k[0].text.as_str()), (1, "Go"));
}

#[test]
fn an_input_shows_its_value_else_its_placeholder_and_a_checkbox_is_skipped() {
    let i = run(
        "<div style=\"width:300px\"><input id=a value=\"Ann\" placeholder=\"Name\"><input id=b placeholder=\"Email\"><input id=c type=checkbox></div>",
    );
    let ns = nodes(&i);
    let text = |id: &str| kids(&ns, &named(&ns, id).id)[0].text.clone();
    assert_eq!((text("a"), text("b")), ("Ann".into(), "Email".into()));
    assert_eq!(ns.iter().filter(|n| n.name == "c").count(), 0);
    assert_eq!(
        i.skipped.iter().map(|s| s.code).collect::<Vec<_>>(),
        ["no_model_equivalent"]
    );
}

#[test]
fn a_table_is_a_column_of_rows_whose_cells_share_the_row() {
    let ns = import_nodes(
        "<table style=\"width:300px\"><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>",
    );
    let table = &ns[0];
    assert_eq!(table.layout, LayoutMode::Column);
    let rows = kids(&ns, &table.id);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].layout, LayoutMode::Row);
    let cells = kids(&ns, &rows[0].id);
    assert_eq!(cells.len(), 2);
    assert!(cells.iter().all(|c| c.sw == SizeMode::Fill));
}

#[test]
fn an_empty_box_with_a_px_size_is_a_rect_and_one_with_flex_is_a_frame() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><div id=r style=\"width:20px;height:10px;background:#00ff00\"></div><div id=f style=\"display:flex;width:20px;height:10px\"></div></div>",
    );
    assert_eq!(named(&ns, "r").kind, NodeKind::Rect);
    assert_eq!(named(&ns, "f").kind, NodeKind::Frame);
}

#[test]
fn an_empty_box_that_only_has_padding_hugs_as_a_frame() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><div id=e style=\"padding:6px;background:#00ff00\"></div></div>",
    );
    let e = named(&ns, "e");
    assert_eq!(
        (e.kind, e.layout, e.sh, e.sw),
        (
            NodeKind::Frame,
            LayoutMode::Column,
            SizeMode::Hug,
            SizeMode::Fill
        )
    );
}

#[test]
fn a_round_empty_box_is_an_ellipse_and_a_round_box_with_children_a_frame_with_a_radius() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><div id=e style=\"width:40px;height:40px;border-radius:50%;background:#000000\"></div><div id=f style=\"width:40px;height:30px;border-radius:50%;background:#000000\"><p>x</p></div></div>",
    );
    assert_eq!(named(&ns, "e").kind, NodeKind::Ellipse);
    let f = named(&ns, "f");
    assert_eq!(
        (f.kind, f.radius.lit().copied()),
        (NodeKind::Frame, Some(15.0))
    );
}

#[test]
fn the_paint_stage_fills_every_paint_field_and_a_border_takes_room() {
    let ns = import_nodes(
        "<div style=\"width:300px\"><div id=a style=\"width:100px;height:60px;background:#102030;border:2px solid #ff0000;box-shadow:0 1px 2px #00000040;opacity:.5;border-radius:4px;box-sizing:border-box\"><p>x</p></div></div>",
    );
    let a = named(&ns, "a");
    assert_eq!(fill(a), "#102030");
    assert_eq!(
        (a.border.width, a.border.position),
        (2.0, BorderPosition::Inside)
    );
    assert_eq!(
        (
            a.shadows.len(),
            a.opacity.lit().copied(),
            a.radius.lit().copied()
        ),
        (1, Some(0.5), Some(4.0))
    );
    assert_eq!(
        a.pad().left,
        2.0,
        "the border takes room, so it is added to the padding"
    );
    assert_eq!(a.w, 100.0, "border-box: the size is the one written");
}

#[test]
fn a_list_item_marker_is_not_carried_and_the_item_text_is() {
    let i = run("<ul style=\"margin:0;padding:0\"><li>One</li><li>Two</li></ul>");
    let ns = nodes(&i);
    assert_eq!(ns.iter().filter(|n| n.kind == NodeKind::Text).count(), 2);
    assert_eq!(i.report.counts.get("list_marker"), Some(&2));
}

#[test]
fn every_node_has_its_source_element_in_the_result() {
    let i = run("<section id=hero class=\"big top\" style=\"width:100px\"><p>x</p></section>");
    assert_eq!(i.nodes[0].source, "section.big#1");
    assert_eq!(i.nodes[0].id, "imp-0");
    assert_eq!(i.nodes[1].source, "p#2");
}

#[test]
fn node_ids_follow_the_prefix_and_the_order_and_the_root_takes_the_place_and_the_parent() {
    let o = Options {
        id_prefix: "imp-abc".into(),
        origin: (10.0, 20.0),
        parent: Some("frame1".into()),
        ..Options::default()
    };
    let i = import("<div style=\"width:50px\"><p>x</p></div>", FLAT, &o).unwrap();
    let ns = nodes(&i);
    assert_eq!(
        ns.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["imp-abc-0", "imp-abc-1"]
    );
    assert_eq!(
        (ns[0].x, ns[0].y, ns[0].parent_id.as_deref()),
        (10.0, 20.0, Some("frame1"))
    );
    assert_eq!(ns[1].parent_id.as_deref(), Some("imp-abc-0"));
}
