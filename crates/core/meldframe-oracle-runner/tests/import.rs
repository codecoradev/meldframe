//! The import side of the oracle runner (E4.4, slice 8): each exported frame is imported back in both forms and
//! compared with the original, and hand-written pages are imported and laid out with the same measurer.

use meldframe_oracle_runner::{Input, run};
use serde_json::{Value, json};

/// A row with three children, a text and a decorated rect: what the export writes, the importer reads.
fn row() -> Value {
    json!({
        "name": "row",
        "nodes": [
            {"id": "f", "name": "f", "type": "frame", "x": 5, "y": 7, "w": 300, "h": 120, "layout": "row", "padding": 10, "gap": 6, "align": "center", "fill": "#eeeeee", "clip": false},
            {"id": "a", "name": "a", "type": "rect", "parentId": "f", "w": 40, "h": 30, "fill": "#ff0000", "radius": 4},
            {"id": "t", "name": "t", "type": "text", "parentId": "f", "text": "Hello", "fontSize": 20, "sw": "hug", "sh": "hug", "fill": "#111111"},
            {"id": "b", "name": "b", "type": "rect", "parentId": "f", "w": 50, "h": 50, "fill": "#0000ff", "shadows": [{"x": 2, "y": 3, "blur": 0, "spread": 1, "color": "#00000080"}]}
        ]
    })
}

fn input(fixtures: Vec<Value>, pages: Vec<Value>) -> Input {
    serde_json::from_value(json!({"fixtures": fixtures, "pages": pages, "table": []})).unwrap()
}

#[test]
fn an_exported_frame_is_imported_back_in_both_forms_with_the_same_boxes_and_fields() {
    let out = run(&input(vec![row()], vec![]));
    let f = &out.fixtures[0].frames[0];
    for (form, e) in [("css", &f.css), ("tailwind", &f.tailwind)] {
        let rt = e.as_ref().unwrap().imported.as_ref().expect(form);
        assert_eq!(rt.error, None, "{form}");
        assert_eq!(rt.original, 4, "{form}");
        assert_eq!(rt.nodes.len(), 4, "{form}");
        assert!(rt.diffs.is_empty(), "{form}: {:?}", rt.diffs);
        assert!(
            rt.max_delta <= 0.0,
            "{form}: {} at {:?}",
            rt.max_delta,
            rt.worst
        );
        assert_eq!(rt.fixed_point, None, "{form}");
        assert!(
            rt.nodes.iter().all(|n| !n.pos.is_empty()),
            "{form}: pos is assigned"
        );
    }
}

#[test]
fn a_field_the_import_got_wrong_is_a_diff_and_a_box_that_moved_is_a_delta() {
    use meldframe_oracle_runner::compare;
    let i = input(vec![row()], vec![]);
    let original = &i.fixtures[0].nodes;
    let mut wrong = original.clone();
    wrong[1].radius = meldframe_protocol::Bind::Lit(9.0);
    wrong[2].font_size = 21.0;
    wrong[3].shadows[0].x = 5.0;
    let d = compare::fields(original, &wrong);
    assert!(d.iter().any(|l| l.contains("radius")), "{d:?}");
    assert!(d.iter().any(|l| l.contains("fontSize")), "{d:?}");
    assert!(d.iter().any(|l| l.contains("shadows")), "{d:?}");
    assert!(compare::fields(original, original).is_empty());
}

#[test]
fn siblings_keep_the_order_of_the_text_past_ten() {
    // ids are imp-1 .. imp-12: sorted as strings imp-10 comes before imp-2, so a pos made from the id would reorder them
    let cells: String = (0..12)
        .map(|i| format!(r#"<div data-name="c{i}" style="width:10px;height:10px"></div>"#))
        .collect();
    let page = json!({"name": "ten", "html": format!(r#"<div style="display:flex;width:400px">{cells}</div>"#), "css": "body{margin:0}"});
    let out = run(&input(vec![], vec![page]));
    let p = &out.pages[0];
    assert_eq!(p.error, None);
    let x = |name: &str| {
        let id = p.nodes.iter().find(|n| n.name == name).unwrap().id.clone();
        p.rects[&id].x
    };
    let xs: Vec<f64> = (0..12).map(|i| x(&format!("c{i}"))).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
    let pos: Vec<&str> = p
        .nodes
        .iter()
        .filter(|n| n.name.starts_with('c'))
        .map(|n| n.pos.as_str())
        .collect();
    assert!(
        pos.windows(2).all(|w| w[0] < w[1]),
        "pos strictly increases among siblings: {pos:?}"
    );
}

#[test]
fn a_page_is_imported_laid_out_and_its_findings_are_listed() {
    let page = json!({
        "name": "p",
        "html": r#"<div data-name="box" style="width:100px;height:50px;background:#ff0000;display:grid;grid-template-columns:1fr 1fr"></div>"#,
        "width": 800
    });
    let out = run(&input(vec![], vec![page]));
    let p = &out.pages[0];
    assert_eq!(p.error, None);
    let id = p
        .nodes
        .iter()
        .find(|n| n.name == "box")
        .expect("a node named box")
        .id
        .clone();
    let r = p.rects[&id];
    // the browser's body margin of 8 px is the page's padding
    assert_eq!((r.x, r.y, r.w, r.h), (8.0, 8.0, 100.0, 50.0));
    assert!(p.sources[&id].starts_with("div"), "{:?}", p.sources);
    assert!(
        p.unsupported
            .iter()
            .any(|u| u.starts_with("grid-template-columns:")),
        "{:?}",
        p.unsupported
    );
    assert!(p.nodes.iter().all(|n| !n.pos.is_empty()));
}

#[test]
fn text_with_no_element_is_an_error_of_the_page_and_not_of_the_run() {
    let page = json!({"name": "empty", "html": "just text"});
    let out = run(&input(
        vec![],
        vec![
            page,
            json!({"name": "ok", "html": "<div style=\"width:10px;height:10px\"></div>"}),
        ],
    ));
    assert!(
        out.pages[0]
            .error
            .as_deref()
            .unwrap()
            .contains("E_IMPORT_HTML_EMPTY")
    );
    assert_eq!(out.pages[1].error, None);
}

#[test]
fn the_text_a_page_needs_is_asked_of_the_table_once_and_answered_on_the_next_run() {
    let page = json!({"name": "t", "html": r#"<p data-name="t" style="font-size:20px;margin:0">Hello</p>"#});
    let out = run(&input(vec![], vec![page]));
    assert!(
        !out.pages[0].misses.is_empty(),
        "the stub answered, so the table lacks the text"
    );
    assert!(
        out.pages[0]
            .misses
            .iter()
            .any(|q| q.text == "Hello" && q.font_size == 20.0)
    );
}
