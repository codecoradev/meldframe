//! E3.2: an agent sets padding per side through apply_commands and reads it back; null makes a side follow `padding` again.

mod common;
use common::*;
use serde_json::json;

#[tokio::test]
async fn an_agent_sets_and_unsets_a_side_and_reads_it_back() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "card", "name": "Card", "type": "frame", "padding": 16, "padL": 4}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f1", "ids": ["card"]})).await;
    let node = &n.body["nodes"][0];
    assert_eq!(
        (
            node["padL"].as_f64(),
            node["padding"].as_f64(),
            node.get("padT").is_none()
        ),
        (Some(4.0), Some(16.0), true)
    );
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k2", "commands": [
            {"type": "updateNode", "id": "card", "patch": {"padL": null, "padT": 2}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f1", "ids": ["card"]})).await;
    let node = &n.body["nodes"][0];
    assert_eq!(
        (node.get("padL").is_none(), node["padT"].as_f64()),
        (true, Some(2.0))
    );
}

#[tokio::test]
async fn a_negative_side_is_refused_and_names_the_side() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "a", "name": "A", "type": "frame", "padB": -3}}]}),
    )
    .await;
    assert!(r.is_error);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("padB"),
        "{}",
        r.body
    );
}

#[tokio::test]
async fn the_tool_text_names_the_sides() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.expect("tools");
    let t = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    let all = serde_json::to_string(&t.input_schema).unwrap();
    assert!(all.contains("padT, padR, padB, padL"), "{all}");
}
