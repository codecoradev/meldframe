//! Rebalancing many siblings: the result is the one the plain `Reorder` chain gives, and the cost grows
//! with the number of siblings, not with its square.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{Rng, node, snap};
use meldframe_core::{ApplyError, Limits, apply_command, rebalance_commands};
use meldframe_history::FileState;
use meldframe_protocol::Node;

/// `root` with `n` children and a second parent `other` with two, in scrambled list order and with
/// uneven keys (the children of `root` are crowded by repeated inserts between the same neighbours).
fn tree(n: usize, seed: u64) -> Vec<Node> {
    let mut rng = Rng(seed);
    let mut ns = vec![node("root", "frame", None), node("other", "frame", None)];
    let mut keys = meldframe_order::fracindex::n_between(None, None, n).unwrap();
    for (i, k) in keys.drain(..).enumerate() {
        let mut c = node(&format!("c{i}"), "rect", Some("root"));
        c.pos = if rng.below(3) == 0 {
            format!("{k}{}", "m".repeat(rng.below(4)))
        } else {
            k
        };
        ns.push(c);
    }
    for id in ["o1", "o2"] {
        let mut c = node(id, "rect", Some("other"));
        c.pos = format!("{id}x");
        ns.push(c);
    }
    for i in (1..ns.len()).rev() {
        ns.swap(i, rng.below(i + 1));
    }
    ns
}

/// What the old `rebalance` produced: every command of `rebalance_commands` through the validated path.
fn reference(nodes: &[Node], parent: &str, l: &Limits) -> Vec<Node> {
    let mut ns = nodes.to_vec();
    for c in rebalance_commands(&ns.clone(), Some(parent)) {
        apply_command(&mut ns, &c, l).unwrap();
    }
    ns
}

fn sorted(mut v: Vec<Node>) -> String {
    v.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&v).unwrap()
}

#[test]
fn the_new_keys_are_the_ones_the_command_chain_gives_for_many_sibling_counts() {
    for n in (0..=40).chain([63, 64, 100, 257]) {
        for seed in 0..3 {
            let mut s = FileState::from_nodes_unchecked(tree(n, seed));
            let want = reference(s.nodes(), "root", &Limits::default());
            let before = snap(&s);
            assert_eq!(s.rebalance(Some("root")), Ok(n), "n={n} seed={seed}");
            assert_eq!(
                sorted(s.nodes().to_vec()),
                sorted(want),
                "n={n} seed={seed}"
            );
            let rebalanced = snap(&s);
            assert_eq!(s.undo_depth(), usize::from(n > 0));
            if n > 0 {
                s.undo().unwrap();
                assert_eq!(snap(&s), before, "undo is exact, n={n} seed={seed}");
                s.redo().unwrap();
                assert_eq!(
                    snap(&s),
                    rebalanced,
                    "redo gives the rebalanced tree, n={n} seed={seed}"
                );
                s.undo().unwrap();
                assert_eq!(snap(&s), before);
            }
        }
    }
}

#[test]
fn a_key_that_would_pass_the_limit_refuses_the_whole_rebalance() {
    // Short keys that fit the limit; the rebalance keeps climbing past the last one and runs out of room
    // part way. The old chain had applied the first keys by then; now nothing changes.
    let mut ns = vec![node("root", "frame", None)];
    for (i, k) in meldframe_order::fracindex::n_between(None, None, 50)
        .unwrap()
        .into_iter()
        .enumerate()
    {
        let mut c = node(&format!("c{i}"), "rect", Some("root"));
        c.pos = k;
        ns.push(c);
    }
    let longest = ns.iter().map(|n| n.pos.len()).max().unwrap();
    let limits = Limits {
        max_pos: longest,
        ..Limits::default()
    };
    let mut s = FileState::with_limits(limits);
    s.reset_nodes(ns);
    let before = snap(&s);
    let version = s.version();
    let r = s.rebalance(Some("root"));
    assert!(
        matches!(r, Err(ApplyError::PosTooLong { .. })),
        "{r:?} {longest}"
    );
    assert_eq!(snap(&s), before, "nothing was changed");
    assert_eq!((s.version(), s.undo_depth()), (version, 0));
}

/// Native only: `Instant` is not available under WASM. 20,000 is the node limit; the quadratic version
/// took about 20 s here. Undo is not timed: it still replays one `Reorder` per child.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn twenty_thousand_siblings_rebalance_in_under_five_seconds() {
    let mut s = FileState::from_nodes_unchecked(tree(20_000, 7));
    let t = std::time::Instant::now();
    assert_eq!(s.rebalance(Some("root")), Ok(20_000));
    let took = t.elapsed();
    assert!(took.as_secs_f64() < 5.0, "rebalance took {took:?}");
    assert_eq!(s.undo_depth(), 1);
}

#[test]
fn five_thousand_siblings_undo_to_the_exact_old_keys() {
    let mut s = FileState::from_nodes_unchecked(tree(5_000, 7));
    let before = snap(&s);
    assert_eq!(s.rebalance(Some("root")), Ok(5_000));
    assert_ne!(snap(&s), before);
    s.undo().unwrap();
    assert_eq!(snap(&s), before);
}

#[test]
fn a_rebalance_with_no_children_still_ends_a_keyed_edit_run() {
    let keyed = |s: &mut FileState, v: f64| {
        let c = common::upd("c0", move |p| p.x = Some(v));
        s.apply_keyed(&c, Some("x")).unwrap();
    };
    let mut s = FileState::from_nodes_unchecked(tree(2, 0));
    keyed(&mut s, 1.0);
    keyed(&mut s, 2.0);
    assert_eq!(s.undo_depth(), 1, "same key joins the step");
    assert_eq!(s.rebalance(Some("c0")), Ok(0));
    keyed(&mut s, 3.0);
    assert_eq!(s.undo_depth(), 2, "the rebalance ended the run");
}
