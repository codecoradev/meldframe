//! The M1 demonstration, steps 1, 2 and 6, with an MCP agent and a person who edits over REST.

mod common;
use common::*;
use serde_json::json;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn an_agent_builds_a_hero_in_one_batch_and_its_stale_edit_of_the_heading_conflicts() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;

    // 1. A landing hero (a frame with heading, text and a button) in one atomic batch.
    let hero = call(
        &agent,
        "apply_commands",
        json!({"file": "landing", "idempotencyKey": "hero-1", "commands": [
            frame("hero", "Hero"),
            text("heading", "hero", "Design with agents"),
            text("blurb", "hero", "One canvas for people and agents."),
            json!({"type": "addNode", "node": {"id": "button", "type": "frame", "name": "Button", "parentId": "hero"}}),
            text("button_label", "button", "Get started"),
        ]}),
    )
    .await;
    assert!(!hero.is_error, "{}", hero.body);
    assert_eq!(hero.body["seq"], 5);
    let tree = call(&agent, "get_tree", json!({"file": "landing"})).await;
    assert_eq!(
        tree.body["roots"][0]["children"].as_array().unwrap().len(),
        3
    );

    // 2. The person moves the heading (over REST, as the editor's transport would).
    let seen = tree.body["seq"].as_u64().unwrap();
    let rest = reqwest::Client::new()
        .post(format!(
            "http://{}/v1/files/landing/tools/apply_commands",
            h.addr
        ))
        .bearer_auth(PERSON)
        .json(&json!({"ops": [{"key": "move-1", "command": mv("heading", 40.0, 60.0)}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(rest.status().as_u16(), 200);

    // The agent's next read shows the new position.
    let read = call(
        &agent,
        "get_nodes",
        json!({"file": "landing", "ids": ["heading"]}),
    )
    .await;
    assert_eq!(
        (
            read.body["nodes"][0]["x"].as_f64(),
            read.body["nodes"][0]["y"].as_f64()
        ),
        (Some(40.0), Some(60.0))
    );

    // Its edit made from the old read is rejected with `conflict`, and says who changed what.
    let stale = call(
        &agent,
        "apply_commands",
        json!({"file": "landing", "idempotencyKey": "retitle", "ifSeq": seen, "commands": [
            json!({"type": "updateNode", "id": "heading", "patch": {"text": "Design faster"}})]}),
    )
    .await;
    assert!(stale.is_error);
    assert_eq!(stale.body["error"]["code"], "conflict");
    assert_eq!(stale.body["error"]["changes"][0]["actor"], "user:bea");

    // The agent's work stays: the hero is still whole.
    let after = call(&agent, "get_tree", json!({"file": "landing"})).await;
    assert_eq!(after.body["seq"], 6);

    // 6. Every write passed the one gate: the submitter saw the agent's batch once and the person's call once.
    assert_eq!(
        h.spy.atomic_submits.load(Ordering::SeqCst),
        2,
        "two MCP writes"
    );
    assert_eq!(h.spy.submits.load(Ordering::SeqCst), 1, "one REST write");
}
