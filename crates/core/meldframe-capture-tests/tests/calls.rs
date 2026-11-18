//! E2.4 on the composed server: the calls of an agent over MCP are in the call log with the decision the gate made,
//! the refused ones too, and no node content.

mod common;
use common::*;
use meldframe_calls::Decision;
use meldframe_module::RiskTier;
use serde_json::json;

fn add(id: &str, parent: Option<&str>, name: &str) -> serde_json::Value {
    json!({"type": "addNode", "node": {"id": id, "type": "frame", "name": name, "parentId": parent, "w": 100, "h": 50}})
}

#[tokio::test]
async fn an_agents_allowed_and_refused_calls_are_logged_with_the_decision_and_no_content() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    // Allowed: a batch that builds a frame with a child.
    let ok = call(
        &a,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [add("hero", None, "SECRET-HERO"), add("kid", Some("hero"), "SECRET-KID")]}),
    )
    .await;
    assert_ne!(ok.is_error, Some(true), "{ok:?}");
    // Refused: removing a frame with content needs the owner's approval, which does not exist yet.
    let refused = call(
        &a,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k2", "commands": [{"type": "removeNode", "id": "hero"}]}),
    )
    .await;
    assert_eq!(code(&refused), "forbidden");
    // `apply_commands` is a Change tool, so a delete is above its tier: the gate denies it (an approval path for it is P3).

    let log = h.server.with_server(|s| s.calls(10));
    assert_eq!(log.len(), 2, "{log:?}");
    assert_eq!(
        (
            log[0].actor.as_str(),
            log[0].tool.as_str(),
            log[0].file.as_str()
        ),
        ("agent:alpha", "apply_commands", "f1")
    );
    assert_eq!(
        (log[0].risk, log[0].decision, log[0].outcome.as_str()),
        (Some(RiskTier::Change), Some(Decision::Allow), "ok")
    );
    assert_eq!(
        (log[1].decision, log[1].outcome.as_str(), log[1].risk),
        (Some(Decision::Deny), "forbidden", Some(RiskTier::Delete))
    );
    assert!(
        !format!("{log:?}").contains("SECRET"),
        "the log holds no content"
    );
}

#[tokio::test]
async fn read_calls_are_logged_under_their_own_tool_name_and_a_conflict_adds_no_read_entry() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    call(
        &a,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [add("hero", None, "Hero")]}),
    )
    .await;
    call(&a, "get_tree", json!({"file": "f1"})).await;
    call(&a, "get_nodes", json!({"file": "f1", "ids": ["hero"]})).await;
    // A stale edit conflicts; explaining the conflict reads the file, which is part of that one call.
    let stale = call(
        &a,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k2", "ifSeq": 0, "commands": [
            {"type": "updateNode", "id": "hero", "patch": {"x": 5.0}}]}),
    )
    .await;
    assert_eq!(code(&stale), "conflict");
    let log = h.server.with_server(|s| s.calls(20));
    let seen: Vec<(&str, &str)> = log
        .iter()
        .map(|e| (e.tool.as_str(), e.outcome.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            ("apply_commands", "ok"),
            ("get_tree", "ok"),
            ("get_nodes", "ok"),
            ("apply_commands", "conflict")
        ],
        "{log:?}"
    );
    assert_eq!(log[1].risk, Some(RiskTier::Read));
    assert_eq!(log[1].decision, Some(Decision::Allow));
}

#[tokio::test]
async fn whoami_and_capture_are_logged_too() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    let me = call(&a, "whoami", json!({})).await;
    assert_ne!(me.is_error, Some(true), "{me:?}");
    // No client of the file can render, so the capture fails with its own code; it is still a call.
    let shot = call(&a, "capture", json!({"file": "f1", "nodeId": "hero"})).await;
    let shot_code = code(&shot);
    let log = h.server.with_server(|s| s.calls(10));
    let seen: Vec<(&str, &str, &str)> = log
        .iter()
        .map(|e| (e.tool.as_str(), e.file.as_str(), e.outcome.as_str()))
        .collect();
    assert_eq!(
        seen,
        [("whoami", "", "ok"), ("capture", "f1", shot_code.as_str())],
        "{log:?}"
    );
    assert!(
        !format!("{log:?}").contains("hero"),
        "no node id in the log"
    );
    for e in &log {
        assert_eq!(e.actor, "agent:alpha");
        assert_eq!(
            (e.risk, e.decision),
            (Some(RiskTier::Read), Some(Decision::Allow)),
            "{e:?}"
        );
    }
}

#[tokio::test]
async fn a_capture_with_bad_arguments_is_logged_as_malformed() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    let bad = call(&a, "capture", json!({"file": "f1", "scale": 9})).await;
    assert_eq!(code(&bad), "invalid");
    let log = h.server.with_server(|s| s.calls(10));
    assert_eq!(log.len(), 1, "{log:?}");
    assert_eq!(
        (
            log[0].tool.as_str(),
            log[0].outcome.as_str(),
            log[0].decision
        ),
        ("capture", "invalid", None)
    );
}
