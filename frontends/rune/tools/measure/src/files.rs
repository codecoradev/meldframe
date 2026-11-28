//! The inputs the criteria share (section 3 of the spike note): the performance files of `tools/hit-perf.mjs` written again in Rust
//! with the same shape and the same seeded points, and the path of the Rune workspace.

use meldframe_protocol::Node;
use std::path::{Path, PathBuf};

/// `frontends/rune/`, the root of the workspace this crate is in.
pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The generator of `tools/hit-perf.mjs`: a linear congruential sequence of numbers in 0 to 1, from the same seed.
pub fn lcg(seed: u32) -> impl FnMut() -> f64 {
    let mut s = seed;
    move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(s) / 4_294_967_296.0
    }
}

/// The file of `tools/hit-perf.mjs` for about `n` nodes: one page frame that does not clip and `(n - 1) / 101` clipping frames
/// of 100 rectangles each on a grid (a file holds at most 20,000 nodes, so 20,000 is built as 19,999). Returns the nodes and the
/// number of clipping frames.
pub fn hit_file(n: usize) -> (Vec<Node>, usize) {
    let groups = ((n.max(1) - 1) / 101).max(1);
    let mut nodes = vec![serde_json::json!({
        "id": "page", "type": "frame", "pos": "a0", "x": 0, "y": 0, "w": 100000, "h": 100000, "clip": false,
    })];
    for g in 0..groups {
        let (gx, gy) = ((g % 20) * 1100, (g / 20) * 1100);
        nodes.push(serde_json::json!({
            "id": format!("g{g}"), "type": "frame", "parentId": "page", "pos": format!("a{g:06}"),
            "x": gx, "y": gy, "w": 1000, "h": 1000, "clip": true,
        }));
        for i in 0..100 {
            nodes.push(serde_json::json!({
                "id": format!("r{g}_{i}"), "type": "rect", "parentId": format!("g{g}"), "pos": format!("a{i:03}"),
                "x": (i % 10) * 100 + 10, "y": (i / 10) * 100 + 10, "w": 80, "h": 80,
            }));
        }
    }
    let nodes = nodes
        .into_iter()
        .map(|v| serde_json::from_value(v).expect("a node of the performance file"))
        .collect();
    (nodes, groups)
}

/// The point of the `k`th call over a box of `hit_file`: the same sequence as `over_a_box` of `hit-perf.mjs`.
pub fn over_a_box(rand: &mut dyn FnMut() -> f64, groups: usize) -> (f64, f64) {
    let g = (rand() * groups as f64).floor() as usize;
    let i = (rand() * 100.0).floor() as usize;
    (
        ((g % 20) * 1100 + (i % 10) * 100 + 40) as f64,
        ((g / 20) * 1100 + (i / 10) * 100 + 40) as f64,
    )
}

/// A point over nothing, as `over_nothing` of `hit-perf.mjs`.
pub fn over_nothing(rand: &mut dyn FnMut() -> f64) -> (f64, f64) {
    let x = -100.0 - rand() * 100.0;
    let y = -100.0 - rand() * 100.0;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_gives_the_numbers_of_the_javascript_one() {
        // seed 7: (7 * 1664525 + 1013904223) mod 2^32 = 1025555898, then the next one.
        let mut rand = lcg(7);
        assert_eq!(rand(), 1_025_555_898.0 / 4_294_967_296.0);
        assert!((0.0..1.0).contains(&rand()));
    }

    #[test]
    fn the_files_have_the_counts_the_note_names() {
        assert_eq!(hit_file(1000).0.len(), 910);
        assert_eq!(hit_file(5000).0.len(), 4950);
        assert_eq!(hit_file(19_999).0.len(), 19_999);
        assert_eq!(hit_file(20_000).0.len(), 19_999);
    }

    #[test]
    fn a_point_over_a_box_is_over_a_rectangle_of_the_file() {
        let (nodes, groups) = hit_file(1000);
        let app = meldframe_app::App::new("user:t", "t", 1, nodes, 0).expect("loads");
        let mut rand = lcg(7);
        for _ in 0..50 {
            let (x, y) = over_a_box(&mut rand, groups);
            assert!(
                app.hit(x, y).is_some_and(|id| id.starts_with('r')),
                "{x},{y}"
            );
        }
        let (x, y) = over_nothing(&mut rand);
        assert!(app.hit(x, y).is_none());
    }
}
