//! Sibling order: effective order is `(pos, id)`, never the position in the list. Covers add,
//! reorder, reparent, colliding positions, refused positions,
//! the deprecated `index` and the `pos` length limit.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{ApplyError, Limits, apply_command};
use meldframe_order::{children_map, fracindex::between};
use meldframe_protocol::{Command, Node};

fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

fn node_pos(id: &str, parent: Option<&str>, pos: &str) -> Node {
    serde_json::from_value(
        serde_json::json!({"id": id, "type": "rect", "parentId": parent, "pos": pos}),
    )
    .unwrap()
}

fn add(n: Node) -> Command {
    Command::AddNode {
        node: n,
        index: None,
    }
}

fn reorder(id: &str, before: Option<&str>, after: Option<&str>, pos: Option<&str>) -> Command {
    Command::Reorder {
        id: id.into(),
        before: before.map(Into::into),
        after: after.map(Into::into),
        pos: pos.map(Into::into),
    }
}

fn order(nodes: &[Node], parent: Option<&str>) -> Vec<String> {
    let kids = children_map(nodes);
    let ids = kids
        .get(&parent)
        .map(|v| v.iter().map(|&i| nodes[i].id.clone()).collect());
    ids.unwrap_or_default()
}

fn pos_of(nodes: &[Node], id: &str) -> String {
    nodes.iter().find(|n| n.id == id).unwrap().pos.clone()
}

fn apply(nodes: &mut Vec<Node>, cmd: &Command) -> Result<(), ApplyError> {
    apply_command(nodes, cmd, &Limits::default())
}

/// A root frame holding rects `a`, `b`, `c` in that order.
fn base_doc() -> Vec<Node> {
    let mut ns = vec![];
    apply(&mut ns, &add(node("root", "frame", None))).unwrap();
    for id in ["a", "b", "c"] {
        apply(&mut ns, &add(node(id, "rect", Some("root")))).unwrap();
    }
    ns
}

#[test]
fn add_without_pos_goes_last_and_order_is_pos_not_list_position() {
    let mut ns = base_doc();
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
    ns.reverse();
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
}

#[test]
fn reorder_by_anchor_by_pos_and_to_the_front() {
    let mut ns = base_doc();
    apply(&mut ns, &reorder("c", Some("a"), None, None)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["c", "a", "b"]);
    apply(&mut ns, &reorder("c", None, Some("b"), None)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
    // No anchor and no pos: to the front, which is the end of the sibling list.
    apply(&mut ns, &reorder("a", None, None, None)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["b", "c", "a"]);
    let pa = pos_of(&ns, "a");
    apply(&mut ns, &reorder("a", None, None, None)).unwrap();
    assert_eq!(pos_of(&ns, "a"), pa, "already in front: nothing changes");
    // A pos that another sibling already holds lands right behind that sibling.
    let pb = pos_of(&ns, "b");
    apply(&mut ns, &reorder("c", None, None, Some(&pb))).unwrap();
    assert_eq!(order(&ns, Some("root")), ["b", "c", "a"]);
    assert_ne!(pos_of(&ns, "c"), pb);
}

#[test]
fn an_incoherent_reorder_is_refused() {
    let mut ns = base_doc();
    let before = serde_json::to_string(&ns).unwrap();
    let bad = [
        reorder("a", Some("b"), Some("c"), None),
        reorder("a", Some("root"), None, None),
        reorder("a", Some("a"), None, None),
    ];
    for cmd in bad {
        assert!(
            matches!(apply(&mut ns, &cmd), Err(ApplyError::BadReorder(_))),
            "{cmd:?}"
        );
    }
    let r = apply(&mut ns, &reorder("a", None, None, Some("!!")));
    assert!(matches!(r, Err(ApplyError::InvalidPos(_))), "{r:?}");
    assert_eq!(serde_json::to_string(&ns).unwrap(), before);
}

#[test]
fn reparent_sets_parent_and_pos_together() {
    let mut ns = base_doc();
    apply(&mut ns, &add(node("f", "frame", Some("root")))).unwrap();
    for id in ["x", "y"] {
        apply(&mut ns, &add(node(id, "rect", Some("f")))).unwrap();
    }
    let p = between(Some(&pos_of(&ns, "x")), Some(&pos_of(&ns, "y"))).unwrap();
    let into = |id: &str, pos: Option<&str>| Command::Reparent {
        id: id.into(),
        parent_id: Some("f".into()),
        pos: pos.map(Into::into),
    };
    apply(&mut ns, &into("a", Some(&p))).unwrap();
    let a = ns.iter().find(|n| n.id == "a").unwrap();
    assert_eq!(
        (a.parent_id.as_deref(), a.pos.as_str()),
        (Some("f"), p.as_str())
    );
    assert_eq!(order(&ns, Some("f")), ["x", "a", "y"]);
    // No pos: after the last child.
    apply(&mut ns, &into("b", None)).unwrap();
    assert_eq!(order(&ns, Some("f")), ["x", "a", "y", "b"]);
    // Same parent and no pos: the order stays.
    let pb = pos_of(&ns, "b");
    apply(&mut ns, &into("b", None)).unwrap();
    assert_eq!(pos_of(&ns, "b"), pb);
}

#[test]
fn concurrent_inserts_into_the_same_gap_stay_unique() {
    // Two clients computed the same pos between `a` and `b`.
    let mut ns = base_doc();
    let p = between(Some(&pos_of(&ns, "a")), Some(&pos_of(&ns, "b"))).unwrap();
    apply(&mut ns, &add(node_pos("u", Some("root"), &p))).unwrap();
    apply(&mut ns, &add(node_pos("v", Some("root"), &p))).unwrap();
    assert_eq!(order(&ns, Some("root")), ["a", "u", "v", "b", "c"]);
    let (pu, pv) = (pos_of(&ns, "u"), pos_of(&ns, "v"));
    assert_eq!(pu, p);
    assert!(pu < pv && pv < pos_of(&ns, "b"));
    // Arriving in the other order gives the other order; the server's order decides.
    let mut ns2 = base_doc();
    apply(&mut ns2, &add(node_pos("v", Some("root"), &p))).unwrap();
    apply(&mut ns2, &add(node_pos("u", Some("root"), &p))).unwrap();
    assert_eq!(order(&ns2, Some("root")), ["a", "v", "u", "b", "c"]);
}

#[test]
fn a_bad_client_pos_is_refused_everywhere_it_can_arrive() {
    let mut ns = base_doc();
    let too_long = "z".repeat(70);
    let bad = [
        "!",
        "a0 ",
        "é",
        "a10",
        "a",
        "A00000000000000000000000000",
        "a0\u{0}",
        &too_long,
    ];
    for pos in bad {
        let r = apply(&mut ns, &add(node_pos("bad", Some("root"), pos)));
        assert!(
            matches!(
                r,
                Err(ApplyError::InvalidPos(_) | ApplyError::PosTooLong { .. })
            ),
            "{pos:?}"
        );
        let to_root = Command::Reparent {
            id: "a".into(),
            parent_id: None,
            pos: Some(pos.into()),
        };
        assert!(apply(&mut ns, &to_root).is_err(), "{pos:?}");
    }
    assert_eq!(order(&ns, Some("root")), ["a", "b", "c"]);
}

#[test]
fn the_deprecated_index_becomes_a_pos_and_pos_wins_over_it() {
    let mut ns = base_doc(); // draw order: root a b c
    let at = |id: &str, index: usize| Command::AddNode {
        node: node(id, "rect", Some("root")),
        index: Some(index),
    };
    apply(&mut ns, &at("m", 2)).unwrap(); // index 2 = between a and b
    assert_eq!(order(&ns, Some("root")), ["a", "m", "b", "c"]);
    apply(&mut ns, &at("z", 999)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["a", "m", "b", "c", "z"]);
    apply(&mut ns, &at("q", 0)).unwrap();
    assert_eq!(order(&ns, Some("root")), ["q", "a", "m", "b", "c", "z"]);
    let pz = pos_of(&ns, "z");
    let with_pos = Command::AddNode {
        node: node_pos("w", Some("root"), &pz),
        index: Some(0),
    };
    apply(&mut ns, &with_pos).unwrap();
    assert_eq!(order(&ns, Some("root")).last().unwrap(), "w");
}

#[test]
fn repeated_inserts_into_one_gap_stop_at_the_pos_length_limit() {
    let l = Limits {
        max_pos: 12,
        ..Limits::default()
    };
    let mut ns = vec![];
    apply_command(&mut ns, &add(node("root", "frame", None)), &l).unwrap();
    apply_command(&mut ns, &add(node("hi", "rect", Some("root"))), &l).unwrap();
    // `lo` goes in front of `hi` through the deprecated index. Each new node then goes right in front of
    // the previous one, between `lo` and the previous node, so the gap keeps narrowing.
    let lo = Command::AddNode {
        node: node("lo", "rect", Some("root")),
        index: Some(1),
    };
    apply_command(&mut ns, &lo, &l).unwrap();
    let mut prev = "hi".to_string();
    let mut n = 0;
    let err = loop {
        let id = format!("x{n}");
        apply_command(&mut ns, &add(node(&id, "rect", Some("root"))), &l).unwrap();
        match apply_command(&mut ns, &reorder(&id, Some(&prev), None, None), &l) {
            Ok(()) => {
                prev = id;
                n += 1;
            }
            Err(e) => break e,
        }
        assert!(n < 500, "the limit was never reached");
    };
    assert_eq!(err, ApplyError::PosTooLong { max: 12 });
    assert!(n > 10, "only {n} inserts fit");
    assert!(ns.iter().all(|x| x.pos.len() <= 12));
}

#[test]
fn random_commands_keep_sibling_positions_valid_and_unique() {
    let l = Limits::default();
    for seed in 1..=300u64 {
        let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        let mut ns = vec![];
        apply_command(&mut ns, &add(node("root", "frame", None)), &l).unwrap();
        let mut serial = 0;
        for _ in 0..40 {
            let ids: Vec<String> = ns.iter().map(|n| n.id.clone()).collect();
            let pick = |i: usize| ids[i % ids.len()].clone();
            let cmd = match next(4) {
                0 | 1 => {
                    serial += 1;
                    add(node(&format!("n{serial}"), "rect", Some(&pick(next(99)))))
                }
                2 => reorder(&pick(next(99)), Some(&pick(next(99))), None, None),
                _ => Command::Reparent {
                    id: pick(next(99)),
                    parent_id: Some(pick(next(99))),
                    pos: None,
                },
            };
            let before = serde_json::to_string(&ns).unwrap();
            if apply_command(&mut ns, &cmd, &l).is_err() {
                assert_eq!(
                    serde_json::to_string(&ns).unwrap(),
                    before,
                    "seed {seed}: {cmd:?}"
                );
            }
            let mut seen = std::collections::HashSet::new();
            for n in &ns {
                assert!(
                    meldframe_core::check_pos(&n.pos, &l).is_ok(),
                    "seed {seed}: {n:?}"
                );
                assert!(
                    seen.insert((n.parent_id.clone(), n.pos.clone())),
                    "seed {seed}: duplicate pos"
                );
            }
        }
    }
}
