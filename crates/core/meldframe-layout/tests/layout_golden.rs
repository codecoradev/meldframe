//! Golden test: the core's layout equals the TypeScript oracle's on recorded trees.
//!
//! The fixture holds node trees and the rects the oracle computed with the stub measurer; the
//! test needs no TypeScript or Node at run time. The trees are random, so they also reach
//! regimes the fidelity contract leaves unspecified (rule G3); there the core and the oracle
//! implement the same arithmetic and still agree.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_layout::{StubMeasurer, compute_rects};
use meldframe_protocol::Node;
use serde::Deserialize;

const FIXTURE: &str = include_str!("fixtures/layout_oracle.json");

/// Largest allowed difference per coordinate. The fixture is rounded to 1e-9.
const TOLERANCE: f64 = 1e-6;

#[derive(Deserialize)]
struct Tree {
    nodes: Vec<Node>,
    rects: Vec<(String, f64, f64, f64, f64)>,
}

#[derive(Deserialize)]
struct Fixture {
    trees: Vec<Tree>,
}

#[test]
fn core_layout_matches_the_oracle_on_every_recorded_tree() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("a valid fixture");
    assert!(fixture.trees.len() >= 15, "the fixture lost trees");
    let mut compared = 0;
    for (t, tree) in fixture.trees.iter().enumerate() {
        let got = compute_rects(&tree.nodes, &StubMeasurer);
        assert_eq!(got.len(), tree.rects.len(), "tree {t}: rect count differs");
        for ((id, rect), (want_id, x, y, w, h)) in got.iter().zip(&tree.rects) {
            assert_eq!(id, want_id, "tree {t}: drawing order differs");
            for (name, have, want) in [
                ("x", rect.x, x),
                ("y", rect.y, y),
                ("w", rect.w, w),
                ("h", rect.h, h),
            ] {
                assert!(
                    (have - want).abs() <= TOLERANCE,
                    "tree {t}, node {id}: {name} is {have}, the oracle says {want}"
                );
            }
            compared += 1;
        }
    }
    assert!(compared >= 200, "only {compared} rects compared");
}
