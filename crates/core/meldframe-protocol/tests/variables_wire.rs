//! The wire shape of variables (E3.3): a field is `T` or `{"var": id}`, a file written before bindings existed
//! still loads, and the strict parse refuses what it does not know.

use meldframe_protocol::{Bind, Command, Node, NodePatch, Value, Variables};
use serde_json::json;

#[test]
fn a_field_is_a_literal_or_a_binding_and_both_round_trip() {
    let lit: Node =
        serde_json::from_value(json!({"id": "a", "type": "rect", "fill": "#112233", "radius": 4}))
            .unwrap();
    assert_eq!(
        (lit.fill.clone(), lit.radius.clone()),
        ("#112233".into(), 4.0.into())
    );
    let bound: Node = serde_json::from_value(
        json!({"id": "a", "type": "rect", "fill": {"var": "v1"}, "opacity": {"var": "v2"}}),
    )
    .unwrap();
    assert_eq!(bound.fill, Bind::var("v1"));
    let v = serde_json::to_value(&bound).unwrap();
    assert_eq!(
        (&v["fill"], &v["opacity"]),
        (&json!({"var": "v1"}), &json!({"var": "v2"}))
    );
}

#[test]
fn a_node_saved_before_bindings_existed_loads_with_its_literals() {
    let old: Node = serde_json::from_str(
        r##"{"id":"old","type":"frame","fill":"#000000","radius":2,"opacity":0.5}"##,
    )
    .unwrap();
    assert_eq!(
        (old.fill, old.radius, old.opacity),
        ("#000000".into(), 2.0.into(), 0.5.into())
    );
}

#[test]
fn a_binding_takes_no_other_key() {
    assert!(serde_json::from_value::<NodePatch>(json!({"fill": {"var": "v", "x": 1}})).is_err());
    assert!(serde_json::from_value::<NodePatch>(json!({"fill": {}})).is_err());
    assert!(serde_json::from_value::<NodePatch>(json!({"fill": {"var": "v"}})).is_ok());
}

#[test]
fn variable_commands_use_the_camel_case_tags_and_a_value_is_a_literal_or_an_alias() {
    let c: Command = serde_json::from_value(
        json!({"type": "setVariableValue", "id": "v", "mode": "m", "value": {"var": "w"}}),
    )
    .unwrap();
    assert!(matches!(
        c,
        Command::SetVariableValue {
            value: Bind::Var(_),
            ..
        }
    ));
    let c: Command = serde_json::from_value(
        json!({"type": "setVariableValue", "id": "v", "mode": "m", "value": 3}),
    )
    .unwrap();
    assert!(
        matches!(c, Command::SetVariableValue { value: Bind::Lit(Value::Number(n)), .. } if n == 3.0)
    );
    assert!(serde_json::from_value::<Command>(json!({"type": "addMode", "collection": "c", "mode": {"id": "m", "name": "m"}, "copyFrom": "d", "extra": 1})).is_err());
    let back = serde_json::to_value(serde_json::from_value::<Command>(json!({"type": "addMode", "collection": "c", "mode": {"id": "m", "name": "m"}, "index": 1})).unwrap()).unwrap();
    assert_eq!(back["index"], 1);
    assert!(back.get("copyFrom").is_none());
}

#[test]
fn an_empty_set_of_variables_has_no_entries() {
    let v: Variables = serde_json::from_value(json!({})).unwrap();
    assert!(v.is_empty());
}
