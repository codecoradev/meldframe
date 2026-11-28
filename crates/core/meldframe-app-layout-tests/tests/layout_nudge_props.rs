//! Property tests for the nudge (`Action::Nudge`, E8.1.1): random selections and random sequences of nudges over two files with
//! free nodes, flow children, an absolute child and nested nodes, with the layout on the canvas on and off, and with nodes put
//! at the limit of a coordinate now and then. Checked after every press:
//!
//! - an oracle written here from the parents of the nodes and the list of the auto layout frames (nothing of the rule is
//!   reused) says where every node lands: a node outside a flow that has no moving selected ancestor moves by the step once,
//!   a node under a moving selected ancestor follows it and is not moved twice, a flow child (layout on) is never given a
//!   stored position and, with its frame or an ancestor of it moving, is not put in another place; a press that would take a
//!   coordinate past the limit is refused whole;
//! - a press is at most one undo step, one undo gives the file back exactly, and an event is raised when there was a step;
//! - the same press repeated `n` times moves the nodes that are not in a flow exactly as `MoveBy` of `n` steps does, whatever
//!   the direction and the size of the step (the nodes are picked so that none is under another one, which `MoveBy` would
//!   move twice; a free node under a flow child is among them).
//!
//! Seeds per run: 20 by default; set `MELDFRAME_SEEDS` for more (the 600-seed runs stay native). `MELDFRAME_SEED=n` replays one
//! seed. A failure prints the seed and the step.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::{Action, App, Direction, Event};
use meldframe_protocol::Node;
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_SEEDS: u64 = 20;
const STEPS: usize = 30;
const DIRECTIONS: [Direction; 4] = [
    Direction::Up,
    Direction::Down,
    Direction::Left,
    Direction::Right,
];
/// The auto layout frames of the two files: what a flow child is a child of.
const FLOW_FRAMES: [&str; 3] = ["f", "r", "col"];
const LIMIT: f64 = 1.0e6;

/// A file of the tests.
type Doc = fn() -> Vec<Node>;

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn seeds() -> std::ops::Range<u64> {
    match env("MELDFRAME_SEED") {
        Some(s) => s..s + 1,
        None => 0..env("MELDFRAME_SEEDS").unwrap_or(DEFAULT_SEEDS),
    }
}

/// A small deterministic generator, so a failing seed can be replayed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self) -> bool {
        self.next() & 1 == 1
    }

    fn some<'a>(&mut self, from: &[&'a str]) -> Vec<&'a str> {
        let mut out: Vec<&str> = from.iter().copied().filter(|_| self.chance()).collect();
        if out.is_empty() {
            out.push(from[self.below(from.len())]);
        }
        out
    }
}

fn select(a: &mut App, ids: &[&str]) {
    a.dispatch(Action::Select {
        ids: ids.iter().map(|s| s.to_string()).collect(),
    });
    a.take_events();
}

type Xy = BTreeMap<String, (f64, f64)>;

fn xys(a: &App) -> Xy {
    a.nodes()
        .iter()
        .map(|n| (n.id.clone(), (n.x, n.y)))
        .collect()
}

/// The children of each auto layout frame in sibling order.
fn flows(a: &App) -> Vec<Vec<String>> {
    FLOW_FRAMES.iter().map(|f| kids(a, Some(f))).collect()
}

/// Whether an ancestor of `id` (or `id` itself with `own`) is in `set`, following the parents of `nodes`.
fn under(nodes: &[Node], id: &str, set: &BTreeSet<String>, own: bool) -> bool {
    let parent = |id: &str| {
        nodes
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.parent_id.clone())
    };
    let mut cur = if own {
        Some(id.to_string())
    } else {
        parent(id)
    };
    while let Some(c) = cur {
        if set.contains(&c) {
            return true;
        }
        cur = parent(&c);
    }
    false
}

/// The selected nodes the layout does not place: the ones a press moves by coordinates.
fn moving(nodes: &[Node], ids: &[&str], on: bool) -> BTreeSet<String> {
    let in_flow = |n: &Node| {
        on && !n.absolute
            && n.parent_id
                .as_deref()
                .is_some_and(|p| FLOW_FRAMES.contains(&p))
    };
    nodes
        .iter()
        .filter(|n| ids.contains(&n.id.as_str()) && !in_flow(n))
        .map(|n| n.id.clone())
        .collect()
}

/// Where each node is to be after a press, by the model of the module doc; none when the press is refused.
fn predicted(a: &App, ids: &[&str], on: bool, d: Direction, coarse: bool) -> Option<Xy> {
    let moving = moving(a.nodes(), ids, on);
    let (dx, dy) = delta(d, coarse);
    let mut out = xys(a);
    for n in a.nodes() {
        if moving.contains(&n.id) && !under(a.nodes(), &n.id, &moving, false) {
            let (x, y) = (n.x + dx, n.y + dy);
            if x.abs() > LIMIT || y.abs() > LIMIT {
                return None;
            }
            out.insert(n.id.clone(), (x, y));
        }
    }
    Some(out)
}

fn delta(d: Direction, coarse: bool) -> (f64, f64) {
    let step = if coarse { 10.0 } else { 1.0 };
    match d {
        Direction::Up => (0.0, -step),
        Direction::Down => (0.0, step),
        Direction::Left => (-step, 0.0),
        Direction::Right => (step, 0.0),
    }
}

#[test]
fn a_press_lands_every_node_where_the_model_says_in_one_step_that_one_undo_takes_back() {
    let docs: [(&str, Doc); 2] = [("fixture", nodes), ("nested", nested)];
    for seed in seeds() {
        for (doc, list) in docs {
            for on in [true, false] {
                let mut rng = Rng(seed);
                let mut a = app_of(list(), on);
                let all: Vec<String> = a.nodes().iter().map(|n| n.id.clone()).collect();
                let all: Vec<&str> = all.iter().map(String::as_str).collect();
                for step in 0..STEPS {
                    if rng.below(6) == 0 {
                        // Some node is put at the edge of what a coordinate may be.
                        let id = all[rng.below(all.len())].to_string();
                        let (x, y) = (LIMIT - rng.below(12) as f64, LIMIT - rng.below(12) as f64);
                        a.dispatch(Action::Move { id, x, y });
                    }
                    let ids = rng.some(&all);
                    select(&mut a, &ids);
                    let (before, steps_before, flows_before) =
                        (snapshot(a.nodes()), steps(&a), flows(&a));
                    let (d, coarse) = (DIRECTIONS[rng.below(4)], rng.chance());
                    let want = predicted(&a, &ids, on, d, coarse);
                    a.dispatch(Action::Nudge {
                        direction: d,
                        coarse,
                    });
                    let events = a.take_events();
                    let at = format!(
                        "seed {seed} {doc} layout {on} step {step} {ids:?} {d:?} coarse {coarse}"
                    );
                    let made = steps(&a) - steps_before;
                    assert!(made <= 1, "{at}: {made} steps");
                    let changed = events
                        .iter()
                        .any(|e| matches!(e, Event::DocumentChanged { .. }));
                    assert_eq!(changed, made == 1, "{at}: event and step disagree");
                    match want {
                        Some(want) => {
                            assert_eq!(xys(&a), want, "{at}: where the nodes land");
                            // A flow whose frame is under a moving node is carried along, never reordered.
                            let moving = moving(a.nodes(), &ids, on);
                            for (i, f) in FLOW_FRAMES.iter().enumerate() {
                                if under(a.nodes(), f, &moving, true) {
                                    assert_eq!(
                                        flows(&a)[i],
                                        flows_before[i],
                                        "{at}: the flow of {f}"
                                    );
                                }
                            }
                        }
                        None => assert_eq!(snapshot(a.nodes()), before, "{at}: refused whole"),
                    }
                    if made == 1 {
                        a.dispatch(Action::Undo {});
                        assert_eq!(snapshot(a.nodes()), before, "{at}: undo");
                        a.dispatch(Action::Redo {});
                    }
                    a.take_events();
                }
            }
        }
    }
}

#[test]
fn n_nudges_are_one_move_by_of_n_steps() {
    // The nodes of each pool have none under another. With the layout on a flow child is left out, and the free rectangle `kk`
    // under the flow child `k` is in.
    type Pool<'a> = (Doc, &'a [&'a str], &'a [&'a str]);
    let pools: [Pool; 2] = [
        (nodes, &["abs", "g", "r"], &["a", "b", "abs", "g", "r"]),
        (nested, &["p", "kk"], &["p", "k2", "kk"]),
    ];
    for seed in seeds() {
        for (list, pool_on, pool_off) in pools {
            for on in [true, false] {
                let mut rng = Rng(seed ^ 0xA5A5);
                let ids = rng.some(if on { pool_on } else { pool_off });
                let (d, coarse) = (DIRECTIONS[rng.below(4)], rng.chance());
                let n = 1 + rng.below(7);
                let (dx, dy) = delta(d, coarse);
                let (mut pressed, mut moved) = (app_of(list(), on), app_of(list(), on));
                select(&mut pressed, &ids);
                select(&mut moved, &ids);
                for _ in 0..n {
                    pressed.dispatch(Action::Nudge {
                        direction: d,
                        coarse,
                    });
                }
                moved.dispatch(Action::MoveBy {
                    dx: dx * n as f64,
                    dy: dy * n as f64,
                });
                let at = format!("seed {seed} layout {on} {ids:?} {d:?} coarse {coarse} n {n}");
                assert_eq!(snapshot(pressed.nodes()), snapshot(moved.nodes()), "{at}");
                assert_eq!(steps(&pressed), n, "{at}: one step for each press");
                assert_eq!(steps(&moved), 1, "{at}");
            }
        }
    }
}
