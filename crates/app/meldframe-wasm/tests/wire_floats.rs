//! The bridge reads and writes a number to the last bit, as the server does (see `apps/server/tests/wire_floats.rs`): the
//! command the app keeps as pending is compared with the one the server gives back, and a number read one bit off would
//! be taken for a divergence. `serde_json` needs its `float_roundtrip` feature for that; this crate asks for it.

use meldframe_wasm::Bridge;
use serde_json::{Value, json};

#[test]
fn an_action_with_a_float_gives_the_node_that_float_and_an_op_with_it() {
    let nodes = json!([{"id": "a", "type": "rect", "w": 10, "h": 10}]);
    let mut b = Bridge::new("usr_me", "web", 7, &nodes.to_string(), 0, None).unwrap();
    b.dispatch(r#"{"type":"connected","nowMs":0}"#).unwrap();
    let fx = b
        .dispatch(r#"{"type":"move","id":"a","x":450.0518134715026,"y":223.73056994818654}"#)
        .unwrap();
    let nodes: Vec<Value> = serde_json::from_str(&b.nodes_json()).unwrap();
    assert_eq!(nodes[0]["y"].as_f64(), Some(223.73056994818654));
    assert!(fx.contains("223.73056994818654"), "{fx}");
}
