//! The bounded record of confirmed ops: when it folds, what it keeps, and what it says about a batch cut by a fold.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_history::FileState;
use meldframe_protocol::{Command, NodePatch};
use meldframe_revert::{ConfirmedLog, FOLD_CHUNK, LoggedOp, MAX_LOG, state_at};

fn base() -> FileState {
    let mut s = FileState::new();
    let n = |id: &str, k: &str, p: Option<&str>| {
        serde_json::from_value(serde_json::json!({"id": id, "type": k, "parentId": p})).unwrap()
    };
    s.apply(&Command::AddNode {
        node: n("root", "frame", None),
        index: None,
    })
    .unwrap();
    s.apply(&Command::AddNode {
        node: n("a", "rect", Some("root")),
        index: None,
    })
    .unwrap();
    s
}

fn op(seq: u64, key: &str, x: f64) -> LoggedOp {
    LoggedOp {
        seq,
        actor: "agent:x".into(),
        key: key.into(),
        command: Command::UpdateNode {
            id: "a".into(),
            patch: NodePatch {
                x: Some(x),
                ..Default::default()
            },
        },
    }
}

fn x_of(nodes: &[meldframe_protocol::Node]) -> f64 {
    nodes.iter().find(|n| n.id == "a").unwrap().x
}

#[test]
fn nothing_is_folded_until_the_record_is_a_chunk_over_its_size() {
    let mut r = ConfirmedLog::new(base().nodes().to_vec(), 0);
    for i in 1..(MAX_LOG + FOLD_CHUNK) as u64 {
        r.push(op(i, &format!("k{i}"), i as f64));
    }
    assert_eq!(r.ops().len(), MAX_LOG + FOLD_CHUNK - 1);
    assert_eq!(r.base_seq(), 0, "one op short of the boundary: no fold yet");
    let at = (MAX_LOG + FOLD_CHUNK) as u64;
    r.push(op(at, "last", at as f64));
    assert_eq!(
        r.ops().len(),
        MAX_LOG,
        "folded down to the size, in one pass"
    );
    assert_eq!(r.base_seq(), FOLD_CHUNK as u64);
    assert_eq!(r.ops()[0].seq, FOLD_CHUNK as u64 + 1);
    assert_eq!(
        x_of(r.base()),
        FOLD_CHUNK as f64,
        "the base is the file as of base_seq"
    );
    assert!(!r.diverged());
    // The record still replays from the new base.
    let end = state_at(r.base(), r.ops(), u64::MAX).unwrap();
    assert_eq!(x_of(&end), at as f64);
    // And it grows again for a chunk before the next fold.
    r.push(op(at + 1, "next", 1.0));
    assert_eq!(r.ops().len(), MAX_LOG + 1);
}

#[test]
fn a_batch_cut_by_a_fold_is_partial_and_one_after_it_is_not() {
    let mut r = ConfirmedLog::new(base().nodes().to_vec(), 0);
    // `old:0` goes into the fold; `old:1` is the first op kept at the boundary.
    r.push(op(1, "old:0", 1.0));
    for i in 2..(MAX_LOG + FOLD_CHUNK) as u64 {
        r.push(op(i, &format!("fill{i}"), i as f64));
    }
    r.push(op((MAX_LOG + FOLD_CHUNK) as u64, "new:0", 5.0));
    assert!(r.batch_is_partial("agent:x", "old"));
    assert!(!r.batch_is_partial("agent:x", "new"));
    assert!(!r.batch_is_partial("agent:y", "old"));
}

#[test]
fn a_base_the_ops_do_not_replay_on_is_reported() {
    let mut r = ConfirmedLog::new(base().nodes().to_vec(), 0);
    let gone = |seq| LoggedOp {
        seq,
        actor: "agent:x".into(),
        key: format!("k{seq}"),
        command: Command::RemoveNode { id: "nope".into() },
    };
    for i in 1..=(MAX_LOG + FOLD_CHUNK) as u64 {
        r.push(gone(i));
    }
    assert!(r.diverged());
}

#[test]
fn a_reset_starts_the_record_again() {
    let mut r = ConfirmedLog::new(base().nodes().to_vec(), 0);
    r.push(op(1, "old:0", 1.0));
    r.reset(base().nodes().to_vec(), 9);
    assert!(r.ops().is_empty());
    assert_eq!(r.base_seq(), 9);
    assert!(!r.diverged());
}
