//! The round trip over MCP (`docs/import-html.md` section 3.7): `export_frame` of a frame, then `import_html` of that
//! HTML, in both forms, through a real client against the real server. The nodes come back with the fields they had,
//! except the losses the note lists: the ids and the `pos` strings (new), the stored `w` and `h` of a hug or fill axis,
//! `x` and `y` of a node in a flow, and a binding to a variable (it comes back as the value). The seeded random files
//! of `import.rs` and `roundtrip.rs` in `html-import` cover the fields one by one; this is the same through the tools.

mod import_html;
use import_html::*;
use serde_json::{Value, json};

fn add(node: Value) -> Value {
    json!({"type": "addNode", "node": node})
}

/// A card: a column frame with a heading, a paragraph, a swatch, a dot and a row of two labels.
fn card() -> Vec<Value> {
    vec![
        add(
            json!({"id": "card", "type": "frame", "name": "Pricing card", "x": 20, "y": 30, "w": 360, "h": 300,
            "layout": "column", "gap": 6, "padding": 12, "align": "start", "justify": "start", "clip": true,
            "fill": "#f0f0f0", "radius": 8, "border": {"width": 2, "color": "#336699", "position": "inside"},
            "shadows": [{"x": 0, "y": 2, "blur": 6, "spread": 0, "color": "#00000040"}]}),
        ),
        add(
            json!({"id": "title", "type": "text", "name": "Plan name", "parentId": "card", "text": "Pro plan",
            "fontSize": 24, "fontWeight": 700, "fill": "#112233", "sw": "hug", "sh": "hug", "w": 90, "h": 29}),
        ),
        add(
            json!({"id": "blurb", "type": "text", "name": "Blurb", "parentId": "card", "text": "For teams that ship",
            "fontSize": 14, "fontWeight": 400, "textAlign": "center", "fill": "#555555", "sw": "fill", "sh": "hug", "w": 100, "h": 17}),
        ),
        add(
            json!({"id": "swatch", "type": "rect", "name": "Swatch", "parentId": "card", "w": 100, "h": 40,
            "fill": "#ff0000", "radius": 4, "marT": 4}),
        ),
        add(
            json!({"id": "dot", "type": "ellipse", "name": "Dot", "parentId": "card", "w": 30, "h": 30, "fill": "#00ff00",
            "opacity": 0.5}),
        ),
        add(
            json!({"id": "row", "type": "frame", "name": "Labels", "parentId": "card", "w": 200, "h": 30, "layout": "row",
            "gap": 8, "padding": 0, "align": "center", "justify": "between", "clip": false, "fill": "none"}),
        ),
        add(
            json!({"id": "l1", "type": "text", "name": "Left label", "parentId": "row", "text": "Monthly", "fontSize": 12,
            "fontWeight": 600, "fill": "#000000", "sw": "hug", "sh": "hug", "w": 42, "h": 15}),
        ),
        add(
            json!({"id": "l2", "type": "text", "name": "Right label", "parentId": "row", "text": "$12", "fontSize": 12,
            "fontWeight": 600, "fill": "#000000", "sw": "hug", "sh": "hug", "w": 18, "h": 15}),
        ),
    ]
}

/// The nodes of a tree in order (a parent, then its children), as `get_nodes` gives them.
async fn flat(c: &Client, roots: &Value) -> Vec<Value> {
    fn ids(v: &Value, out: &mut Vec<String>) {
        out.push(v["id"].as_str().unwrap().to_owned());
        for k in v["children"].as_array().into_iter().flatten() {
            ids(k, out);
        }
    }
    let mut all = Vec::new();
    for r in roots.as_array().unwrap() {
        ids(r, &mut all);
    }
    let got = call(c, "get_nodes", json!({"file": "f", "ids": all})).await;
    got.body["nodes"].as_array().unwrap().clone()
}

fn subtree<'a>(all: &'a [Value], root: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    let mut todo = vec![root.to_owned()];
    while let Some(id) = todo.pop() {
        let n = all.iter().find(|n| n["id"] == id).unwrap();
        out.push(n);
        let kids: Vec<String> = all
            .iter()
            .filter(|k| k["parentId"] == id)
            .map(|k| k["id"].as_str().unwrap().to_owned())
            .collect();
        todo.extend(kids.into_iter().rev());
    }
    out
}

async fn round(format: &str) {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "seed", "commands": card()}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let e = call(
        &c,
        "export_frame",
        json!({"file": "f", "frame": "card", "format": format}),
    )
    .await;
    assert!(!e.is_error, "{}", e.body);
    let css = if format == "tailwind" {
        e.body["css"].clone()
    } else {
        json!("")
    };
    let imported = import(
        &c,
        "back",
        e.body["html"].as_str().unwrap(),
        json!({"css": css, "strict": true}),
    )
    .await;
    assert!(!imported.is_error, "{format}: {}", imported.body);
    assert_eq!(imported.body["commands"], 8, "{format}");
    let all = flat(&c, &tree(&c).await["roots"]).await;
    let root = imported.body["root"].as_str().unwrap();
    let (was, now) = (subtree(&all, "card"), subtree(&all, root));
    assert_eq!(was.len(), now.len(), "{format}");
    for (a, b) in was.iter().zip(&now) {
        let id = a["id"].as_str().unwrap();
        assert_ne!(a["id"], b["id"], "{format} {id}: the ids are new");
        let kind = a["type"].as_str().unwrap();
        let mut fields = vec!["type", "name", "opacity", "sw", "sh", "marT"];
        match kind {
            "text" => fields.extend(["text", "fontSize", "fontWeight", "textAlign", "fill"]),
            "frame" => fields.extend(["fill", "radius", "layout", "clip", "border", "shadows"]),
            "rect" => fields.extend(["fill", "radius", "border", "shadows"]),
            _ => fields.extend(["fill", "border", "shadows"]),
        }
        if kind == "frame" && a["layout"] != "none" {
            fields.extend(["gap", "padding", "align", "justify"]);
        }
        for field in fields {
            assert_eq!(a[field], b[field], "{format} {id}: {field}");
        }
        // The stored size of a hug or fill axis is not written by the export and not read back.
        if a["sw"] == "fixed" {
            assert_eq!(a["w"], b["w"], "{format} {id}: w");
        }
        if a["sh"] == "fixed" {
            assert_eq!(a["h"], b["h"], "{format} {id}: h");
        }
    }
    // The root keeps its place, the one the caller gave it: beside the card, 40 px past its right edge.
    let r = &now[0];
    assert_eq!(
        (r["x"].clone(), r["parentId"].clone()),
        (json!(420.0), Value::Null),
        "{format}"
    );
    // And an export of the import is the export of the card with the ids renamed (a fixed point).
    let again = call(
        &c,
        "export_frame",
        json!({"file": "f", "frame": root, "format": format}),
    )
    .await;
    let strip = |html: &str, ids: Vec<&str>| {
        let mut out = html.to_owned();
        for (n, id) in ids.iter().enumerate() {
            out = out.replace(&format!("\"{id}\""), &format!("\"N{n}\""));
        }
        out
    };
    let ids = |all: &[&Value]| -> Vec<String> {
        all.iter()
            .map(|n| n["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let (a, b) = (ids(&was), ids(&now));
    let first = strip(
        e.body["html"].as_str().unwrap(),
        a.iter().map(String::as_str).collect(),
    );
    let second = strip(
        again.body["html"].as_str().unwrap(),
        b.iter().map(String::as_str).collect(),
    );
    // The root of the export is at its own x and y: compare past the first rule of it.
    let rules = |s: &str| {
        s.lines()
            .filter(|l| !l.contains("left:") && !l.contains("data-node=\"N0\""))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(rules(&first), rules(&second), "{format}: a fixed point");
}

#[tokio::test]
async fn a_card_exported_as_css_and_imported_comes_back() {
    round("css").await;
}

#[tokio::test]
async fn a_card_exported_as_tailwind_and_imported_with_its_css_comes_back() {
    round("tailwind").await;
}

#[tokio::test]
async fn without_the_css_of_a_tailwind_export_the_browser_defaults_show_and_the_classes_are_listed()
{
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "seed", "commands": card()}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let e = call(
        &c,
        "export_frame",
        json!({"file": "f", "frame": "card", "format": "tailwind"}),
    )
    .await;
    // The html alone has the browser's defaults under it: a paragraph keeps its margin (the reset is in the css).
    let imported = import(&c, "back", e.body["html"].as_str().unwrap(), json!({})).await;
    assert!(!imported.is_error, "{}", imported.body);
    assert_eq!(
        imported.body["commands"], 9,
        "the 8 nodes of the card and the page frame that holds the margin of the body"
    );
    let blurb = imported.body["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "Blurb")
        .unwrap()
        .clone();
    let n = node(&c, blurb["id"].as_str().unwrap()).await;
    assert_eq!(n["marT"], json!(14.0), "1em of a 14 px paragraph");
}
