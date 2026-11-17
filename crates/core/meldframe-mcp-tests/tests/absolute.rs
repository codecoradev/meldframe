//! E3.2: an agent makes a child absolute through apply_commands and reads it back; the tool text says what it means.

mod common;
use common::*;
use serde_json::json;

#[tokio::test]
async fn an_agent_sets_absolute_and_reads_it_back() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "row", "name": "Row", "type": "frame", "layout": "row"}},
            {"type": "addNode", "node": {"id": "badge", "name": "Badge", "type": "rect", "parentId": "row", "absolute": true, "x": 4, "y": 4}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(
        &c,
        "get_nodes",
        json!({"file": "f1", "ids": ["badge", "row"]}),
    )
    .await;
    let by = |id: &str| {
        n.body["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        (
            by("badge")["absolute"].as_bool(),
            by("row")["absolute"].as_bool()
        ),
        (Some(true), Some(false))
    );
}

#[tokio::test]
async fn the_tool_text_explains_absolute() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.expect("tools");
    let t = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    let all = serde_json::to_string(&t.input_schema).unwrap();
    assert!(
        all.contains("absolute (true takes a child of an auto layout frame out of the flow"),
        "{all}"
    );
}
