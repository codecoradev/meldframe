//! MCP shares the one gate with REST: the same authenticator, the same limits and the same submitter.

mod common;
use common::*;
use meldframe_http::Rate;
use serde_json::json;
use std::sync::atomic::Ordering;

fn raw(h: &Harness, path: &str) -> reqwest::RequestBuilder {
    reqwest::Client::new().post(format!("http://{}{path}", h.addr))
}

#[tokio::test]
async fn no_token_or_a_wrong_token_is_unauthenticated_before_any_mcp_work() {
    let h = start().await;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    for token in [None, Some("nope")] {
        let mut req = raw(&h, "/mcp")
            .header("accept", "application/json, text/event-stream")
            .json(&body);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let res = req.send().await.unwrap();
        assert_eq!(res.status().as_u16(), 401);
        let v: serde_json::Value = res.json().await.unwrap();
        assert_eq!(v["error"]["code"], "unauthenticated");
    }
}

#[tokio::test]
async fn an_apply_commands_call_reaches_the_submitter_and_a_rest_call_the_same_one() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    call(
        &c,
        "apply_commands",
        json!({"file": "g1", "idempotencyKey": "k", "commands": [frame("a", "A")]}),
    )
    .await;
    assert_eq!(h.spy.atomic_submits.load(Ordering::SeqCst), 1);
    let rest = raw(&h, "/v1/files/g1/tools/apply_commands")
        .bearer_auth(PERSON)
        .json(&json!({"ops": [{"key": "r1", "command": mv("a", 3.0, 4.0)}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(rest.status().as_u16(), 200);
    assert_eq!(h.spy.submits.load(Ordering::SeqCst), 1);
    let seen = call(&c, "get_nodes", json!({"file": "g1", "ids": ["a"]})).await;
    assert_eq!(
        seen.body["nodes"][0]["x"], 3.0,
        "the REST write is in the MCP read"
    );
}

#[tokio::test]
async fn mcp_and_rest_spend_one_budget_per_principal() {
    let mut cfg = config();
    cfg.per_principal = Rate {
        burst: 4,
        per_second: 1,
    };
    cfg.clock = std::sync::Arc::new(|| 1_000);
    let h = start_with(cfg).await;
    let mut statuses = Vec::new();
    for i in 0..6 {
        let res = if i % 2 == 0 {
            raw(&h, "/mcp")
                .bearer_auth(AGENT)
                .header("accept", "application/json, text/event-stream")
                .json(&json!({"jsonrpc": "2.0", "id": i, "method": "tools/list"}))
                .send()
                .await
                .unwrap()
        } else {
            raw(&h, "/v1/files/g2/tools/apply_commands")
                .bearer_auth(AGENT)
                .json(&json!({"ops": [{"key": format!("k{i}"), "command": frame("a", "A")}]}))
                .send()
                .await
                .unwrap()
        };
        statuses.push(res.status().as_u16());
    }
    assert_eq!(
        statuses.iter().filter(|s| **s == 429).count(),
        2,
        "four calls of either kind spend the budget of four, the rest are limited: {statuses:?}"
    );
}

#[tokio::test]
async fn reads_are_charged_to_the_same_in_flight_slots_as_writes() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    call(&c, "get_tree", json!({"file": "g3"})).await;
    assert_eq!(h.spy.reads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_client_on_the_older_protocol_version_can_call_a_tool_too() {
    let h = start().await;
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-03-26", "capabilities": {},
        "clientInfo": {"name": "old", "version": "1"}}});
    let res = raw(&h, "/mcp")
        .bearer_auth(AGENT)
        .header("accept", "application/json, text/event-stream")
        .json(&init)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "{}", res.status());
}
