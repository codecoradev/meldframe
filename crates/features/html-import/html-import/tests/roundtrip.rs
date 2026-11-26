//! The importer reads what the export writes: frames exported in the css form and in the Tailwind form, read back
//! with `style_tree`, have the same computed style for every element. A few hundred seeded random frames (rects,
//! ellipses, texts, nested frames with layout, fills, gradients, images under an asset base with odd characters,
//! borders on `::after`, shadows, bound variables). With the `css` the Tailwind export returns there are no
//! differences; without it, exactly the two the docs name: the reset (`p{margin:0}`) and the variables that live in
//! it. Seeds per run: 300 natively, 10 under WASM; `MELDFRAME_SEEDS` sets the count and `MELDFRAME_SEED=n` replays one.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use export::{ExportOptions, Format, export_frame};
use html_import::css::{Limits, Report, Style, style_tree};
use html_import::html::{Tree, parse};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::{Node, Variables};
#[path = "roundtrip_common/mod.rs"]
mod common;
use common::{Rng, merge};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const COLOURS: [&str; 6] = [
    "#ff0000",
    "#3366ff",
    "#00000080",
    "#abcdef",
    "#112233",
    "#ffffff",
];
const WORDS: [&str; 6] = [
    "Hello",
    "a < b & \"c\"",
    "Lorem ipsum",
    "x",
    "Quick brown fox",
    "100%",
];
const BASES: [&str; 4] = [
    "assets/",
    "assets/a b&c'd\"e/",
    "https://cdn.example/x_y/",
    "a\\b;c{d}/",
];

fn variables() -> Value {
    json!({
        "collections": [{"id": "c1", "name": "Theme", "modes": [{"id": "m1", "name": "Light"}]}],
        "variables": [
            {"id": "v1", "collection": "c1", "name": "color/brand", "type": "color", "values": {"m1": "#336699"}},
            {"id": "v2", "collection": "c1", "name": "radius/card", "type": "number", "values": {"m1": 6}},
            {"id": "v3", "collection": "c1", "name": "opacity/muted", "type": "number", "values": {"m1": 0.5}}
        ]
    })
}

/// The paint of a box: a few of the fields that the export writes, chosen at random.
fn paint(r: &mut Rng, kind: &str) -> Value {
    let mut n = serde_json::Map::new();
    if r.chance(3) {
        n.insert(
            "fill".into(),
            if r.chance(3) {
                json!({"var": "v1"})
            } else {
                json!(*r.pick(&COLOURS))
            },
        );
    }
    if kind != "text" {
        if r.chance(3) {
            n.insert(
                "radius".into(),
                if r.chance(3) {
                    json!({"var": "v2"})
                } else {
                    json!(r.below(20))
                },
            );
        }
        if r.chance(4) {
            let pos = *r.pick(&["inside", "center", "outside"]);
            n.insert(
                "border".into(),
                json!({"width": 1 + r.below(4), "color": *r.pick(&COLOURS), "position": pos}),
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
    }
    if r.chance(4) {
        n.insert(
            "opacity".into(),
            if r.chance(3) {
                json!({"var": "v3"})
            } else {
                json!(0.5)
            },
        );
    }
    Value::Object(n)
}

fn child(r: &mut Rng, parent: &str, i: usize, depth: usize, out: &mut Vec<Value>) {
    let id = format!("{parent}-{i}");
    let kind = if depth < 2 && r.chance(4) {
        "frame"
    } else {
        *r.pick(&["rect", "rect", "ellipse", "text"])
    };
    let mut n = json!({"id": id, "type": kind, "parentId": parent, "pos": format!("a{i}"), "x": r.below(50), "y": r.below(50), "w": 20 + r.below(80), "h": 10 + r.below(40)});
    merge(&mut n, paint(r, kind));
    if r.chance(4) {
        merge(
            &mut n,
            json!({"sw": *r.pick(&["fill", "hug", "fixed"]), "sh": *r.pick(&["fill", "hug", "fixed"])}),
        );
    }
    if r.chance(5) {
        merge(&mut n, json!({"marT": r.below(8), "marL": r.below(8)}));
    }
    match kind {
        "text" => merge(
            &mut n,
            json!({"text": *r.pick(&WORDS), "fontSize": 10 + r.below(14), "fontWeight": *r.pick(&[400, 600, 700]), "textAlign": *r.pick(&["left", "center", "right"])}),
        ),
        "frame" => merge(&mut n, container(r)),
        _ => {}
    }
    out.push(n);
    if kind == "frame" {
        for k in 0..r.below(3) as usize {
            child(r, &format!("{parent}-{i}"), k, depth + 1, out);
        }
    }
}

fn container(r: &mut Rng) -> Value {
    let mut n = json!({"clip": r.chance(2)});
    if !r.chance(4) {
        merge(
            &mut n,
            json!({"layout": *r.pick(&["row", "column"]), "gap": r.below(12), "padding": r.below(16),
            "align": *r.pick(&["start", "center", "end"]), "justify": *r.pick(&["start", "center", "end", "between"])}),
        );
        if r.chance(4) {
            merge(&mut n, json!({"padT": r.below(9), "padL": r.below(9)}));
        }
    }
    n
}

fn file(seed: u64) -> Vec<Value> {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let mut root = json!({"id": "f", "type": "frame", "x": 10, "y": 20, "w": 300 + r.below(200), "h": 200 + r.below(200)});
    merge(&mut root, container(&mut r));
    merge(&mut root, paint(&mut r, "frame"));
    let mut out = vec![root];
    for i in 0..1 + r.below(5) as usize {
        child(&mut r, "f", i, 0, &mut out);
    }
    out
}

struct Read {
    tree: Tree,
    report: Report,
    styles: html_import::css::Styles,
}

fn read(html: &str, css: &str) -> Read {
    let tree = parse(html, &Default::default())
        .expect("the export parses")
        .tree;
    let mut report = Report::default();
    let styles = style_tree(&tree, css, &Limits::default(), &mut report);
    Read {
        tree,
        report,
        styles,
    }
}

/// What the importer made of one element.
struct View {
    props: BTreeMap<String, String>,
    after: BTreeMap<String, String>,
    typed: String,
    notes: Vec<String>,
}

/// The elements that are nodes, by their `data-node`, and the `body`.
fn view(r: &Read) -> BTreeMap<String, (String, View)> {
    let mut out = BTreeMap::new();
    for id in r.tree.ids() {
        let el = r.tree.element(id);
        let key = match (el.attr("data-node"), el.tag.as_str()) {
            (Some(n), _) => n.to_owned(),
            (None, "body") => "body".into(),
            _ => continue,
        };
        let s: &Style = r.styles.get(id);
        let map = |p: &html_import::css::Props| {
            p.iter()
                .map(|(k, v)| (k.to_string(), v.value.to_string()))
                .collect()
        };
        let v = View {
            props: map(&s.props),
            after: s.after.as_ref().map(map).unwrap_or_default(),
            typed: format!(
                "{} {} {} {:?}",
                s.color, s.font_size, s.font_weight, s.custom
            ),
            notes: s
                .notes
                .iter()
                .map(|n| format!("{}:{}", n.property, n.code))
                .collect(),
        };
        out.insert(key, (el.tag.clone(), v));
    }
    out
}

fn same(a: &BTreeMap<String, (String, View)>, b: &BTreeMap<String, (String, View)>) -> Vec<String> {
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    let eq = |x: &View, y: &View| {
        (&x.props, &x.after, &x.typed, &x.notes) == (&y.props, &y.after, &y.typed, &y.notes)
    };
    a.iter()
        .filter(|(k, (_, v))| !eq(v, &b[*k].1))
        .map(|(k, _)| k.clone())
        .collect()
}

/// What the frames of a run had, so that a run that never made a border or an image says so.
#[derive(Default)]
struct Seen {
    reset: bool,
    variables: bool,
    border: bool,
    image: bool,
    shadow: bool,
    gradient: bool,
    flex: bool,
    text: bool,
}

fn run_seed(seed: u64, seen: &mut Seen) {
    let nodes: Vec<Node> =
        serde_json::from_value(Value::Array(file(seed))).expect("the generated nodes are nodes");
    let vars: Variables = serde_json::from_value(variables()).unwrap();
    let base = BASES[seed as usize % BASES.len()];
    let go = |format| {
        let mut o = ExportOptions::new(format, &StubMeasurer);
        o.asset_base = base.into();
        export_frame(&nodes, &vars, "f", &o).unwrap_or_else(|e| panic!("seed {seed}: {e}"))
    };
    let (css_form, tw_form) = (go(Format::Css), go(Format::Tailwind));
    let from_css = read(&css_form.html, "");
    let with = read(&tw_form.html, &tw_form.css);
    let without = read(&tw_form.html, "");
    let ctx = |what: &str| format!("seed {seed}: {what}; replay with MELDFRAME_SEED={seed}");
    // With the stylesheet the export returns: no difference at all, no class left over, nothing refused.
    let (a, b) = (view(&from_css), view(&with));
    let d = same(&a, &b);
    assert!(
        d.is_empty(),
        "{}: {:?}",
        ctx("css form and tailwind form differ"),
        d.iter()
            .map(|k| (&a[k].1.props, &b[k].1.props))
            .collect::<Vec<_>>()
    );
    for (tag, v) in a.values() {
        seen.border |= v.after.contains_key("border-top-width");
        seen.image |= v
            .props
            .get("background-image")
            .is_some_and(|b| b.contains("url("));
        seen.shadow |= v.props.contains_key("box-shadow");
        seen.gradient |= v
            .props
            .get("background-image")
            .is_some_and(|b| b.contains("-gradient("));
        seen.flex |= v.props.get("display").is_some_and(|d| d == "flex");
        seen.text |= tag == "p" && v.props.contains_key("line-height");
    }
    // The two forms agree with each other; they must also agree with the model. A few fields that the export writes
    // as plain numbers are read back as the node's own.
    for n in file(seed) {
        let (id, kind) = (n["id"].as_str().unwrap(), n["type"].as_str().unwrap());
        let props = &a[id].1.props;
        let px = |name: &str| {
            let v = props.get(name).and_then(|v| v.strip_suffix("px"));
            v.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
        };
        if kind == "text" {
            assert_eq!(
                px("font-size"),
                n["fontSize"].as_f64().unwrap(),
                "{}",
                ctx("the font size of a text")
            );
            assert_eq!(
                props["font-weight"],
                n["fontWeight"].to_string(),
                "{}",
                ctx("the font weight of a text")
            );
        }
        if kind == "frame" && n.get("layout").is_some() {
            for (side, key) in [
                ("top", "padT"),
                ("left", "padL"),
                ("right", "padR"),
                ("bottom", "padB"),
            ] {
                let want = n
                    .get(key)
                    .and_then(Value::as_f64)
                    .unwrap_or_else(|| n["padding"].as_f64().unwrap());
                assert_eq!(
                    px(&format!("padding-{side}")),
                    want,
                    "{}",
                    ctx("the padding of a frame")
                );
            }
            assert_eq!(
                px("row-gap"),
                n["gap"].as_f64().unwrap(),
                "{}",
                ctx("the gap of a frame")
            );
        }
    }
    for r in [&from_css, &with] {
        assert_eq!(
            r.report.counts.get("class_unresolved"),
            None,
            "{}",
            ctx("a class of the export is not read")
        );
        assert!(r.styles.complete, "{}", ctx("a bound was reached"));
        assert!(
            view(r).values().all(|(_, v)| v.notes.is_empty()),
            "{}",
            ctx("a note on a node")
        );
    }
    // Without it, only what the stylesheet holds is missing. The reset (`*{box-sizing}`, `body{margin;font-family}`,
    // `p{margin:0}`) is not there, so a paragraph and the body have the browser's margin; and the variables are not
    // there, so what is bound to one is unset with a note.
    let c = view(&without);
    for (k, (tag, x)) in &a {
        let y = &c[k].1;
        let reset = |name: &str| {
            name == "box-sizing"
                || name == "font-family"
                || (name.starts_with("margin-") && (tag == "p" || k == "body"))
        };
        let had = |m: &BTreeMap<String, String>| -> BTreeMap<String, String> {
            m.iter()
                .filter(|(n, _)| !reset(n))
                .map(|(n, v)| (n.clone(), v.clone()))
                .collect()
        };
        for (name, v) in had(&y.props) {
            assert_eq!(
                x.props.get(&name),
                Some(&v),
                "{}",
                ctx(&format!("{k}: {name} differs without the stylesheet"))
            );
        }
        let missing = had(&x.props)
            .keys()
            .filter(|n| !y.props.contains_key(*n))
            .count()
            + had(&x.after)
                .keys()
                .filter(|n| !y.after.contains_key(*n))
                .count();
        assert!(
            missing == 0 || !y.notes.is_empty(),
            "{}",
            ctx(&format!("{k}: properties are missing and nothing says why"))
        );
        assert!(
            y.notes.iter().all(|n| n.ends_with("var_undefined")),
            "{}",
            ctx("another note")
        );
        seen.variables |= !y.notes.is_empty();
        seen.reset |= tag == "p"
            && y.props.get("margin-top").map(String::as_str) == Some("1em")
            && x.props.get("margin-top").map(String::as_str) != Some("1em");
    }
}

fn seeds() -> std::ops::Range<u64> {
    let env = |n: &str| std::env::var(n).ok().and_then(|v| v.parse::<u64>().ok());
    let default = if cfg!(target_arch = "wasm32") {
        10
    } else {
        300
    };
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(default),
    }
}

#[test]
fn the_css_form_and_the_tailwind_form_of_random_frames_read_back_the_same() {
    let mut seen = Seen::default();
    for seed in seeds() {
        run_seed(seed, &mut seen);
    }
    // Both documented differences happen in a run of any size, so the check above is not vacuous.
    if seeds().end - seeds().start >= 10 {
        assert!(seen.reset, "no frame had a paragraph without the reset");
        assert!(
            seen.variables,
            "no frame had a bound field without the variables"
        );
    }
}
