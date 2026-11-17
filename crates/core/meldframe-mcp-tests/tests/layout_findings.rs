//! E3.2: get_tree reports the layout findings of the fidelity contract (G1 to G3) next to the layer-name warnings, and the
//! tool text says so. They only warn: nothing is refused.

mod common;
use common::*;
use serde_json::json;

fn warnings(t: &serde_json::Value) -> Vec<(String, String)> {
    t["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| {
            (
                w["code"].as_str().unwrap().to_string(),
                w["nodeId"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn get_tree_warns_about_overflow_and_fill_under_hug() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "row", "name": "Row", "type": "frame", "layout": "row", "w": 100, "h": 50, "padding": 0, "gap": 0}},
            {"type": "addNode", "node": {"id": "a", "name": "Wide", "type": "rect", "parentId": "row", "w": 80, "h": 10}},
            {"type": "addNode", "node": {"id": "b", "name": "Wider", "type": "rect", "parentId": "row", "w": 80, "h": 10}},
            {"type": "addNode", "node": {"id": "hug", "name": "Hugger", "type": "frame", "layout": "row", "sw": "hug", "x": 300}},
            {"type": "addNode", "node": {"id": "c", "name": "Stretch", "type": "rect", "parentId": "hug", "sw": "fill", "w": 20, "h": 10}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let t = call(&c, "get_tree", json!({"file": "f1"})).await;
    let got = warnings(&t.body);
    assert!(got.contains(&("overflow".into(), "row".into())), "{got:?}");
    assert!(
        got.contains(&("fill_under_hug".into(), "c".into())),
        "{got:?}"
    );
}

#[tokio::test]
async fn a_tidy_layout_has_no_layout_warnings() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "row", "name": "Row", "type": "frame", "layout": "row", "w": 200, "h": 50, "padding": 0}},
            {"type": "addNode", "node": {"id": "a", "name": "Left", "type": "rect", "parentId": "row", "w": 50, "h": 10}},
            {"type": "addNode", "node": {"id": "b", "name": "Right", "type": "rect", "parentId": "row", "sw": "fill", "h": 10}}]}),
    )
    .await;
    let t = call(&c, "get_tree", json!({"file": "f1"})).await;
    assert_eq!(warnings(&t.body), []);
}

#[tokio::test]
async fn the_tool_text_mentions_layout_warnings() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.expect("tools");
    let d = tools
        .iter()
        .find(|t| t.name == "get_tree")
        .unwrap()
        .description
        .clone()
        .unwrap_or_default();
    assert!(
        d.contains("overflow") && d.contains("fill under a hug"),
        "{d}"
    );
}

#[tokio::test]
async fn layout_findings_are_not_cut_by_a_flood_of_default_names() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let first = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "row", "name": "Row", "type": "frame", "layout": "row", "w": 100, "h": 50, "padding": 0, "gap": 0}},
            {"type": "addNode", "node": {"id": "a", "name": "Wide", "type": "rect", "parentId": "row", "w": 80, "h": 10}},
            {"type": "addNode", "node": {"id": "b", "name": "Wider", "type": "rect", "parentId": "row", "w": 80, "h": 10}}]}),
    )
    .await;
    assert!(!first.is_error, "{}", first.body);
    // More default names than the cap, in two calls (the batch limit is 100 commands).
    for (k, range) in [(2, 0..60), (3, 60..120)] {
        let cmds: Vec<_> = range
            .map(|i| json!({"type": "addNode", "node": {"id": format!("r{i}"), "name": format!("Rectangle {i}"), "type": "rect", "x": 300, "w": 5, "h": 5}}))
            .collect();
        let r = call(
            &c,
            "apply_commands",
            json!({"file": "f1", "idempotencyKey": format!("k{k}"), "commands": cmds}),
        )
        .await;
        assert!(!r.is_error, "{}", r.body);
    }
    let t = call(&c, "get_tree", json!({"file": "f1"})).await;
    let got = warnings(&t.body);
    assert_eq!(got.len(), 100, "capped");
    assert!(got.contains(&("overflow".into(), "row".into())), "{got:?}");
}
