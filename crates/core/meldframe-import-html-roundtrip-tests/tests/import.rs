//! The round trip of the importer (`docs/import-html.md` section 3.7): a file is exported (`export_frame`, in the css
//! form and in the Tailwind form with the stylesheet it returns) and imported (`html_import::map::import`); every field
//! of every node comes back except the losses the note lists, and an export of the imported file is the export of the
//! first one with the ids renamed (a fixed point). Seeded random files: 300 natively, 20 under WASM; `MELDFRAME_SEEDS`
//! sets the count and `MELDFRAME_SEED=n` replays one.
//!
//! The losses, each checked here and not hidden: the ids and the `pos` strings; the stored `w` and `h` of an axis
//! that is hug or fill (and of a hug that the export writes as fixed: rule G1); `x` and `y` of a node in a flow; a
//! binding to a variable (it comes back as the literal); a childless frame without layout and a shader (the shapes of
//! the note's list, which the generator leaves out and `known_losses` below shows for themselves).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use export::{ExportOptions, Format, export_frame};
use html_import::map::{Imported, Options, import};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::{LayoutMode, Node, NodeKind, SizeMode, Variables};
mod common;
use common::{Rng, merge};
use serde_json::{Value, json};
use std::collections::HashMap;

const COLOURS: [&str; 5] = ["#ff0000", "#3366ff", "#00000080", "#abcdef", "#112233"];
const WORDS: [&str; 5] = [
    "Hello",
    "a < b & \"c\"",
    "Lorem ipsum",
    "x",
    "Quick brown fox",
];

fn variables() -> Value {
    json!({
        "collections": [{"id": "c1", "name": "Theme", "modes": [{"id": "m1", "name": "Light"}]}],
        "variables": [
            {"id": "v1", "collection": "c1", "name": "color/brand", "type": "color", "values": {"m1": "#336699"}},
            {"id": "v2", "collection": "c1", "name": "radius/card", "type": "number", "values": {"m1": 3}},
            {"id": "v3", "collection": "c1", "name": "opacity/muted", "type": "number", "values": {"m1": 0.5}}
        ]
    })
}

/// The opacity of any node, a text included: a number or a binding to a variable.
fn opacity(r: &mut Rng, bind: bool) -> Value {
    if bind && r.chance(3) {
        json!({"var": "v3"})
    } else {
        json!(0.5)
    }
}

/// The paint of a box that is not a text.
fn paint(r: &mut Rng, bind: bool) -> Value {
    let mut n = serde_json::Map::new();
    if r.chance(2) {
        let f = if bind && r.chance(3) {
            json!({"var": "v1"})
        } else {
            json!(*r.pick(&COLOURS))
        };
        n.insert("fill".into(), f);
    } else {
        n.insert("fill".into(), json!("none"));
    }
    if r.chance(3) {
        let v = if bind && r.chance(3) {
            json!({"var": "v2"})
        } else {
            json!(1 + r.below(4))
        };
        n.insert("radius".into(), v);
    }
    if r.chance(4) {
        let pos = *r.pick(&["inside", "center", "outside"]);
        n.insert(
            "border".into(),
            json!({"width": 1 + r.below(3), "color": *r.pick(&COLOURS), "position": pos}),
        );
    }
    if r.chance(4) {
        let list: Vec<Value> = (0..1 + r.below(2))
            .map(|_| json!({"x": r.below(6), "y": r.below(6), "blur": r.below(12), "spread": r.below(4), "color": *r.pick(&COLOURS)}))
            .collect();
        n.insert("shadows".into(), json!(list));
    }
    if r.chance(5) {
        n.insert("gradient".into(), json!({"kind": "linear", "angle": 90, "stops": [{"pos": 0, "color": "#ff0000"}, {"pos": 1, "color": "#0000ff"}]}));
    }
    if r.chance(6) {
        n.insert(
            "image".into(),
            json!({"assetId": "ab".repeat(32), "fit": *r.pick(&["cover", "contain", "fill"])}),
        );
    }
    if r.chance(4) {
        n.insert("opacity".into(), opacity(r, bind));
    }
    Value::Object(n)
}

fn container(r: &mut Rng) -> Value {
    let mut n = json!({"clip": r.chance(2), "layout": *r.pick(&["row", "column"]), "gap": r.below(12), "padding": r.below(16),
        "align": *r.pick(&["start", "center", "end"]), "justify": *r.pick(&["start", "center", "end", "between"])});
    if r.chance(4) {
        merge(&mut n, json!({"padT": r.below(9), "padL": r.below(9)}));
    }
    n
}

fn child(r: &mut Rng, parent: &str, i: usize, depth: usize, bind: bool, out: &mut Vec<Value>) {
    let id = format!("{parent}-{i}");
    let kind = if depth < 2 && r.chance(4) {
        "frame"
    } else {
        *r.pick(&["rect", "rect", "ellipse", "text"])
    };
    let mut n = json!({"id": id, "name": format!("Node {id}"), "type": kind, "parentId": parent, "pos": format!("a{i}"),
        "x": r.below(50), "y": r.below(50), "w": 20 + r.below(80), "h": 10 + r.below(40)});
    if kind != "text" {
        merge(&mut n, paint(r, bind));
    }
    if r.chance(4) {
        merge(
            &mut n,
            json!({"sw": *r.pick(&["fill", "hug", "fixed"]), "sh": *r.pick(&["fill", "hug", "fixed"])}),
        );
    }
    if r.chance(5) {
        merge(&mut n, json!({"marT": r.below(8), "marL": r.below(8)}));
    }
    if r.chance(8) {
        merge(&mut n, json!({"absolute": true}));
    }
    match kind {
        "text" => {
            merge(
                &mut n,
                json!({"text": *r.pick(&WORDS), "fontSize": 10 + r.below(14), "fontWeight": *r.pick(&[400, 600, 700]),
                    "textAlign": *r.pick(&["left", "center", "right"]), "fill": *r.pick(&COLOURS)}),
            );
            // A text keeps its opacity (a literal, or a binding that comes back as its value).
            if r.chance(3) {
                merge(&mut n, json!({"opacity": opacity(r, bind)}));
            }
        }
        "frame" => merge(&mut n, container(r)),
        _ => {}
    }
    out.push(n);
    if kind == "frame" {
        // A frame with a layout is a frame when it holds nothing; the generator never makes one without a layout.
        for k in 0..r.below(3) as usize {
            child(r, &format!("{parent}-{i}"), k, depth + 1, bind, out);
        }
    }
}

fn file(seed: u64, bind: bool) -> Vec<Value> {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let mut root = json!({"id": "f", "name": "Root frame", "type": "frame", "x": 10, "y": 20, "w": 300 + r.below(200), "h": 200 + r.below(200)});
    merge(&mut root, container(&mut r));
    merge(&mut root, paint(&mut r, bind));
    let mut out = vec![root];
    for i in 0..1 + r.below(5) as usize {
        child(&mut r, "f", i, 0, bind, &mut out);
    }
    out
}

/// The nodes in the order of the export: a parent, then its children by position.
fn order<'a>(nodes: &'a [Node], id: &str, out: &mut Vec<&'a Node>) {
    let me = nodes.iter().find(|n| n.id == id).unwrap();
    out.push(me);
    let mut kids: Vec<&Node> = nodes
        .iter()
        .filter(|n| n.parent_id.as_deref() == Some(id))
        .collect();
    kids.sort_by(|a, b| a.pos.cmp(&b.pos).then_with(|| a.id.cmp(&b.id)));
    for k in kids {
        order(nodes, &k.id, out);
    }
}

fn imported_nodes(i: &Imported) -> Vec<Node> {
    i.commands
        .iter()
        .map(|c| match c {
            meldframe_protocol::Command::AddNode { node, .. } => node.clone(),
            _ => unreachable!(),
        })
        .collect()
}

/// How the export writes an axis: a fill with no layout parent to share from, or whose parent hugs on that axis, is
/// fixed (rule G1), and a hug that is neither a text nor a layout frame is fixed too.
fn effective(n: &Node, mode: SizeMode, parent: Option<&Node>, horizontal: bool) -> SizeMode {
    let layout = |x: &Node| x.kind == NodeKind::Frame && x.layout != LayoutMode::None;
    match mode {
        SizeMode::Fixed => SizeMode::Fixed,
        SizeMode::Hug if n.kind == NodeKind::Text || layout(n) => SizeMode::Hug,
        SizeMode::Hug => SizeMode::Fixed,
        SizeMode::Fill => match parent.filter(|p| layout(p) && !n.absolute) {
            Some(p) if (if horizontal { p.sw } else { p.sh }) != SizeMode::Hug => SizeMode::Fill,
            _ => SizeMode::Fixed,
        },
    }
}

fn resolved(n: &Node, vars: &Value) -> Node {
    let lit = |id: &str| {
        vars["variables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == id)
            .unwrap()["values"]["m1"]
            .clone()
    };
    let mut n = n.clone();
    if let Some(v) = n.fill.var_id() {
        n.fill = serde_json::from_value(lit(v)).unwrap();
    }
    if let Some(v) = n.opacity.var_id() {
        n.opacity = serde_json::from_value(lit(v)).unwrap();
    }
    if let Some(v) = n.radius.var_id() {
        n.radius = serde_json::from_value(lit(v)).unwrap();
    }
    n
}

/// Export and import one file. In a layout the contract leaves unspecified (a box smaller than its padding, overflow,
/// a fill of nothing: rule G3; the export warns of it) a box can have a negative size, and the radius of its border
/// is then written as a negative length, which is the one thing the import may list as unreadable. `true` when the
/// file had no such layout.
fn check(
    seed: u64,
    bind: bool,
    format: Format,
    nodes: &[Node],
    vars: &Variables,
    vjson: &Value,
) -> bool {
    let ctx =
        |what: &str| format!("seed {seed} {format:?}: {what}; replay with MELDFRAME_SEED={seed}");
    let exp = export_frame(nodes, vars, "f", &ExportOptions::new(format, &StubMeasurer))
        .unwrap_or_else(|e| panic!("{}", ctx(&e.to_string())));
    let unspecified = ["smaller_than_padding", "overflow", "zero_fill"];
    let degenerate = exp
        .report
        .warnings
        .iter()
        .any(|w| unspecified.contains(&w.code));
    let css = if format == Format::Tailwind {
        exp.css.as_str()
    } else {
        ""
    };
    let opts = Options {
        origin: (10.0, 20.0),
        ..Options::default()
    };
    let got = import(&exp.html, css, &opts).unwrap_or_else(|e| panic!("{}", ctx(&e.to_string())));
    let refused: Vec<_> = got
        .nodes
        .iter()
        .flat_map(|n| n.unsupported.iter().map(move |l| (&n.id, l)))
        .filter(|(_, l)| !(degenerate && l.code == "radius_unreadable"))
        .collect();
    assert!(
        refused.is_empty(),
        "{}: {refused:?}\n{}",
        ctx("an export has nothing unsupported"),
        exp.html
    );
    let back = imported_nodes(&got);
    let mut want = Vec::new();
    order(nodes, "f", &mut want);
    assert_eq!(want.len(), back.len(), "{}", ctx("the number of nodes"));
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    for (o, i) in want.iter().zip(&back) {
        let o = &if bind {
            resolved(o, vjson)
        } else {
            (*o).clone()
        };
        let at = |f: &str| {
            ctx(&format!(
                "{} ({}) {:?} {}x{} r{:?}: {f}",
                o.id, o.name, o.kind, o.w, o.h, o.radius
            ))
        };
        let parent = o.parent_id.as_deref().and_then(|p| by_id.get(p)).copied();
        let flow = parent.is_some_and(|p| p.layout != LayoutMode::None);
        let (kind, name) = (o.kind, &o.name);
        assert_eq!((i.kind, &i.name), (kind, name), "{}", at("kind and name"));
        for (axis, h) in [("sw", true), ("sh", false)] {
            let (m, got) = if h { (o.sw, i.sw) } else { (o.sh, i.sh) };
            let want = effective(o, m, parent, h);
            assert_eq!(got, want, "{}", at(axis));
            if want == SizeMode::Fixed {
                let (a, b) = if h { (o.w, i.w) } else { (o.h, i.h) };
                assert_eq!(a, b, "{}", at(&format!("{axis} px")));
            }
        }
        if parent.is_some() && (!flow || o.absolute) {
            assert_eq!((o.x, o.y), (i.x, i.y), "{}", at("x and y"));
        }
        assert_eq!(
            (o.fill.clone(), &o.gradient, &o.image),
            (i.fill.clone(), &i.gradient, &i.image),
            "{}",
            at("paint")
        );
        assert_eq!(
            (&o.border, &o.shadows, &o.opacity),
            (&i.border, &i.shadows, &i.opacity),
            "{}",
            at("border, shadows, opacity")
        );
        if kind != NodeKind::Ellipse {
            // An ellipse is `border-radius: 50%`: its stored radius is not written and not read back.
            assert_eq!(o.radius, i.radius, "{}", at("radius"));
        }
        if kind == NodeKind::Frame {
            assert_eq!(o.layout, i.layout, "{}", at("layout"));
            assert_eq!(o.clip, i.clip, "{}", at("clip"));
            if o.layout != LayoutMode::None {
                assert_eq!(
                    (o.gap, o.pad(), o.align, o.justify),
                    (i.gap, i.pad(), i.align, i.justify),
                    "{}",
                    at("flow")
                );
            }
        }
        if kind == NodeKind::Text {
            assert_eq!(
                (&o.text, o.font_size, o.font_weight, o.text_align),
                (&i.text, i.font_size, i.font_weight, i.text_align),
                "{}",
                at("text")
            );
        }
        if flow && !o.absolute {
            assert_eq!(o.margin(), i.margin(), "{}", at("margin"));
        }
        assert_eq!(o.absolute && flow, i.absolute, "{}", at("absolute"));
    }
    if !bind {
        fixed_point(
            format,
            &exp.html,
            &back,
            ctx("the export of the import is the export"),
        );
    }
    !degenerate
}

/// An export of the imported file is the export of the first, with the ids renamed in order.
fn fixed_point(format: Format, first: &str, back: &[Node], msg: String) {
    let mut again: Vec<Node> = back.to_vec();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for n in &mut again {
        let k = seen
            .entry(n.parent_id.clone().unwrap_or_default())
            .or_insert(0);
        n.pos = format!("a{k:03}");
        *k += 1;
    }
    let root = again[0].id.clone();
    let vars: Variables =
        serde_json::from_value(json!({"collections": [], "variables": []})).unwrap();
    let exp = export_frame(
        &again,
        &vars,
        &root,
        &ExportOptions::new(format, &StubMeasurer),
    )
    .unwrap();
    let ids_of = |html: &str| -> Vec<String> {
        html.split("data-node=\"")
            .skip(1)
            .map(|r| r.split('"').next().unwrap().to_owned())
            .collect()
    };
    let (a, b) = (
        strip(first, &ids_of(first)),
        strip(&exp.html, &ids_of(&exp.html)),
    );
    assert_eq!(a, b, "{msg}");
}

/// The HTML with each `data-node` id replaced by its number in order.
fn strip(html: &str, ids: &[String]) -> String {
    let mut out = html.to_owned();
    for (n, id) in ids.iter().enumerate() {
        out = out.replace(&format!("\"{id}\""), &format!("\"N{n}\""));
    }
    out
}

fn seeds() -> std::ops::Range<u64> {
    if let Ok(s) = std::env::var("MELDFRAME_SEED") {
        let n: u64 = s.parse().unwrap();
        return n..n + 1;
    }
    let default = if cfg!(target_arch = "wasm32") {
        20
    } else {
        300
    };
    let n = std::env::var("MELDFRAME_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default);
    0..n
}

#[test]
fn a_random_file_exported_and_imported_comes_back_in_both_forms() {
    // What the files of the run held, so that a run that never made a border or an ellipse says so.
    let mut seen = [false; 11];
    let (mut checked, mut total) = (0, 0);
    for seed in seeds() {
        for bind in [false, true] {
            let nodes: Vec<Node> =
                serde_json::from_value(Value::Array(file(seed, bind))).expect("nodes");
            let vjson = variables();
            let vars: Variables = serde_json::from_value(vjson.clone()).unwrap();
            for format in [Format::Css, Format::Tailwind] {
                checked += usize::from(check(seed, bind, format, &nodes, &vars, &vjson));
                total += 1;
            }
            for n in &nodes {
                let hits = [
                    n.kind == NodeKind::Text,
                    n.kind == NodeKind::Ellipse,
                    n.kind == NodeKind::Rect,
                    n.border.width > 0.0,
                    !n.shadows.is_empty(),
                    n.gradient.is_some(),
                    n.image.is_some(),
                    n.absolute,
                    n.fill.var_id().is_some() || n.radius.var_id().is_some(),
                    n.kind == NodeKind::Text && n.opacity.var_id().is_some(),
                    n.kind == NodeKind::Text && n.opacity.lit().is_some_and(|o| *o < 1.0),
                ];
                for (s, h) in seen.iter_mut().zip(hits) {
                    *s |= h;
                }
            }
        }
    }
    if std::env::var("MELDFRAME_SEED").is_err() {
        // Some files are in a layout the contract leaves unspecified; that is not most of them.
        assert!(
            checked * 2 >= total,
            "only {checked} of {total} round trips were in a specified layout"
        );
        assert!(
            seen.iter().all(|s| *s),
            "the files did not cover everything: {seen:?}"
        );
    }
}
