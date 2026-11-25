//! The importer reads what the export writes: frames of seeded random shapes (rows and columns with gap, padding,
//! alignment, margins and clipping; fixed, hug and fill sizes; texts) are exported in the css and Tailwind forms,
//! planned back, and every field the mapping owns is compared with the original (`docs/import-html.md` section 3.7: the
//! ids, the positions and the stored sizes of what hugs or fills are the losses). Seeds per run: 300 natively, 10 under
//! WASM; `MELDFRAME_SEEDS` sets the count and `MELDFRAME_SEED=n` replays one.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use export::{ExportOptions, Format, export_frame};
use html_import::map::{Options, plan};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::{Node, NodeKind, SizeMode, Variables};
use serde_json::{Value, json};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
    fn pick<'a, T>(&mut self, of: &'a [T]) -> &'a T {
        &of[self.below(of.len() as u64) as usize]
    }
}

const WORDS: [&str; 5] = ["Hello", "Lorem ipsum dolor", "x", "Quick brown fox", "100%"];
const FILLS: [&str; 4] = ["#336699", "#ff0000", "#abcdef", "#112233"];

/// One level of the tree. `hug` says whether the parent hugs on the (width, height) axis, where a fill is not allowed.
fn child(
    r: &mut Rng,
    parent: &str,
    i: usize,
    depth: usize,
    hug: (bool, bool),
    out: &mut Vec<Value>,
) {
    let id = format!("{parent}-{i}");
    let kind = if depth < 2 && r.chance(3) {
        "frame"
    } else {
        *r.pick(&["text", "text", "rect"])
    };
    let mut n = json!({"id": id, "name": id, "type": kind, "parentId": parent, "pos": format!("a{i}"), "w": 20 + r.below(80), "h": 10 + r.below(40), "clip": false});
    let mode = |r: &mut Rng, hug: bool| {
        let mut m = vec!["fixed", "hug"];
        if !hug {
            m.push("fill");
        }
        *r.pick(&m)
    };
    let (mut sw, mut sh) = ("fixed", "fixed");
    if kind == "text" {
        sw = mode(r, hug.0);
        sh = if r.chance(2) { "hug" } else { sh };
        n["text"] = json!(*r.pick(&WORDS));
        n["fontSize"] = json!(10 + r.below(14));
        n["fontWeight"] = json!(*r.pick(&[400, 600, 700]));
        n["textAlign"] = json!(*r.pick(&["left", "center", "right"]));
        n["fill"] = json!(*r.pick(&FILLS));
    } else if kind == "rect" {
        n["fill"] = json!(*r.pick(&FILLS));
        if r.chance(4) {
            sw = "fill";
            if hug.0 {
                sw = "fixed";
            }
        }
    } else {
        sw = mode(r, hug.0);
        sh = mode(r, hug.1);
    }
    n["sw"] = json!(sw);
    n["sh"] = json!(sh);
    if r.chance(4) {
        n["marT"] = json!(r.below(8));
        n["marL"] = json!(r.below(8));
    }
    if kind == "frame" {
        n["layout"] = json!(*r.pick(&["row", "column"]));
        n["gap"] = json!(r.below(12));
        n["padding"] = json!(r.below(16));
        if r.chance(4) {
            n["padT"] = json!(r.below(9));
        }
        n["align"] = json!(*r.pick(&["start", "center", "end"]));
        n["justify"] = json!(*r.pick(&["start", "center", "end", "between"]));
        n["clip"] = json!(r.chance(2));
        n["fill"] = json!(*r.pick(&FILLS));
    }
    out.push(n);
    if kind == "frame" {
        let inner = (sw == "hug", sh == "hug");
        for k in 0..1 + r.below(3) as usize {
            child(r, &format!("{parent}-{i}"), k, depth + 1, inner, out);
        }
    }
}

fn file(seed: u64) -> Vec<Node> {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let root = json!({"id": "f", "name": "f", "type": "frame", "x": 10, "y": 20, "w": 300 + r.below(200), "h": 200 + r.below(200),
        "layout": *r.pick(&["row", "column"]), "gap": r.below(12), "padding": r.below(16),
        "align": *r.pick(&["start", "center", "end"]), "justify": *r.pick(&["start", "center", "end", "between"]),
        "clip": r.chance(2), "fill": *r.pick(&FILLS)});
    let mut out = vec![root];
    for i in 0..1 + r.below(4) as usize {
        child(&mut r, "f", i, 0, (false, false), &mut out);
    }
    out.into_iter()
        .map(|v| serde_json::from_value(v).expect("node"))
        .collect()
}

/// What differs between the original and the import, for the fields the mapping owns.
fn diff(o: &Node, n: &Node, root: bool) -> Vec<String> {
    let mut d = Vec::new();
    let mut cmp = |name: &str, a: String, b: String| {
        if a != b {
            d.push(format!("{} {name}: {a} became {b}", o.id));
        }
    };
    cmp("kind", format!("{:?}", o.kind), format!("{:?}", n.kind));
    cmp("name", o.name.clone(), n.name.clone());
    if root {
        cmp("x", o.x.to_string(), n.x.to_string());
        cmp("y", o.y.to_string(), n.y.to_string());
    }
    cmp("sw", format!("{:?}", o.sw), format!("{:?}", n.sw));
    cmp("sh", format!("{:?}", o.sh), format!("{:?}", n.sh));
    if o.sw == SizeMode::Fixed {
        cmp("w", o.w.to_string(), n.w.to_string());
    }
    if o.sh == SizeMode::Fixed {
        cmp("h", o.h.to_string(), n.h.to_string());
    }
    cmp(
        "margin",
        format!("{:?}", o.margin()),
        format!("{:?}", n.margin()),
    );
    cmp("fill", format!("{:?}", o.fill), format!("{:?}", n.fill));
    if o.kind == NodeKind::Text {
        cmp("text", o.text.clone(), n.text.clone());
        cmp("fontSize", o.font_size.to_string(), n.font_size.to_string());
        cmp(
            "fontWeight",
            o.font_weight.to_string(),
            n.font_weight.to_string(),
        );
        cmp(
            "textAlign",
            format!("{:?}", o.text_align),
            format!("{:?}", n.text_align),
        );
    }
    if o.kind == NodeKind::Frame {
        cmp(
            "layout",
            format!("{:?}", o.layout),
            format!("{:?}", n.layout),
        );
        cmp("gap", o.gap.to_string(), n.gap.to_string());
        cmp(
            "padding",
            format!("{:?}", o.pad()),
            format!("{:?}", n.pad()),
        );
        cmp("align", format!("{:?}", o.align), format!("{:?}", n.align));
        cmp(
            "justify",
            format!("{:?}", o.justify),
            format!("{:?}", n.justify),
        );
        cmp("clip", o.clip.to_string(), n.clip.to_string());
    }
    d
}

fn seeds() -> Vec<u64> {
    if let Some(s) = std::env::var("MELDFRAME_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return vec![s];
    }
    let default = if cfg!(target_arch = "wasm32") {
        10
    } else {
        300
    };
    let n = std::env::var("MELDFRAME_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default);
    (0..n).collect()
}

fn round(seed: u64, format: Format) -> Vec<String> {
    let nodes = file(seed);
    let measurer = StubMeasurer;
    let e = export_frame(
        &nodes,
        &Variables::default(),
        "f",
        &ExportOptions::new(format, &measurer),
    )
    .expect("export");
    // The css form carries its stylesheet in the document; the Tailwind form has the reset in the css it returns.
    let css = if format == Format::Tailwind {
        e.css.as_str()
    } else {
        ""
    };
    let p = plan(&e.html, css, &Options::default()).expect("plan");
    let got: Vec<Node> = common::nodes(&p);
    if got.len() != nodes.len() {
        return vec![format!("{} nodes became {}", nodes.len(), got.len())];
    }
    nodes
        .iter()
        .zip(&got)
        .enumerate()
        .flat_map(|(i, (o, n))| diff(o, n, i == 0))
        .collect()
}

#[test]
fn what_the_css_export_writes_comes_back() {
    for seed in seeds() {
        let d = round(seed, Format::Css);
        assert!(
            d.is_empty(),
            "seed {seed} (MELDFRAME_SEED={seed} replays it): {d:#?}"
        );
    }
}

#[test]
fn what_the_tailwind_export_writes_comes_back_with_its_css() {
    for seed in seeds() {
        let d = round(seed, Format::Tailwind);
        assert!(
            d.is_empty(),
            "seed {seed} (MELDFRAME_SEED={seed} replays it): {d:#?}"
        );
    }
}

#[test]
fn an_import_of_the_import_is_the_same_plan() {
    for seed in seeds().into_iter().take(40) {
        let nodes = file(seed);
        let m = StubMeasurer;
        let e = export_frame(
            &nodes,
            &Variables::default(),
            "f",
            &ExportOptions::new(Format::Css, &m),
        )
        .unwrap();
        let a = plan(&e.html, "", &Options::default()).unwrap();
        let b = plan(&e.html, "", &Options::default()).unwrap();
        assert_eq!(
            format!("{:?}", a.commands),
            format!("{:?}", b.commands),
            "seed {seed}"
        );
    }
}
