//! The import side of the runner (E4.4 slice 8, `docs/import-html.md` section 5, point 4): an export imported back and laid out
//! again, and hand-written HTML imported and laid out, both with the same table measurer as the original.

use meldframe_oracle_runner::{Input, run};
use serde_json::json;

/// A column frame with a text, a rect that fills, an ellipse and a nested row, which the export writes and the importer reads.
fn input() -> Input {
    serde_json::from_value(json!({
        "fixtures": [{
            "name": "one",
            "nodes": [
                {"id": "f", "type": "frame", "x": 5, "y": 7, "w": 300, "h": 200, "layout": "column", "padding": 10, "gap": 4, "fill": "#eeeeee"},
                {"id": "t", "type": "text", "parentId": "f", "text": "Hello", "fontSize": 20, "sw": "hug", "sh": "hug", "fontWeight": 700, "pos": "a0"},
                {"id": "r", "type": "rect", "parentId": "f", "sw": "fill", "h": 30, "fill": "#ff0000", "pos": "a1"},
                {"id": "row", "type": "frame", "parentId": "f", "sw": "fill", "sh": "hug", "layout": "row", "padding": 6, "gap": 5, "pos": "a2"},
                {"id": "e", "type": "ellipse", "parentId": "row", "w": 24, "h": 24, "fill": "#00ff00", "pos": "a0"},
                {"id": "r2", "type": "rect", "parentId": "row", "w": 40, "h": 12, "radius": 3, "pos": "a1"}
            ]
        }],
        "table": []
    }))
    .unwrap()
}

#[test]
fn an_export_imported_back_has_the_boxes_of_the_original_in_both_forms() {
    let out = run(&input());
    let one = &out.fixtures[0];
    let f = &one.frames[0];
    for (form, e) in [("css", &f.css), ("tailwind", &f.tailwind)] {
        let back = e
            .as_ref()
            .unwrap()
            .back
            .as_ref()
            .expect("an import of the export");
        assert_eq!(back.error, None, "{form}");
        let ids: Vec<&str> = back.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(
            ids,
            ["f", "t", "r", "row", "e", "r2"],
            "{form}: the order of the export"
        );
        for n in &back.nodes {
            let want = one.rects[&n.id];
            let got = n.rect;
            let worst = [
                got.x - want.x,
                got.y - want.y,
                got.w - want.w,
                got.h - want.h,
            ]
            .into_iter()
            .fold(0.0_f64, |m, d| m.max(d.abs()));
            assert!(
                worst <= 0.05,
                "{form}: {} is {worst} px off ({got:?} against {want:?})",
                n.id
            );
        }
        assert!(
            back.unsupported.is_empty(),
            "{form}: {:?}",
            back.unsupported
        );
    }
}

#[test]
fn the_imported_file_is_exported_again_and_its_rects_are_given_for_the_page() {
    let out = run(&input());
    let back = out.fixtures[0].frames[0]
        .css
        .as_ref()
        .unwrap()
        .back
        .as_ref()
        .unwrap();
    let again = back.again.as_ref().expect("an export of the imported file");
    assert!(again.html.contains("data-node=\"imp-0\""), "{}", again.html);
    assert_eq!(back.again_rects.len(), 6);
    assert_eq!(
        back.again_rects["imp-0"].x, 5.0,
        "the root stays where the original was"
    );
    // the imported file is not a copy of the export: its ids are fresh
    assert!(back.nodes.iter().all(|n| n.imported.starts_with("imp-")));
}

#[test]
fn a_loss_of_the_note_shows_in_the_kind_of_the_imported_node() {
    // a childless frame with no layout, no padding and a px size comes back as a rect (docs/import-html.md section 3.7): the boxes agree,
    // the kind does not, and the harness reports kinds so that the loss is stated and not hidden
    let i: Input = serde_json::from_value(json!({
        "fixtures": [{"name": "x", "nodes": [
            {"id": "f", "type": "frame", "w": 100, "h": 100, "layout": "column", "padding": 0, "gap": 0},
            {"id": "c", "type": "frame", "parentId": "f", "w": 50, "h": 50, "layout": "none", "padding": 0, "pos": "a0"}
        ]}],
        "table": []
    }))
    .unwrap();
    let out = run(&i);
    let back = out.fixtures[0].frames[0]
        .css
        .as_ref()
        .unwrap()
        .back
        .as_ref()
        .unwrap();
    assert_eq!(back.error, None);
    let kinds: Vec<&str> = back.nodes.iter().map(|n| n.kind.as_str()).collect();
    assert_eq!(kinds, ["frame", "rect"]);
}

fn pages(html: &str) -> Input {
    serde_json::from_value(json!({
        "fixtures": [],
        "imports": [{"name": "h", "html": html}],
        "table": []
    }))
    .unwrap()
}

#[test]
fn hand_written_html_is_imported_laid_out_and_each_node_names_its_source() {
    let html = "<!doctype html><html><head><style>body{margin:0}.box{width:100px;height:40px;background:#f00}</style></head>\
                <body><div class=\"box\"></div></body></html>";
    let out = run(&pages(html));
    let a = &out.imports[0];
    assert_eq!(a.error, None);
    // the page is a frame (the style of the page is a rule of its own) and the box is its one child
    assert_eq!(a.nodes.len(), 2, "{:?}", a.nodes);
    assert_eq!(a.nodes[0].source, "body");
    let n = &a.nodes[1];
    assert!(n.source.starts_with("div.box#"), "{}", n.source);
    assert_eq!(n.kind, "rect");
    let r = a.rects[&n.id];
    assert_eq!((r.x, r.y, r.w, r.h), (0.0, 0.0, 100.0, 40.0));
}

#[test]
fn what_an_import_could_not_take_is_listed_by_code_for_each_node() {
    let html = "<!doctype html><body style=\"margin:0\"><div style=\"width:80px;height:20px;background:#00f;transform:rotate(3deg)\"></div></body>";
    let a = &run(&pages(html)).imports[0];
    let codes: Vec<&str> = a
        .nodes
        .iter()
        .flat_map(|n| n.unsupported.iter().map(String::as_str))
        .collect();
    assert!(codes.iter().any(|c| c.starts_with("refused_")), "{codes:?}");
}

#[test]
fn html_that_makes_no_node_is_an_error_with_its_code() {
    let a = &run(&pages("<div style=\"display:none\">x</div>")).imports[0];
    assert_eq!(a.error.as_deref(), Some("E_IMPORT_HTML_EMPTY"));
    assert!(a.nodes.is_empty());
}

#[test]
fn text_the_table_lacks_is_a_miss_of_the_import_too() {
    let a = &run(&pages(
        "<!doctype html><body style=\"margin:0\"><p style=\"margin:0\">Quick brown fox</p></body>",
    ))
    .imports[0];
    assert!(
        a.misses.iter().any(|q| q.text == "Quick brown fox"),
        "{:?}",
        a.misses
    );
}
