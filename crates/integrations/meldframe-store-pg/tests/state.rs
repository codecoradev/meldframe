//! How a snapshot is kept in the `state` column (E3.3): a file without variables stays the plain array of nodes
//! it always was, a file with variables is an object, and both read back.

use meldframe_host::Snapshot;
use meldframe_protocol::Variables;
use meldframe_store_pg::{snapshot_from_state, state_json};
use serde_json::{Value, json};

fn variables() -> Variables {
    serde_json::from_value(json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "m", "name": "m"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color", "values": {"m": "#fff"}}]
    }))
    .unwrap()
}

fn snapshot(variables: Variables) -> Snapshot {
    let node = serde_json::from_value(json!({"id": "n", "type": "rect", "pos": "a"})).unwrap();
    Snapshot {
        seq: 3,
        nodes: vec![node],
        variables,
    }
}

#[test]
fn a_file_without_variables_is_stored_as_the_array_of_nodes_it_always_was() {
    let v = state_json(&snapshot(Variables::default()));
    assert!(matches!(&v, Value::Array(a) if a.len() == 1), "{v}");
    let back = snapshot_from_state(3, v).unwrap();
    assert_eq!(
        (back.seq, back.nodes.len(), back.variables.is_empty()),
        (3, 1, true)
    );
}

#[test]
fn a_file_with_variables_is_stored_as_an_object_and_reads_back_whole() {
    let v = state_json(&snapshot(variables()));
    assert!(
        v["nodes"].is_array() && v["variables"]["variables"][0]["id"] == "bg",
        "{v}"
    );
    let back = snapshot_from_state(3, v).unwrap();
    assert_eq!(back.variables, variables());
    assert_eq!(back.nodes.len(), 1);
}

#[test]
fn a_state_that_is_neither_is_an_error() {
    assert!(snapshot_from_state(1, json!("nope")).is_err());
    assert!(snapshot_from_state(1, json!({"nodes": 5})).is_err());
}
