//! T9: a click and a drop answer "what is there" with one rule (`meldframe_render::HitIndex`). The frame a drop finds is
//! the front-most frame of what a click would find, in the case the drop used to read differently: a frame that a clip
//! cuts away at the pointer is no target.
//!
//! - Scenes: a drop over the part of a frame that its parent's clip cuts away.
//! - A property over random trees (frames with and without auto layout and clip, rectangles, texts under frames): for a grid
//!   of points and a random node that is dragged, `frame_under` (the drop) is the first frame of `HitIndex::under` (the hit)
//!   that is neither the node nor inside it.
//! - One known gap, pinned: a node under a rectangle, a text or an ellipse has no box in the layout (the layout core places
//!   the children of frames only), so a drop reads none for it while the canvas draws it and a click finds it. Seeds per run: 600 by default, 20 under WASM; `MELDFRAME_SEEDS` sets more and
//!   `MELDFRAME_SEED=n` replays one.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app_rules::drop::frame_under;
use meldframe_layout::{StubMeasurer, compute_rects};
use meldframe_protocol::{Node, Variables};
use meldframe_render::{HitIndex, ModeChoice, resolve_laid_out};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// A page with a clipping frame `c` (0 to 100) that holds a column `l` at (80, 80) of 100 by 100, so the part of `l` past 100
/// is cut away; `n` is a rectangle far from both.
fn clipped_column() -> Vec<Node> {
    serde_json::from_value(json!([
        {"id": "p", "type": "frame", "pos": "a0", "w": 1000, "h": 1000, "clip": false},
        {"id": "c", "type": "frame", "parentId": "p", "pos": "a0", "w": 100, "h": 100, "clip": true},
        {"id": "l", "type": "frame", "parentId": "c", "pos": "a0", "x": 80, "y": 80, "w": 100, "h": 100,
         "layout": "column", "padding": 10, "clip": false},
        {"id": "n", "type": "rect", "parentId": "p", "pos": "a1", "x": 500, "y": 500, "w": 20, "h": 20},
    ]))
    .unwrap()
}

#[test]
fn a_drop_over_the_visible_part_of_a_frame_goes_into_it() {
    let mut a = app_of(clipped_column(), true);
    drop_node(&mut a, "n", (90.0, 90.0), (-410.0, -410.0));
    assert_eq!(node(&a, "n").parent_id.as_deref(), Some("l"));
}

#[test]
fn a_drop_over_the_part_of_a_frame_that_a_clip_cuts_away_does_not_go_into_it() {
    // (150, 150) is inside the box of `l` and outside the clip of `c`: it is not drawn there, so a click there finds the page
    // and a drop finds the page too, where it used to find `l`.
    let mut a = app_of(clipped_column(), true);
    let boxes = a.scene();
    assert_eq!(
        meldframe_render::hit_test(&boxes, 150.0, 150.0).as_deref(),
        Some("p")
    );
    drop_node(&mut a, "n", (150.0, 150.0), (-350.0, -350.0));
    assert_eq!(node(&a, "n").parent_id.as_deref(), Some("p"));
}

#[test]
fn known_gap_a_frame_under_a_rectangle_is_hit_but_has_no_box_for_a_drop() {
    let nodes: Vec<Node> = serde_json::from_value(json!([
        {"id": "p", "type": "frame", "pos": "a0", "w": 1000, "h": 1000, "clip": false},
        {"id": "r", "type": "rect", "parentId": "p", "pos": "a0", "w": 100, "h": 100, "clip": false},
        {"id": "f", "type": "frame", "parentId": "r", "pos": "a0", "x": 10, "y": 10, "w": 50, "h": 50},
    ]))
    .unwrap();
    let boxes = resolve_laid_out(
        &nodes,
        &Variables::default(),
        &ModeChoice::default(),
        &StubMeasurer,
    );
    assert_eq!(
        meldframe_render::hit_test(&boxes, 20.0, 20.0).as_deref(),
        Some("f")
    );
    let rects = compute_rects(&nodes, &StubMeasurer);
    assert!(rects.iter().all(|(id, _)| id != "f"), "{rects:?}");
    let found = frame_under(&nodes, &rects, (20.0, 20.0), &HashSet::new());
    assert_eq!(found.map(|n| n.id.as_str()), Some("p"));
}

// ---- the property ------------------------------------------------------------------------------------------------

const DEFAULT_SEEDS: u64 = if cfg!(target_arch = "wasm32") {
    20
} else {
    600
};

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS),
    }
}

/// splitmix64.
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
}

/// A random tree of 3 to 14 nodes: a top frame, then frames, rectangles and texts under a frame already made, each with a
/// stored box on a 10-unit grid (so edges meet), and each frame with or without auto layout and clip.
fn random_tree(r: &mut Rng) -> Vec<Node> {
    let count = 3 + r.below(12) as usize;
    let mut list: Vec<Value> = Vec::new();
    let mut frames: Vec<String> = Vec::new();
    for i in 0..count {
        let id = format!("n{i}");
        let frame = i == 0 || r.below(3) == 0;
        let parent = if i == 0 {
            Value::Null
        } else {
            json!(r.pick(&frames))
        };
        let kind = if frame {
            "frame"
        } else {
            *r.pick(&["rect", "text", "ellipse"])
        };
        let layout = *r.pick(&["none", "none", "row", "column"]);
        list.push(json!({
            "id": id, "type": kind, "parentId": parent, "pos": format!("a{}", r.below(4)),
            "x": 10 * r.below(14), "y": 10 * r.below(14), "w": 10 * (1 + r.below(12)), "h": 10 * (1 + r.below(12)),
            "layout": layout, "gap": 10 * r.below(2), "padding": 10 * r.below(2),
            "clip": r.below(2) == 0, "text": "t", "fontSize": 14,
        }));
        if frame {
            frames.push(id);
        }
    }
    serde_json::from_value(Value::Array(list)).unwrap()
}

/// Whether `n` is `skip` or inside it.
fn inside(by_id: &HashMap<&str, &Node>, n: &Node, skip: &str) -> bool {
    let mut cur = Some(n);
    for _ in 0..=by_id.len() {
        match cur {
            Some(c) if c.id == skip => return true,
            Some(c) => cur = c.parent_id.as_deref().and_then(|p| by_id.get(p).copied()),
            None => return false,
        }
    }
    false
}

fn run_seed(seed: u64, probes: &mut usize, frames_found: &mut usize) {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let nodes = random_tree(&mut r);
    let boxes = resolve_laid_out(
        &nodes,
        &Variables::default(),
        &ModeChoice::default(),
        &StubMeasurer,
    );
    let rects = compute_rects(&nodes, &StubMeasurer);
    let index = HitIndex::from_boxes(&boxes);
    let by_id: HashMap<&str, &Node> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let dragged = r.pick(&nodes).id.clone();
    let skip = HashSet::from([dragged.as_str()]);
    // A grid of 10 units, over the boxes and a little past them, on the edges and between them.
    for gy in -1..=16 {
        for gx in -1..=16 {
            for (ox, oy) in [(0.0, 0.0), (5.0, 5.0)] {
                let at = (f64::from(gx) * 10.0 + ox, f64::from(gy) * 10.0 + oy);
                let want = index.under(at.0, at.1).find(|id| {
                    let n = by_id[id];
                    n.kind == meldframe_protocol::NodeKind::Frame && !inside(&by_id, n, &dragged)
                });
                let got = frame_under(&nodes, &rects, at, &skip).map(|n| n.id.as_str());
                assert_eq!(got, want, "seed {seed}, {dragged} dragged, at {at:?}");
                *probes += 1;
                *frames_found += usize::from(got.is_some());
            }
        }
    }
}

#[test]
fn drop_frame_under_agrees_with_hit_test() {
    let (mut probes, mut found) = (0, 0);
    for seed in seeds() {
        run_seed(seed, &mut probes, &mut found);
    }
    assert!(
        found * 20 > probes,
        "the points land on frames: {found} of {probes}"
    );
}
