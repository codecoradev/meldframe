//! E3.2: an agent sets a margin through apply_commands and reads it back; the tool text names the sides.

#[path = "mcp_common/mod.rs"]
mod common;
use common::*;
use serde_json::json;

#[tokio::test]
async fn an_agent_sets_a_margin_and_reads_it_back() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "btn", "name": "Button", "type": "rect", "marB": 12, "marT": 4}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f1", "ids": ["btn"]})).await;
    let node = &n.body["nodes"][0];
    assert_eq!(
        (
            node["marB"].as_f64(),
            node["marT"].as_f64(),
            node["marL"].as_f64()
        ),
        (Some(12.0), Some(4.0), Some(0.0))
    );
}

#[tokio::test]
async fn the_tool_text_names_the_margin_sides() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.expect("tools");
    let t = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    let all = serde_json::to_string(&t.input_schema).unwrap();
    assert!(all.contains("marT, marR, marB, marL"), "{all}");
}
