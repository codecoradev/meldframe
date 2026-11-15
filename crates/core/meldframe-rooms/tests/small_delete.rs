//! ADR-0010: a small delete is Auto, a delete of a frame or of a large subtree is Ask. The gate gets the
//! facts from the room's tree as it is at submit time (`TreeFacts`); a node it cannot see is a Delete.

use meldframe_host::{Gate, MemoryStore, Op, Principal, Refusal, Room, RoomError};
use meldframe_module::{Policy, RiskTier};
use meldframe_protocol::{Command, Node};
use meldframe_rooms::{SMALL_DELETE_MAX_NODES, TreeFacts};

fn who() -> Principal {
    Principal::new("ws_1", "agent:a")
}

fn add(key: &str, id: &str, kind: &str, parent: Option<&str>) -> Op {
    let node =
        serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent}))
            .unwrap();
    Op::new(key, Command::AddNode { node, index: None })
}

fn rm(key: &str, id: &str) -> Op {
    Op::new(key, Command::RemoveNode { id: id.into() })
}

fn node(id: &str, kind: &str, parent: Option<&str>) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": kind, "parentId": parent})).unwrap()
}

/// A room holding frame `f` with one rect `r` in it, built through the gate and the room.
fn room() -> Room<MemoryStore> {
    let mut room = Room::open("ws_1", "file_1", MemoryStore::new()).unwrap();
    let ops = vec![
        add("k-f", "f", "frame", None),
        add("k-r", "r", "rect", Some("f")),
    ];
    for adm in Gate::new().submit(&who(), ops).unwrap().admitted {
        room.accept(adm).unwrap();
    }
    room
}

/// A tree where rect `big` has `children` rects below it. The room does not let a rect have children
/// (`ParentNotFrame`), so this is built by hand: the size rule must hold for any tree it is given.
fn tree(children: usize) -> Vec<Node> {
    let mut nodes = vec![node("f", "frame", None), node("big", "rect", Some("f"))];
    nodes.extend((0..children).map(|i| node(&format!("c{i}"), "rect", Some("big"))));
    nodes
}

fn submit(nodes: &[Node], ops: Vec<Op>) -> Result<usize, Refusal> {
    Gate::new()
        .submit_with(&who(), ops, &TreeFacts::new(nodes))
        .map(|r| r.admitted.len())
}

fn forbidden() -> Refusal {
    Refusal::Forbidden {
        tier: RiskTier::Delete,
        policy: Policy::Ask,
    }
}

#[test]
fn a_leaf_delete_is_auto() {
    assert_eq!(submit(room().nodes(), vec![rm("k1", "r")]), Ok(1));
}

#[test]
fn a_delete_at_the_limit_is_auto_and_one_over_is_ask() {
    let at = tree(SMALL_DELETE_MAX_NODES - 1);
    assert_eq!(submit(&at, vec![rm("k1", "big")]), Ok(1));
    let over = tree(SMALL_DELETE_MAX_NODES);
    assert_eq!(submit(&over, vec![rm("k1", "big")]), Err(forbidden()));
}

#[test]
fn a_frame_delete_is_ask_even_when_it_is_small() {
    let room = room();
    assert_eq!(room.nodes().len(), 2);
    assert_eq!(submit(room.nodes(), vec![rm("k1", "f")]), Err(forbidden()));
}

#[test]
fn a_node_that_is_not_in_the_file_is_ask() {
    assert_eq!(
        submit(room().nodes(), vec![rm("k1", "nope")]),
        Err(forbidden())
    );
}

#[test]
fn a_batch_that_adds_a_node_gets_no_relief_for_that_node() {
    let ops = vec![add("k1", "new", "rect", Some("f")), rm("k2", "new")];
    assert_eq!(submit(room().nodes(), ops), Err(forbidden()));
}

#[test]
fn a_batch_that_can_grow_a_subtree_gets_no_relief() {
    // Reparenting a big subtree under a small node and then removing that node is not a small delete.
    let reparent = Command::Reparent {
        id: "big".into(),
        parent_id: Some("r".into()),
        pos: None,
    };
    let mut nodes = tree(SMALL_DELETE_MAX_NODES);
    nodes.push(node("r", "rect", Some("f")));
    let ops = vec![Op::new("k1", reparent), rm("k2", "r")];
    assert_eq!(submit(&nodes, ops), Err(forbidden()));
}

#[test]
fn without_facts_every_delete_is_ask() {
    let ops = vec![rm("k1", "r")];
    assert_eq!(
        Gate::new().submit(&who(), ops).map(|_| ()),
        Err(forbidden())
    );
}

#[test]
fn the_permit_carries_the_relaxed_tier() {
    let room = room();
    let facts = TreeFacts::new(room.nodes());
    let mut report = Gate::new()
        .submit_with(&who(), vec![rm("k1", "r")], &facts)
        .unwrap();
    assert_eq!(report.admitted.remove(0).permit().tier(), RiskTier::Change);
}

#[test]
fn a_relaxed_delete_is_accepted_by_the_room() {
    let mut room = room();
    let facts = TreeFacts::new(room.nodes());
    let adm = Gate::new()
        .submit_with(&who(), vec![rm("k1", "r")], &facts)
        .unwrap()
        .admitted
        .remove(0);
    room.accept(adm).unwrap();
    assert_eq!(room.nodes().len(), 1);
}

#[test]
fn a_node_the_batch_does_not_add_but_the_file_lacks_is_unknown_even_beside_an_update() {
    let update = Op::new(
        "k1",
        Command::UpdateNode {
            id: "r".into(),
            patch: Default::default(),
        },
    );
    assert_eq!(
        submit(room().nodes(), vec![update, rm("k2", "nope")]),
        Err(forbidden())
    );
}

#[test]
fn a_batch_with_an_update_or_a_reorder_keeps_the_relief() {
    let update = Op::new(
        "k1",
        Command::UpdateNode {
            id: "r".into(),
            patch: Default::default(),
        },
    );
    assert_eq!(submit(room().nodes(), vec![update, rm("k2", "r")]), Ok(2));
}

#[test]
fn an_admission_for_a_removed_leaf_does_not_delete_a_frame_that_took_its_id() {
    let mut room = room();
    let facts = TreeFacts::new(room.nodes());
    let old = Gate::new()
        .submit_with(&who(), vec![rm("k-old", "r")], &facts)
        .unwrap()
        .admitted
        .remove(0);
    // Meanwhile: the leaf goes, and a frame with the same id comes back with a child.
    let gone = Gate::new()
        .submit_with(&who(), vec![rm("k1", "r")], &facts)
        .unwrap()
        .admitted
        .remove(0);
    room.accept(gone).unwrap();
    let ops = vec![
        add("k2", "r", "frame", Some("f")),
        add("k3", "kid", "rect", Some("r")),
    ];
    for adm in Gate::new().submit(&who(), ops).unwrap().admitted {
        room.accept(adm).unwrap();
    }
    let before = room.nodes().len();
    let err = room.accept(old).unwrap_err();
    assert_eq!(err, RoomError::Refused(forbidden()));
    assert_eq!(room.nodes().len(), before);
}
