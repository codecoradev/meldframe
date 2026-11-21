//! The preview of a drag of several nodes and the drop agree (`drop_preview_many` and `Action::DropNodes`). Over the two
//! fixture files, a number of selections (flow children of one frame, of two, with free nodes, with a node and its
//! descendant) dragged to every point of a coarse grid: when the preview names a slot, the dragged flow children that are
//! not drawn following the pointer land in that frame at that index, together and in order; the nodes that follow the
//! pointer end up outside every flow, and the nodes that are neither (flow children that stay) are left as they were.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::App;
use meldframe_protocol::{LayoutMode, Node, NodeKind};
use std::collections::BTreeSet;

fn in_flow(a: &App, id: &str) -> bool {
    let n = node(a, id);
    n.parent_id
        .as_deref()
        .map(|p| node(a, p))
        .is_some_and(|p| !n.absolute && p.kind == NodeKind::Frame && p.layout != LayoutMode::None)
}

/// The children of `frame` that the layout places, in order.
fn flow(a: &App, frame: &str) -> Vec<String> {
    let mut k = kids(a, Some(frame));
    k.retain(|id| !node(a, id).absolute);
    k
}

/// The ids that have no selected ancestor.
fn tops(a: &App, ids: &[&str]) -> Vec<String> {
    let under = |id: &str| {
        let mut cur = node(a, id).parent_id;
        while let Some(p) = cur {
            if ids.contains(&p.as_str()) {
                return true;
            }
            cur = node(a, &p).parent_id;
        }
        false
    };
    ids.iter()
        .filter(|i| !under(i))
        .map(|i| i.to_string())
        .collect()
}

struct Seen {
    slots: BTreeSet<String>,
    nones: usize,
    stayed: usize,
}

fn sweep(list: Vec<Node>, picks: &[&[&str]], xs: (f64, f64), ys: (f64, f64), step: f64) -> Seen {
    let mut seen = Seen {
        slots: BTreeSet::new(),
        nones: 0,
        stayed: 0,
    };
    for ids in picks {
        let mut y = ys.0;
        while y <= ys.1 {
            let mut x = xs.0;
            while x <= xs.1 {
                // A fresh app for each point: the ops of the drops would pile up unconfirmed in one that goes on.
                let mut a = app_of(list.clone(), true);
                let (at, by) = ((x, y), (3.0, 4.0));
                let owned: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
                let preview = a
                    .drop_preview_many(&owned, at, by)
                    .expect("a preview with the layout on");
                let before = a.nodes().to_vec();
                drop_nodes(&mut a, ids, at, by);
                let top = tops(&a, ids);
                let ctx = format!("{ids:?} at {at:?}");
                for id in &top {
                    let was = before.iter().find(|n| n.id == *id).unwrap();
                    let now = node(&a, id);
                    if preview.follow.contains(id) {
                        assert!(
                            !in_flow(&a, id),
                            "{ctx}: {id} follows the pointer, so it is out of every flow"
                        );
                    } else if let Some(s) = &preview.slot {
                        assert_eq!(
                            now.parent_id.as_deref(),
                            Some(s.frame.as_str()),
                            "{ctx}: {id} lands in the frame of the slot"
                        );
                        assert_eq!(
                            (now.x, now.y),
                            (was.x, was.y),
                            "{ctx}: {id} has no stored position written"
                        );
                    } else {
                        seen.stayed += 1;
                        assert_eq!(
                            (now.parent_id.clone(), now.x, now.y, now.pos.clone()),
                            (was.parent_id.clone(), was.x, was.y, was.pos.clone()),
                            "{ctx}: {id} stays"
                        );
                    }
                }
                if let Some(s) = &preview.slot {
                    // The block of the flow children: together, in order, at the index of the slot.
                    let block: Vec<String> = flow(&a, &s.frame)
                        .into_iter()
                        .filter(|k| top.contains(k) && !preview.follow.contains(k))
                        .collect();
                    assert!(!block.is_empty(), "{ctx}: a slot with nobody to put in it");
                    let at_index: Vec<String> = flow(&a, &s.frame)
                        .into_iter()
                        .skip(s.index)
                        .take(block.len())
                        .collect();
                    assert_eq!(
                        at_index, block,
                        "{ctx}: the block is at index {} of {}",
                        s.index, s.frame
                    );
                    seen.slots.insert(s.frame.clone());
                } else {
                    seen.nones += 1;
                }
                x += step;
            }
            y += step;
        }
    }
    seen
}

#[test]
fn the_preview_and_the_drop_agree_over_a_column_a_free_frame_and_a_row() {
    let picks: [&[&str]; 9] = [
        &["a", "b"],
        &["a", "c"],
        &["b", "c"],
        &["a", "b", "c"],
        &["a", "r1"],
        &["b", "r1", "fg"],
        &["a", "fg"],
        &["fg", "abs"],
        &["f", "a"],
    ];
    let seen = sweep(nodes(), &picks, (-20.0, 1050.0), (-20.0, 340.0), 60.0);
    assert!(
        seen.slots.contains("f") && seen.slots.contains("r"),
        "{:?}",
        seen.slots
    );
    assert!(
        seen.nones > 0 && seen.stayed > 0,
        "the grid has drops with no slot and flow children that stay"
    );
}

#[test]
fn the_preview_and_the_drop_agree_over_a_row_of_frames_an_empty_frame_and_a_nest() {
    let picks: [&[&str]; 5] = [
        &["c1", "c3"],
        &["c1", "c2", "c3"],
        &["c1", "k"],
        &["c2", "k", "y"],
        &["c3", "other"],
    ];
    let seen = sweep(cards(), &picks, (-20.0, 760.0), (-20.0, 720.0), 60.0);
    assert!(
        seen.slots.contains("deck") && seen.slots.contains("empty"),
        "{:?}",
        seen.slots
    );
}

#[test]
fn the_preview_of_one_node_is_the_slot_of_drop_preview_and_the_node_follows_when_there_is_none() {
    let a = app(true);
    for id in ["a", "b", "c", "r1", "fg", "g"] {
        for at in [
            (30.0, 12.0),
            (30.0, 100.0),
            (720.0, 50.0),
            (450.0, 50.0),
            (900.0, 900.0),
        ] {
            let p = a
                .drop_preview_many(&[id.to_string()], at, (3.0, 4.0))
                .unwrap();
            assert_eq!(p.slot, a.drop_preview(id, at, (3.0, 4.0)), "{id} {at:?}");
            assert_eq!(
                p.follow,
                if p.slot.is_some() {
                    Vec::<String>::new()
                } else {
                    vec![id.to_string()]
                },
                "{id} {at:?}"
            );
        }
    }
}

#[test]
fn with_the_flag_off_there_is_no_preview_and_when_no_id_is_a_node_or_a_number_is_bad_none_either() {
    let a = app(false);
    assert!(
        a.drop_preview_many(&["a".into(), "b".into()], (30.0, 100.0), (0.0, 0.0))
            .is_none()
    );
    let a = app(true);
    assert!(
        a.drop_preview_many(&["nope".into(), "nor".into()], (30.0, 100.0), (0.0, 0.0))
            .is_none()
    );
    // An id that is gone is skipped, as the drop skips it: the preview is the one of the others.
    let some = a.drop_preview_many(&["a".into(), "nope".into()], (30.0, 100.0), (0.0, 0.0));
    assert_eq!(
        some,
        a.drop_preview_many(&["a".into()], (30.0, 100.0), (0.0, 0.0))
    );
    assert!(some.is_some());
    assert!(
        a.drop_preview_many(&["a".into(), "b".into()], (f64::NAN, 100.0), (0.0, 0.0))
            .is_none()
    );
}

#[test]
fn a_preview_reads_the_cached_layout_and_changes_nothing() {
    let a = app(true);
    let ids: Vec<String> = ["a", "b"].map(String::from).to_vec();
    let doc = serde_json::to_string(a.nodes()).unwrap();
    a.drop_preview_many(&ids, (30.0, 100.0), (0.0, 0.0));
    for y in [12.0, 60.0, 100.0, 250.0] {
        a.drop_preview_many(&ids, (30.0, y), (0.0, 0.0));
    }
    assert_eq!(a.layout_runs(), 1, "one layout for all of them");
    assert_eq!(serde_json::to_string(a.nodes()).unwrap(), doc);
}
