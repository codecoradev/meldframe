//! The paint stage reads what the export writes: seeded random rects, ellipses and frames with a fill, a gradient, an
//! image, a border (in its three positions), a radius, shadows and an opacity, some of them bound to a variable, are
//! exported in the css and in the Tailwind form, read back with `style_tree` and `paint::read`, and the paint of
//! every node is the node's own. What does not come back is the list of `docs/import-html.md` section 3.7 and is
//! checked as such: an ellipse comes back as the flag, not as a radius. Seeds per run: 300 natively, 10 under WASM;
//! `MELDFRAME_SEEDS` sets the count and `MELDFRAME_SEED=n` replays one.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use export::{ExportOptions, Format, export_frame};
use html_import::css::{Limits, Report, style_tree};
use html_import::html::parse;
use html_import::paint::{self, Ctx};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::{Bind, BorderPosition, GradientKind, Node, NodeKind, Variables};
use serde_json::{Value, json};
use std::collections::BTreeMap;

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

    fn pick<'a, T>(&mut self, of: &'a [T]) -> &'a T {
        &of[self.below(of.len() as u64) as usize]
    }

    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

const COLOURS: [&str; 6] = [
    "#ff0000",
    "#3366ff",
    "#00000080",
    "#abcdef",
    "#112233",
    "#ffffff",
];
const BASES: [&str; 3] = ["assets/", "assets/a b&c'd\"e/", "https://cdn.example/x_y/"];

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

fn paint_fields(r: &mut Rng) -> Value {
    let mut n = serde_json::Map::new();
    if r.chance(2) {
        n.insert(
            "fill".into(),
            if r.chance(3) {
                json!({"var": "v1"})
            } else {
                json!(*r.pick(&COLOURS))
            },
        );
    }
    if r.chance(2) {
        n.insert(
            "radius".into(),
            if r.chance(3) {
                json!({"var": "v2"})
            } else {
                json!(r.below(40))
            },
        );
    }
    if r.chance(2) {
        let pos = *r.pick(&["inside", "center", "outside"]);
        n.insert(
            "border".into(),
            json!({"width": 1 + r.below(5), "color": *r.pick(&COLOURS), "position": pos}),
        );
    }
    if r.chance(3) {
        let list: Vec<Value> = (0..1 + r.below(4))
            .map(|_| json!({"x": r.below(6) as i64 - 2, "y": r.below(6), "blur": r.below(12), "spread": r.below(4) as i64 - 1, "color": *r.pick(&COLOURS)}))
            .collect();
        n.insert("shadows".into(), json!(list));
    }
    if r.chance(3) {
        let stops: Vec<Value> = (0..2 + r.below(4))
            .map(|i| json!({"pos": (i as f64) / 5.0, "color": *r.pick(&COLOURS)}))
            .collect();
        let g = if r.chance(3) {
            json!({"kind": "radial", "stops": stops})
        } else {
            json!({"kind": "linear", "angle": *r.pick(&[0, 45, 90, 135, 180, 270]), "stops": stops})
        };
        n.insert("gradient".into(), g);
    }
    if r.chance(4) {
        n.insert(
            "image".into(),
            json!({"assetId": "ab".repeat(32), "fit": *r.pick(&["cover", "contain", "fill"])}),
        );
    }
    if r.chance(3) {
        n.insert(
            "opacity".into(),
            if r.chance(3) {
                json!({"var": "v3"})
            } else {
                json!(*r.pick(&[0.25, 0.5, 0.75]))
            },
        );
    }
    Value::Object(n)
}

fn file(seed: u64) -> Vec<Value> {
    let mut r = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let mut root =
        json!({"id": "f", "type": "frame", "x": 10, "y": 20, "w": 300, "h": 200, "clip": false});
    root.as_object_mut()
        .unwrap()
        .extend(paint_fields(&mut r).as_object().unwrap().clone());
    let mut out = vec![root];
    for i in 0..1 + r.below(5) as usize {
        let kind = *r.pick(&["rect", "rect", "ellipse"]);
        let mut n = json!({"id": format!("f-{i}"), "type": kind, "parentId": "f", "pos": format!("a{i}"), "x": r.below(50), "y": r.below(50), "w": 20 + r.below(80), "h": 10 + r.below(40)});
        n.as_object_mut()
            .unwrap()
            .extend(paint_fields(&mut r).as_object().unwrap().clone());
        out.push(n);
    }
    out
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa` as eight lower-case digits.
fn norm(c: &str) -> String {
    let h = c.trim_start_matches('#').to_ascii_lowercase();
    match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).chain("ff".chars()).collect(),
        4 => h.chars().flat_map(|c| [c, c]).collect(),
        6 => format!("{h}ff"),
        _ => h,
    }
}

fn lit_colour(b: &Bind<String>, vars: &Value) -> String {
    match b {
        Bind::Lit(s) => s.clone(),
        Bind::Var(r) => vars["variables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == r.var)
            .map(|v| v["values"]["m1"].as_str().unwrap().to_owned())
            .unwrap(),
    }
}

fn lit_number(b: &Bind<f64>, vars: &Value) -> f64 {
    match b {
        Bind::Lit(n) => *n,
        Bind::Var(r) => vars["variables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == r.var)
            .map(|v| v["values"]["m1"].as_f64().unwrap())
            .unwrap(),
    }
}

fn check(seed: u64, format: Format, html: &str, css: &str, nodes: &[Node], vars: &Value) {
    let ctx =
        |what: &str| format!("seed {seed} ({format:?}): {what}; replay with MELDFRAME_SEED={seed}");
    let tree = parse(html, &Default::default())
        .expect("the export parses")
        .tree;
    let mut report = Report::default();
    let styles = style_tree(&tree, css, &Limits::default(), &mut report);
    let mut by_id = BTreeMap::new();
    for id in tree.ids() {
        if let Some(n) = tree.element(id).attr("data-node") {
            by_id.insert(n.to_owned(), id);
        }
    }
    for n in nodes {
        let id = by_id[&n.id];
        let has_children = nodes.iter().any(|c| c.parent_id.as_deref() == Some(&n.id));
        let c = Ctx {
            width: Some(n.w),
            height: Some(n.h),
            has_children,
        };
        let mut rep = Report::default();
        let p = paint::read(styles.get(id), &c, tree.element(id).at, &mut rep);
        assert!(
            rep.entries.is_empty(),
            "{}: {:?}",
            ctx(&format!(
                "{}: the paint stage says something about an export",
                n.id
            )),
            rep.entries
        );
        let fill = lit_colour(&n.fill, vars);
        // A node with no `fill` key has the default grey, which the export writes as it is.
        assert_eq!(
            norm(&lit_colour(&p.fill, vars)),
            if fill == "none" {
                "none".into()
            } else {
                norm(&fill)
            },
            "{}",
            ctx(&format!("{}: fill", n.id))
        );
        assert_eq!(
            p.gradient.is_some(),
            n.gradient.is_some(),
            "{}",
            ctx("gradient present")
        );
        if let (Some(a), Some(b)) = (&p.gradient, &n.gradient) {
            assert_eq!(a.kind, b.kind, "{}", ctx("gradient kind"));
            if b.kind == GradientKind::Linear {
                assert!(
                    (a.angle - b.angle).abs() < 1e-9,
                    "{}",
                    ctx("gradient angle")
                );
            }
            assert_eq!(a.stops.len(), b.stops.len(), "{}", ctx("stops"));
            for (x, y) in a.stops.iter().zip(&b.stops) {
                assert!((x.pos - y.pos).abs() < 1e-4, "{}", ctx("stop position"));
                assert_eq!(norm(&x.color), norm(&y.color), "{}", ctx("stop colour"));
            }
        }
        assert_eq!(p.image, n.image, "{}", ctx(&format!("{}: image", n.id)));
        assert_eq!(p.border.width, n.border.width, "{}", ctx("border width"));
        if n.border.width > 0.0 {
            assert_eq!(
                norm(&p.border.color),
                norm(&n.border.color),
                "{}",
                ctx("border colour")
            );
            assert_eq!(
                p.border.position,
                n.border.position,
                "{}",
                ctx("border position")
            );
            assert_eq!(
                p.border_room,
                0.0,
                "{}",
                ctx("the export's border takes no room")
            );
        }
        assert_eq!(p.shadows.len(), n.shadows.len(), "{}", ctx("shadows"));
        for (x, y) in p.shadows.iter().zip(&n.shadows) {
            assert_eq!(
                (x.x, x.y, x.blur, x.spread),
                (y.x, y.y, y.blur, y.spread),
                "{}",
                ctx("shadow")
            );
            assert_eq!(norm(&x.color), norm(&y.color), "{}", ctx("shadow colour"));
        }
        assert_eq!(
            lit_number(&p.opacity, vars),
            lit_number(&n.opacity, vars),
            "{}",
            ctx(&format!("{}: opacity", n.id))
        );
        if n.kind == NodeKind::Ellipse {
            assert!(p.ellipse, "{}", ctx("an ellipse comes back as the flag"));
        } else {
            assert!(!p.ellipse, "{}", ctx("a radius is not an ellipse"));
            assert_eq!(
                lit_number(&p.radius, vars),
                lit_number(&n.radius, vars),
                "{}",
                ctx(&format!("{}: radius", n.id))
            );
        }
    }
}

fn run_seed(seed: u64, seen: &mut [bool; 6]) {
    let raw = file(seed);
    let nodes: Vec<Node> =
        serde_json::from_value(Value::Array(raw)).expect("the generated nodes are nodes");
    let vars_json = variables();
    let vars: Variables = serde_json::from_value(vars_json.clone()).unwrap();
    let base = BASES[seed as usize % BASES.len()];
    let go = |format| {
        let mut o = ExportOptions::new(format, &StubMeasurer);
        o.asset_base = base.into();
        export_frame(&nodes, &vars, "f", &o).unwrap_or_else(|e| panic!("seed {seed}: {e}"))
    };
    let (css_form, tw_form) = (go(Format::Css), go(Format::Tailwind));
    check(seed, Format::Css, &css_form.html, "", &nodes, &vars_json);
    check(
        seed,
        Format::Tailwind,
        &tw_form.html,
        &tw_form.css,
        &nodes,
        &vars_json,
    );
    for n in &nodes {
        seen[0] |= n.border.width > 0.0 && n.border.position == BorderPosition::Center;
        seen[1] |= n.border.width > 0.0 && n.border.position == BorderPosition::Outside;
        seen[2] |= n.border.width > 0.0 && matches!(n.radius, Bind::Var(_));
        seen[3] |= n.image.is_some() && n.gradient.is_some();
        seen[4] |= n.kind == NodeKind::Ellipse && n.border.width > 0.0;
        seen[5] |= n.shadows.len() >= 2;
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
fn the_paint_of_random_exported_nodes_is_read_back_in_both_forms() {
    let mut seen = [false; 6];
    for seed in seeds() {
        run_seed(seed, &mut seen);
    }
    if seeds().end - seeds().start >= 100 {
        let names = [
            "a centred border",
            "an outside border",
            "a bound radius with a border",
            "an image under a gradient",
            "a border on an ellipse",
            "two shadows",
        ];
        for (s, n) in seen.iter().zip(names) {
            assert!(s, "no frame had {n}");
        }
    }
}

#[test]
fn a_radius_the_box_clamps_comes_back_as_the_element_says_it() {
    // A radius of 50 on a 40 by 20 frame: the element says 50, the box of the border says 10.
    let raw = json!([{"id": "f", "type": "frame", "x": 0, "y": 0, "w": 40, "h": 20, "clip": false,
        "radius": 50, "border": {"width": 2, "color": "#ff0000", "position": "outside"}}]);
    let nodes: Vec<Node> = serde_json::from_value(raw).unwrap();
    let vars: Variables = serde_json::from_value(variables()).unwrap();
    for format in [Format::Css, Format::Tailwind] {
        let out = export_frame(
            &nodes,
            &vars,
            "f",
            &ExportOptions::new(format, &StubMeasurer),
        )
        .unwrap();
        check(
            0,
            format,
            &out.html,
            if format == Format::Css { "" } else { &out.css },
            &nodes,
            &variables(),
        );
    }
}
