//! The snapshot a client starts from: the nodes as they are, each with the actor and the sequence number of
//! the last op that touched it. A node the server cannot attribute has neither field; there is no time field,
//! because the log keeps none.

use meldframe_protocol::{FileSnapshot, Node, SnapshotNode};

fn node() -> Node {
    serde_json::from_str(r#"{"id":"n1","type":"rect"}"#).unwrap()
}

#[test]
fn a_snapshot_node_adds_the_last_actor_and_sequence_number_to_the_node() {
    let snap = FileSnapshot {
        seq: 7,
        nodes: vec![SnapshotNode {
            node: node(),
            last_actor: Some("agent:a".into()),
            last_seq: Some(7),
        }],
    };
    let v = serde_json::to_value(&snap).unwrap();
    assert_eq!(v["seq"], 7);
    assert_eq!(v["nodes"][0]["id"], "n1");
    assert_eq!(v["nodes"][0]["type"], "rect");
    assert_eq!(v["nodes"][0]["lastActor"], "agent:a");
    assert_eq!(v["nodes"][0]["lastSeq"], 7);
    assert!(v["nodes"][0].get("lastAt").is_none());
}

#[test]
fn a_node_without_attribution_has_neither_field() {
    let plain = SnapshotNode {
        node: node(),
        last_actor: None,
        last_seq: None,
    };
    let v = serde_json::to_value(&plain).unwrap();
    assert_eq!(v["id"], "n1");
    assert!(v.get("lastActor").is_none() && v.get("lastSeq").is_none());
}
