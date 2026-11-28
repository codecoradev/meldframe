//! E3.2: an agent sets opacity and no fill through apply_commands and reads them back; a bad opacity is refused.

#[path = "mcp_common/mod.rs"]
mod common;
use common::*;
use serde_json::json;

#[tokio::test]
async fn an_agent_sets_opacity_and_no_fill_and_reads_them_back() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "veil", "name": "Veil", "type": "rect", "fill": "none", "opacity": 0.5}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f1", "ids": ["veil"]})).await;
    let node = &n.body["nodes"][0];
    assert_eq!(
        (node["fill"].as_str(), node["opacity"].as_f64()),
        (Some("none"), Some(0.5))
    );
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k2", "commands": [
            {"type": "updateNode", "id": "veil", "patch": {"opacity": 0.2}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f1", "ids": ["veil"]})).await;
    assert_eq!(n.body["nodes"][0]["opacity"].as_f64(), Some(0.2));
}

#[tokio::test]
async fn an_opacity_above_one_is_refused_and_names_the_field() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "a", "name": "A", "type": "rect", "opacity": 3}}]}),
    )
    .await;
    assert!(r.is_error);
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("opacity"),
        "{}",
        r.body
    );
}

#[tokio::test]
async fn the_tool_text_tells_an_agent_about_opacity_and_no_fill() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.expect("tools");
    let t = tools.iter().find(|t| t.name == "apply_commands").unwrap();
    let all = format!(
        "{} {}",
        t.description.clone().unwrap_or_default(),
        serde_json::to_string(&t.input_schema).unwrap()
    );
    assert!(
        all.contains("the word none for no fill") && all.contains("opacity (0 to 1"),
        "{all}"
    );
}
