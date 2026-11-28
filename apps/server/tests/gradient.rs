//! E3.2: an agent paints a gradient through apply_commands and reads it back; a bad one is refused with its code.

#[path = "mcp_common/mod.rs"]
mod common;
use common::*;
use serde_json::json;

#[tokio::test]
async fn an_agent_sets_and_removes_a_gradient_and_reads_it_back() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let g = json!({"kind": "linear", "angle": 90, "stops": [{"pos": 0, "color": "#ff3b30"}, {"pos": 1, "color": "#0a84ff"}]});
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "hero", "name": "Hero", "type": "frame", "gradient": g.clone()}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f1", "ids": ["hero"]})).await;
    assert_eq!(
        n.body["nodes"][0]["gradient"]["stops"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k2", "commands": [
            {"type": "updateNode", "id": "hero", "patch": {"gradient": null}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f1", "ids": ["hero"]})).await;
    assert!(n.body["nodes"][0].get("gradient").is_none(), "{}", n.body);
}

#[tokio::test]
async fn a_gradient_with_one_stop_is_refused_and_says_why() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "a", "name": "A", "type": "rect", "gradient": {"stops": [{"pos": 0, "color": "#fff"}]}}}]}),
    )
    .await;
    assert!(r.is_error);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("2 stops"),
        "{}",
        r.body
    );
}

#[tokio::test]
async fn the_tool_text_explains_gradient() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.expect("tools");
    let t = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    let all = serde_json::to_string(&t.input_schema).unwrap();
    assert!(all.contains("gradient {kind linear or radial"), "{all}");
}
