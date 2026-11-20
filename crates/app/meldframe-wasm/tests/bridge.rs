//! The JSON boundary: what the browser sends and reads, exercised natively on the plain-Rust `Bridge`.

use meldframe_wasm::Bridge;
use serde_json::{Value, json};

fn bridge() -> Bridge {
    Bridge::new("usr_me", "web", 7, "[]", 0, None).expect("empty file loads")
}

fn run(b: &mut Bridge, action: Value) -> Value {
    serde_json::from_str(&b.dispatch(&action.to_string()).expect("dispatch")).expect("json")
}

#[test]
fn start_asks_for_a_subscription_from_the_last_confirmed_number() {
    let mut b = bridge();
    let fx = run(&mut b, json!({"type": "start", "nowMs": 1}));
    assert_eq!(fx, json!([{"type": "subscribe", "afterSeq": 0}]));
}

#[test]
fn create_node_changes_nodes_state_and_events() {
    let mut b = bridge();
    run(&mut b, json!({"type": "start", "nowMs": 1}));
    run(&mut b, json!({"type": "connected", "nowMs": 2}));
    let fx = run(
        &mut b,
        json!({"type": "createNode", "kind": "rectangle", "x": 10.0, "y": 20.0, "parent": null}),
    );
    assert!(
        fx.as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "sendOps")
    );
    let nodes: Value = serde_json::from_str(&b.nodes_json()).unwrap();
    assert_eq!(nodes.as_array().unwrap().len(), 1);
    let state: Value = serde_json::from_str(&b.state_json()).unwrap();
    assert_eq!(state["selection"].as_array().unwrap().len(), 1);
    assert_eq!(state["history"]["canUndo"], true);
    let events: Value = serde_json::from_str(&b.take_events_json()).unwrap();
    assert!(
        events
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "documentChanged")
    );
    // Taking drains.
    assert_eq!(b.take_events_json(), "[]");
}

#[test]
fn opacity_and_clip_cross_the_boundary_in_the_action_and_in_the_properties() {
    let mut b = bridge();
    run(&mut b, json!({"type": "start", "nowMs": 1}));
    run(&mut b, json!({"type": "connected", "nowMs": 2}));
    run(
        &mut b,
        json!({"type": "createNode", "kind": "frame", "x": 0.0, "y": 0.0, "parent": null}),
    );
    let state: Value = serde_json::from_str(&b.state_json()).unwrap();
    let id = state["selection"][0].clone();
    for property in [
        json!({"property": "opacity", "value": 0.5}),
        json!({"property": "clip", "value": false}),
    ] {
        run(
            &mut b,
            json!({"type": "setProperty", "id": id, "property": property}),
        );
    }
    let state: Value = serde_json::from_str(&b.state_json()).unwrap();
    assert_eq!(state["properties"]["opacity"], 0.5);
    assert_eq!(state["properties"]["clip"], false);
    assert_eq!(state["properties"]["bindings"], json!([]));
}

#[test]
fn bad_json_and_unknown_actions_are_errors_not_panics() {
    let mut b = bridge();
    assert!(b.dispatch("not json").is_err());
    assert!(b.dispatch(r#"{"type":"nope"}"#).is_err());
}

#[test]
fn a_file_that_does_not_load_is_an_error() {
    assert!(Bridge::new("usr_me", "web", 1, "{", 0, None).is_err());
}

#[test]
fn a_file_that_does_not_load_names_its_stable_code() {
    // Take a valid node list, then repeat the node so that the ids clash.
    let mut b = bridge();
    run(&mut b, json!({"type": "start", "nowMs": 1}));
    run(&mut b, json!({"type": "connected", "nowMs": 2}));
    run(
        &mut b,
        json!({"type": "createNode", "kind": "frame", "x": 0.0, "y": 0.0, "parent": null}),
    );
    let nodes: Vec<Value> = serde_json::from_str(&b.nodes_json()).unwrap();
    let doubled = json!([nodes[0], nodes[0]]).to_string();
    let err = Bridge::new("usr_me", "web", 1, &doubled, 0, None).unwrap_err();
    assert_eq!(err, "load: E_DUPLICATE_ID");
}

#[test]
fn whole_numbers_are_checked_not_coerced() {
    use meldframe_wasm::whole_number;
    assert_eq!(whole_number("seq", 0.0, 10.0), Ok(0));
    assert_eq!(whole_number("seq", 10.0, 10.0), Ok(10));
    for bad in [-1.0, 0.5, 11.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            whole_number("seq", bad, 10.0).is_err(),
            "{bad} must be refused"
        );
    }
    assert_eq!(whole_number("seq", -0.0, 10.0), Ok(0));
}

#[test]
fn the_scene_is_the_painted_boxes_of_the_document_as_json() {
    let mut b = bridge();
    assert_eq!(b.scene_json(), "[]");
    run(&mut b, json!({"type": "start", "nowMs": 1}));
    run(&mut b, json!({"type": "connected", "nowMs": 2}));
    run(
        &mut b,
        json!({"type": "createNode", "kind": "frame", "x": 10.0, "y": 20.0, "parent": null}),
    );
    // The new node is still pending (not confirmed by the server), and the scene already shows it.
    let nodes: Vec<Value> = serde_json::from_str(&b.nodes_json()).unwrap();
    let scene: Vec<Value> = serde_json::from_str(&b.scene_json()).unwrap();
    assert_eq!(scene.len(), 1);
    assert_eq!(scene[0]["id"], nodes[0]["id"]);
    assert_eq!(scene[0]["paint"], "frame");
    assert_eq!(
        (scene[0]["x"].as_f64(), scene[0]["y"].as_f64()),
        (Some(10.0), Some(20.0))
    );
    assert_eq!(scene[0]["parent"], Value::Null);
}

#[test]
fn the_scene_paints_variables_and_a_mode_switch_repaints_without_touching_the_nodes() {
    let nodes =
        json!([{"id": "card", "type": "rect", "pos": "a0", "fill": {"var": "bg"}}]).to_string();
    let vars = json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff", "dark": "#000000"}}]
    })
    .to_string();
    let mut b = Bridge::new("usr_me", "web", 7, &nodes, 0, Some(&vars)).expect("the file loads");
    let fill = |b: &Bridge| -> Value {
        let scene: Vec<Value> = serde_json::from_str(&b.scene_json()).unwrap();
        scene[0]["fill"].clone()
    };
    assert_eq!(fill(&b), json!(0xffffff));
    let before = b.nodes_json();
    let fx = run(
        &mut b,
        json!({"type": "setMode", "collection": "t", "mode": "dark"}),
    );
    assert_eq!(fill(&b), json!(0));
    assert!(fx.to_string().contains(r#""type":"render""#), "{fx}");
    assert_eq!(b.nodes_json(), before, "the document is the same");
    let v: Value = serde_json::from_str(&b.variables_json()).unwrap();
    assert_eq!(v["variables"][0]["id"], "bg");
    // A variables file that does not load names its code.
    let mut broken: Value = serde_json::from_str(&vars).unwrap();
    broken["variables"][0]["values"]
        .as_object_mut()
        .unwrap()
        .remove("dark");
    let broken = broken.to_string();
    assert_eq!(
        Bridge::new("usr_me", "web", 1, &nodes, 0, Some(&broken)).unwrap_err(),
        "load: E_INCOMPLETE_VALUES"
    );
}

#[test]
fn the_variables_panel_is_state_and_its_actions_are_json() {
    let nodes = json!([{"id": "card", "type": "rect", "pos": "a0"}]).to_string();
    let vars = json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff", "dark": "#000000"}}]
    })
    .to_string();
    let mut b = Bridge::new("usr_me", "web", 7, &nodes, 0, Some(&vars)).expect("the file loads");
    let state = |b: &Bridge| -> Value { serde_json::from_str(&b.state_json()).unwrap() };
    let fill = |b: &Bridge| -> Value {
        let scene: Vec<Value> = serde_json::from_str(&b.scene_json()).unwrap();
        scene[0]["fill"].clone()
    };
    // Open at first load: the list is there. Closed: nothing is listed.
    assert_eq!(state(&b)["panels"]["variables"], true);
    assert_eq!(state(&b)["variables"]["collections"][0]["id"], "t");
    run(
        &mut b,
        json!({"type": "setPanel", "panel": "variables", "open": false}),
    );
    assert_eq!(state(&b)["variables"]["collections"], json!([]));
    run(
        &mut b,
        json!({"type": "setPanel", "panel": "variables", "open": true}),
    );
    run(&mut b, json!({"type": "select", "ids": ["card"]}));
    let row = state(&b)["variables"]["collections"][0]["variables"][0].clone();
    assert_eq!(row["id"], "bg");
    assert_eq!(row["type"], "color");
    assert_eq!(row["binds"], json!([{"field": "fill", "bound": false}]));
    assert_eq!(
        row["values"][0],
        json!({"mode": "light", "text": "#ffffff", "resolved": "#ffffff"})
    );
    // Bind, switch the mode, edit a value: the canvas follows each.
    run(
        &mut b,
        json!({"type": "bindProperty", "id": "card", "field": "fill", "variable": "bg"}),
    );
    assert_eq!(fill(&b), json!(0xffffff));
    run(
        &mut b,
        json!({"type": "setMode", "collection": "t", "mode": "dark"}),
    );
    assert_eq!(fill(&b), json!(0));
    assert_eq!(state(&b)["variables"]["collections"][0]["active"], "dark");
    run(
        &mut b,
        json!({"type": "setVariableValue", "variable": "bg", "mode": "dark",
            "value": {"type": "literal", "text": "#102030"}}),
    );
    assert_eq!(fill(&b), json!(0x102030));
    run(
        &mut b,
        json!({"type": "setVariablesQuery", "query": "nothing"}),
    );
    assert_eq!(state(&b)["variables"]["matched"], 0);
    let events: Vec<Value> = serde_json::from_str(&b.take_events_json()).unwrap();
    assert!(events.contains(&json!({"type": "variablesQueryChanged", "query": "nothing"})));
}

#[test]
fn the_bind_picker_is_state_and_its_actions_are_json() {
    let nodes = json!([{"id": "card", "type": "rect", "pos": "a0", "fill": "#336699"}]).to_string();
    let vars = json!({
        "collections": [{"id": "t", "name": "appearance", "modes": [{"id": "light", "name": "light"}]}],
        "variables": [{"id": "bg", "collection": "t", "name": "color/bg", "type": "color",
            "values": {"light": "#ffffff"}}]
    })
    .to_string();
    let mut b = Bridge::new("usr_me", "web", 7, &nodes, 0, Some(&vars)).expect("the file loads");
    let state = |b: &Bridge| -> Value { serde_json::from_str(&b.state_json()).unwrap() };
    run(&mut b, json!({"type": "select", "ids": ["card"]}));
    // The picker is absent until opened, lists the colour variables, and a bind closes it.
    assert!(state(&b).get("picker").is_none());
    run(
        &mut b,
        json!({"type": "openPicker", "id": "card", "field": "fill"}),
    );
    assert_eq!(state(&b)["picker"]["rows"][0]["id"], "bg");
    run(
        &mut b,
        json!({"type": "bindProperty", "id": "card", "field": "fill", "variable": "bg"}),
    );
    assert!(state(&b).get("picker").is_none());
}

#[test]
fn fill_types_and_gradient_stops_are_state_and_actions_in_json() {
    let nodes = json!([{"id": "card", "type": "rect", "pos": "a0", "fill": "#336699"}]).to_string();
    let mut b = Bridge::new("usr_me", "web", 7, &nodes, 0, None).expect("the file loads");
    let state = |b: &Bridge| -> Value { serde_json::from_str(&b.state_json()).unwrap() };
    run(&mut b, json!({"type": "select", "ids": ["card"]}));
    assert_eq!(state(&b)["properties"]["fillKind"], "solid");
    run(
        &mut b,
        json!({"type": "setProperty", "id": "card", "property": {"property": "fillType", "value": "linear"}}),
    );
    let p = state(&b)["properties"].clone();
    assert_eq!(p["fillKind"], "linear");
    assert_eq!(
        p["gradient"]["stops"][0],
        json!({"pos": 0.0, "color": "#336699"})
    );
    run(
        &mut b,
        json!({"type": "setProperty", "id": "card", "property": {"property": "addGradientStop", "value": {}}}),
    );
    assert_eq!(
        state(&b)["properties"]["gradient"]["stops"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn the_border_and_the_shadows_are_state_and_actions_in_json() {
    let nodes = json!([{"id": "card", "type": "rect", "pos": "a0"}]).to_string();
    let mut b = Bridge::new("usr_me", "web", 7, &nodes, 0, None).expect("the file loads");
    let state = |b: &Bridge| -> Value { serde_json::from_str(&b.state_json()).unwrap() };
    run(&mut b, json!({"type": "select", "ids": ["card"]}));
    assert_eq!(state(&b)["properties"]["border"]["width"], 0.0);
    run(
        &mut b,
        json!({"type": "setProperty", "id": "card", "property": {"property": "borderWidth", "value": 2.0}}),
    );
    run(
        &mut b,
        json!({"type": "setProperty", "id": "card", "property": {"property": "addShadow", "value": {}}}),
    );
    let p = state(&b)["properties"].clone();
    assert_eq!(p["border"]["width"], 2.0);
    assert_eq!(p["shadows"][0]["blur"], 8.0);
}

#[test]
fn the_layout_fields_and_their_notes_are_state_and_actions_in_json() {
    let nodes = json!([
        {"id": "root", "type": "frame", "pos": "a0"},
        {"id": "card", "type": "rect", "pos": "a1", "parentId": "root"}
    ])
    .to_string();
    let mut b = Bridge::new("usr_me", "web", 7, &nodes, 0, None).expect("the file loads");
    let state = |b: &Bridge| -> Value { serde_json::from_str(&b.state_json()).unwrap() };
    run(&mut b, json!({"type": "select", "ids": ["card"]}));
    run(
        &mut b,
        json!({"type": "setProperty", "id": "card", "property": {"property": "sizing", "value": {"axis": "width", "mode": "fill"}}}),
    );
    let p = state(&b)["properties"].clone();
    assert_eq!(p["sw"], "fill");
    assert_eq!(p["inLayout"], false);
    assert_eq!(
        p["notes"],
        json!([{"note": "fillUnderHug", "axis": "width"}])
    );
    run(&mut b, json!({"type": "select", "ids": ["root"]}));
    run(
        &mut b,
        json!({"type": "setProperty", "id": "root", "property": {"property": "padding", "value": {"side": "top", "value": 3.0}}}),
    );
    assert_eq!(state(&b)["properties"]["padding"]["top"], 3.0);
}
