//! Keeping sibling positions healthy: migrating files without positions, and rebalancing a parent
//! whose gaps grew crowded.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{ApplyError, Limits, apply_command, normalize_positions, rebalance_commands};
use meldframe_order::{children_map, fracindex::is_valid};
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

fn order(nodes: &[Node], parent: Option<&str>) -> Vec<String> {
    let kids = children_map(nodes);
    let ids = kids
        .get(&parent)
        .map(|v| v.iter().map(|&i| nodes[i].id.clone()).collect());
    ids.unwrap_or_default()
}

#[test]
fn an_old_file_without_positions_is_migrated_by_list_order() {
    // Children come before their parent in the list, and list order is the draw order.
    let mut ns = vec![
        node("c2", "rect", Some("r")),
        node("r", "frame", None),
        node("c1", "rect", Some("r")),
        node("c3", "rect", Some("r")),
    ];
    assert_eq!(
        order(&ns, Some("r")),
        ["c2", "c1", "c3"],
        "before migration the list decides"
    );
    assert_eq!(normalize_positions(&mut ns), 4);
    assert!(ns.iter().all(|n| is_valid(&n.pos)));
    assert_eq!(order(&ns, Some("r")), ["c2", "c1", "c3"]);
    // Idempotent: valid positions are left alone.
    assert_eq!(normalize_positions(&mut ns), 0);
}

#[test]
fn duplicate_positions_are_repaired_keeping_pos_then_id_order() {
    let mut dup = vec![
        node("r", "frame", None),
        node_pos("b", Some("r"), "a1"),
        node_pos("a", Some("r"), "a1"),
    ];
    assert_eq!(normalize_positions(&mut dup), 3); // `r` has none, and the twins collide
    assert_eq!(order(&dup, Some("r")), ["a", "b"]);
}

#[test]
fn json_without_a_pos_field_still_reads_as_empty() {
    let old: Vec<Node> =
        serde_json::from_str(r#"[{"id":"a","type":"rect"},{"id":"b","type":"rect"}]"#).unwrap();
    assert!(old.iter().all(|n| n.pos.is_empty()));
    let mut ns = old;
    normalize_positions(&mut ns);
    assert_eq!(order(&ns, None), ["a", "b"]);
}

#[test]
fn migration_gives_the_same_positions_every_time() {
    let make = || {
        vec![
            node("r", "frame", None),
            node("a", "rect", Some("r")),
            node("b", "rect", Some("r")),
        ]
    };
    let (mut one, mut two) = (make(), make());
    normalize_positions(&mut one);
    normalize_positions(&mut two);
    assert_eq!(
        serde_json::to_string(&one).unwrap(),
        serde_json::to_string(&two).unwrap()
    );
}

#[test]
fn rebalance_restores_room_and_keeps_order() {
    let l = Limits::default();
    let mut ns = vec![];
    apply_command(&mut ns, &add(node("root", "frame", None)), &l).unwrap();
    apply_command(&mut ns, &add(node("hi", "rect", Some("root"))), &l).unwrap();
    let lo = Command::AddNode {
        node: node("lo", "rect", Some("root")),
        index: Some(1),
    };
    apply_command(&mut ns, &lo, &l).unwrap();
    // Crowd the gap between `lo` and `hi` until a key would pass the length limit.
    let mut prev = "hi".to_string();
    let mut n = 0;
    let err = loop {
        let id = format!("x{n}");
        apply_command(&mut ns, &add(node(&id, "rect", Some("root"))), &l).unwrap();
        let before = Command::Reorder {
            id: id.clone(),
            before: Some(prev.clone()),
            after: None,
            pos: None,
        };
        match apply_command(&mut ns, &before, &l) {
            Ok(()) => {
                prev = id;
                n += 1;
            }
            Err(e) => {
                apply_command(&mut ns, &Command::RemoveNode { id }, &l).unwrap();
                break e;
            }
        }
    };
    assert!(matches!(err, ApplyError::PosTooLong { .. }));
    let order_before = order(&ns, Some("root"));
    let longest = ns.iter().map(|x| x.pos.len()).max().unwrap();
    assert!(longest > 55, "{longest}");

    let cmds = rebalance_commands(&ns, Some("root"));
    assert_eq!(cmds.len(), order_before.len());
    for c in &cmds {
        apply_command(&mut ns, c, &l).unwrap();
    }
    assert_eq!(
        order(&ns, Some("root")),
        order_before,
        "relative order is unchanged"
    );
    let children = ns.iter().filter(|x| x.parent_id.is_some());
    assert!(children.map(|x| x.pos.len()).max().unwrap() <= 3);
    // The crowded gap can be used again.
    let again = Command::Reorder {
        id: "lo".into(),
        before: Some(prev),
        after: None,
        pos: None,
    };
    apply_command(&mut ns, &again, &l).unwrap();
}

#[test]
fn rebalance_of_a_parent_without_children_is_empty() {
    let ns = vec![node("a", "frame", None)];
    assert!(rebalance_commands(&ns, Some("a")).is_empty());
    assert!(rebalance_commands(&ns, Some("missing")).is_empty());
}
