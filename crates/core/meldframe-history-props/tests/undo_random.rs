//! Property test: for a random run of commands, undoing every step restores each earlier tree exactly
//! (sibling order included), and redoing every step gives the same trees again.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;

use common::{Rng, add, node, snap};
use meldframe_history::FileState;
use meldframe_protocol::{Command, NodeKind, NodePatch};

/// Seeds per run: 600 by default, more with `MELDFRAME_SEEDS` (nightly). Under WASM the environment
/// does not reach the test, and the default is 20.
fn seeds() -> u64 {
    std::env::var("MELDFRAME_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(target_arch = "wasm32") {
            20
        } else {
            600
        })
}

fn random_cmd(r: &mut Rng, s: &FileState, next_id: &mut u32) -> Command {
    let nodes = s.nodes();
    match r.below(10) {
        0..=3 if !nodes.is_empty() => {
            let n = &nodes[r.below(nodes.len())];
            let mut p = NodePatch::default();
            match r.below(4) {
                0 => p.x = Some(r.below(500) as f64),
                1 => p.fill = Some(format!("#{:06x}", r.below(0xffffff)).into()),
                2 => p.text = Some(format!("t{}", r.below(99))),
                _ => {
                    p.w = Some(r.below(300) as f64);
                    p.fill2 = Some("#112233".into());
                }
            }
            Command::UpdateNode {
                id: n.id.clone(),
                patch: p,
            }
        }
        4..=6 => {
            *next_id += 1;
            let frames: Vec<_> = nodes.iter().filter(|n| n.kind == NodeKind::Frame).collect();
            let parent = if !frames.is_empty() && r.below(3) > 0 {
                Some(frames[r.below(frames.len())].id.as_str())
            } else {
                None
            };
            let kind = if r.below(3) == 0 { "frame" } else { "rect" };
            add(&format!("n{next_id}"), kind, parent)
        }
        7 if !nodes.is_empty() => Command::RemoveNode {
            id: nodes[r.below(nodes.len())].id.clone(),
        },
        8 if nodes.len() > 1 => {
            // Anchors are random: a non-sibling anchor is refused and the step is skipped.
            let id = nodes[r.below(nodes.len())].id.clone();
            let t = nodes[r.below(nodes.len())].id.clone();
            let (before, after) = match r.below(3) {
                0 => (Some(t), None),
                1 => (None, Some(t)),
                _ => (None, None),
            };
            Command::Reorder {
                id,
                before,
                after,
                pos: None,
            }
        }
        _ if nodes.len() > 1 => {
            let i = r.below(nodes.len());
            let frames: Vec<_> = nodes[..i]
                .iter()
                .filter(|n| n.kind == NodeKind::Frame)
                .collect();
            let parent = if frames.is_empty() || r.below(4) == 0 {
                None
            } else {
                Some(frames[r.below(frames.len())].id.clone())
            };
            Command::Reparent {
                id: nodes[i].id.clone(),
                parent_id: parent,
                pos: None,
            }
        }
        _ => {
            *next_id += 1;
            Command::AddNode {
                node: node(&format!("z{next_id}"), "rect", None),
                index: None,
            }
        }
    }
}

#[test]
fn undo_and_redo_walk_through_every_earlier_tree() {
    for seed in 0..seeds() {
        let mut r = Rng(seed * 7919 + 1);
        let mut s = FileState::new();
        s.apply(&add("root", "frame", None)).unwrap();
        let mut next = 0u32;
        let mut states = vec![snap(&s)];
        for _ in 0..80 {
            let c = random_cmd(&mut r, &s, &mut next);
            if s.apply(&c).is_ok() {
                states.push(snap(&s));
            }
        }
        assert!(states.len() > 20, "seed {seed}: too few commands applied");
        for k in (0..states.len() - 1).rev() {
            assert!(s.undo().is_some(), "seed {seed}: undo failed at step {k}");
            assert_eq!(snap(&s), states[k], "seed {seed}: tree after undo to {k}");
        }
        // The root was added before the history started being compared, so one step remains: undo it too.
        assert!(s.undo().is_some() && !s.can_undo());
        for (k, state) in states.iter().enumerate() {
            assert!(s.redo().is_some(), "seed {seed}: redo failed at step {k}");
            assert_eq!(&snap(&s), state, "seed {seed}: tree after redo to {k}");
        }
        assert!(!s.can_redo());
    }
}
