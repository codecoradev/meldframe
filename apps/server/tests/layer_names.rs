//! E3.5: a layer that keeps a default name (`Frame 3`, `Text`) is reported to the agent that made it, and the tool
//! descriptions ask for semantic names. The lint only warns: the call is written either way.

#[path = "mcp_common/mod.rs"]
mod common;
use common::*;
use serde_json::json;

fn add(id: &str, name: &str) -> serde_json::Value {
    json!({"type": "addNode", "node": {"id": id, "type": "frame", "name": name, "w": 100, "h": 50}})
}

#[tokio::test]
async fn a_default_name_in_a_batch_is_warned_about_and_the_batch_is_still_written() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [add("a", "Hero"), add("b", "Frame 3")]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["seq"], 2, "both commands are written");
    let w = r.body["warnings"].as_array().expect("warnings");
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0]["code"], "default_layer_name");
    assert_eq!(
        (w[0]["nodeId"].as_str(), w[0]["name"].as_str()),
        (Some("b"), Some("Frame 3"))
    );
    assert!(
        w[0]["message"].as_str().unwrap().contains("semantic"),
        "{}",
        w[0]["message"]
    );
}

#[tokio::test]
async fn semantic_names_make_no_warnings() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [add("a", "Hero"), add("b", "Pricing card")]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["warnings"], json!([]));
}

#[tokio::test]
async fn renaming_a_layer_to_a_default_name_is_warned_about() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [add("a", "Hero")]}),
    )
    .await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k2", "commands": [
            {"type": "updateNode", "id": "a", "patch": {"name": "Rectangle 2"}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["warnings"][0]["nodeId"], "a");
    // A patch that does not touch the name says nothing about it.
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k3", "commands": [
            {"type": "updateNode", "id": "a", "patch": {"x": 5.0}}]}),
    )
    .await;
    assert_eq!(r.body["warnings"], json!([]));
}

#[tokio::test]
async fn get_tree_lists_the_default_names_of_the_whole_file() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [add("a", "Hero"), add("b", "Text"), add("c", "Group 2")]}),
    )
    .await;
    let t = call(&c, "get_tree", json!({"file": "f1"})).await;
    let ids: Vec<&str> = t.body["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["nodeId"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["b", "c"]);
}

#[tokio::test]
async fn the_tool_descriptions_ask_for_semantic_names() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.expect("tools");
    for name in ["apply_commands", "get_tree"] {
        let d = tools
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .description
            .clone()
            .unwrap_or_default();
        assert!(d.contains("semantic name"), "{name}: {d}");
    }
}
