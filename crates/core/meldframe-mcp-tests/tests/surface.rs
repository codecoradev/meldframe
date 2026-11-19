//! The tools of the MCP surface (apart from make_variant, which has its own file) through a real MCP client: read tools, whoami, atomic apply_commands, both spellings of
//! the idempotency key and of `ifSeq`, conflict details, batch status.

mod common;
use common::*;
use serde_json::json;

#[tokio::test]
async fn whoami_names_the_principal_and_the_limits() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(&c, "whoami", json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["workspace"], "ws_1");
    assert_eq!(r.body["actor"], "agent:alpha");
    assert_eq!(r.body["batchLimit"], 100);
    assert_eq!(r.body["limits"]["maxNodes"], 20_000);
    assert!(r.body["rate"]["burst"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn a_batch_builds_a_tree_and_get_tree_and_get_nodes_read_it() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            frame("hero", "Hero"), text("title", "hero", "Hello"), text("sub", "hero", "World")]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["seq"], 3);
    assert_eq!(r.body["atomic"], true);
    assert_eq!(r.body["results"].as_array().unwrap().len(), 3);

    let t = call(&c, "get_tree", json!({"file": "f1"})).await;
    assert_eq!(t.body["seq"], 3);
    let root = &t.body["roots"][0];
    assert_eq!(
        (
            root["id"].as_str(),
            root["name"].as_str(),
            root["kind"].as_str()
        ),
        (Some("hero"), Some("Hero"), Some("frame"))
    );
    assert_eq!(root["lastActor"], "agent:alpha");
    let kids: Vec<&str> = root["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["id"].as_str().unwrap())
        .collect();
    assert_eq!(kids, ["title", "sub"], "children come in sibling order");

    let n = call(
        &c,
        "get_nodes",
        json!({"file": "f1", "ids": ["title", "nope"]}),
    )
    .await;
    assert_eq!(n.body["nodes"][0]["text"], "Hello");
    assert_eq!(n.body["missing"], json!(["nope"]));
    assert_eq!(n.body["seq"], 3);
}

#[tokio::test]
async fn a_multi_command_call_is_atomic_by_default() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f2", "idempotencyKey": "k", "commands": [
            frame("a", "A"), text("t", "missing_parent", "x"), frame("b", "B")]}),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "E_PARENT_NOT_FOUND");
    assert_eq!(r.body["error"]["index"], 1, "{}", r.body);
    let t = call(&c, "get_tree", json!({"file": "f2"})).await;
    assert_eq!(t.body["seq"], 0, "nothing was written");
    assert_eq!(t.body["roots"], json!([]));
}

#[tokio::test]
async fn atomic_false_keeps_the_commands_before_the_failing_one() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f3", "idempotencyKey": "k", "atomic": false, "commands": [
            frame("a", "A"), text("t", "missing_parent", "x")]}),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "E_PARENT_NOT_FOUND");
    let t = call(&c, "get_tree", json!({"file": "f3"})).await;
    assert_eq!(t.body["seq"], 1, "the first command stays written");
}

#[tokio::test]
async fn the_idempotency_key_works_in_both_spellings() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let cmds = json!([frame("a", "A"), frame("b", "B")]);
    let first = call(
        &c,
        "apply_commands",
        json!({"file": "f4", "idempotencyKey": "same", "commands": cmds}),
    )
    .await;
    assert_eq!(first.body["results"][0]["written"], true);
    let again = call(
        &c,
        "apply_commands",
        json!({"file": "f4", "idempotency_key": "same", "commands": cmds}),
    )
    .await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(again.body["results"][0]["written"], false);
    assert_eq!(again.body["results"][1]["written"], false);
    assert_eq!(
        again.body["results"][1]["seq"],
        first.body["results"][1]["seq"]
    );
    assert_eq!(again.body["replayed"], true);
    let t = call(&c, "get_tree", json!({"file": "f4"})).await;
    assert_eq!(t.body["seq"], 2, "the retry wrote nothing");
}

#[tokio::test]
async fn a_missing_empty_or_doubled_key_is_invalid() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let cmds = json!([frame("a", "A")]);
    for args in [
        json!({"file": "f5", "commands": cmds}),
        json!({"file": "f5", "idempotencyKey": "", "commands": cmds}),
        json!({"file": "f5", "idempotencyKey": "a", "idempotency_key": "b", "commands": cmds}),
    ] {
        let r = call(&c, "apply_commands", args).await;
        assert!(r.is_error);
        assert_eq!(r.body["error"]["code"], "invalid", "{}", r.body);
    }
    let t = call(&c, "get_tree", json!({"file": "f5"})).await;
    assert_eq!(t.body["seq"], 0);
}

#[tokio::test]
async fn a_batch_over_the_limit_or_empty_is_invalid() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let many: Vec<_> = (0..101).map(|i| frame(&format!("n{i}"), "n")).collect();
    for cmds in [json!(many), json!([])] {
        let r = call(
            &c,
            "apply_commands",
            json!({"file": "f6", "idempotencyKey": "k", "commands": cmds}),
        )
        .await;
        assert!(r.is_error);
        assert_eq!(r.body["error"]["code"], "invalid");
    }
    let r = call(&c, "get_tree", json!({"file": "f6"})).await;
    assert_eq!(r.body["seq"], 0);
}

#[tokio::test]
async fn a_stale_if_seq_is_a_conflict_that_says_what_changed() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;
    let person = connect(&h, PERSON).await;
    call(
        &agent,
        "apply_commands",
        json!({"file": "f7", "idempotencyKey": "k1", "commands": [frame("h", "H")]}),
    )
    .await;
    let read = call(&agent, "get_nodes", json!({"file": "f7", "ids": ["h"]})).await;
    let seen = read.body["seq"].as_u64().unwrap();
    let moved = call(
        &person,
        "apply_commands",
        json!({"file": "f7", "idempotencyKey": "p1", "commands": [mv("h", 10.0, 20.0)]}),
    )
    .await;
    assert!(!moved.is_error, "{}", moved.body);
    for (n, spelling) in ["ifSeq", "if_seq"].into_iter().enumerate() {
        let mut args = json!({"file": "f7", "idempotencyKey": format!("a{n}"),
            "commands": [mv("h", 99.0, 99.0)]});
        args[spelling] = json!(seen);
        let r = call(&agent, "apply_commands", args).await;
        assert!(r.is_error, "{spelling}: {}", r.body);
        let e = &r.body["error"];
        assert_eq!(e["code"], "conflict");
        assert_eq!(e["nodeId"], "h");
        assert_eq!(e["currentSeq"], 2);
        assert_eq!(e["current"]["x"], 10.0);
        assert_eq!(e["changes"][0]["actor"], "user:bea");
        assert_eq!(e["changes"][0]["seq"], 2);
    }
    let after = call(&agent, "get_nodes", json!({"file": "f7", "ids": ["h"]})).await;
    assert_eq!(
        after.body["nodes"][0]["x"], 10.0,
        "the stale edit wrote nothing"
    );
    let ok = call(
        &agent,
        "apply_commands",
        json!({"file": "f7", "idempotencyKey": "a9", "ifSeq": 2, "commands": [mv("h", 5.0, 5.0)]}),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
}

#[tokio::test]
async fn an_atomic_batch_with_a_stale_if_seq_on_a_later_command_writes_nothing() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;
    let person = connect(&h, PERSON).await;
    call(
        &agent,
        "apply_commands",
        json!({"file": "f8", "idempotencyKey": "k1", "commands": [frame("a", "A"), frame("b", "B")]}),
    )
    .await;
    call(
        &person,
        "apply_commands",
        json!({"file": "f8", "idempotencyKey": "p1", "commands": [mv("b", 1.0, 1.0)]}),
    )
    .await;
    let r = call(
        &agent,
        "apply_commands",
        json!({"file": "f8", "idempotencyKey": "k2", "ifSeq": 2,
            "commands": [mv("a", 7.0, 7.0), mv("b", 7.0, 7.0)]}),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "conflict");
    assert_eq!(r.body["error"]["index"], 1);
    let a = call(&agent, "get_nodes", json!({"file": "f8", "ids": ["a"]})).await;
    assert_eq!(
        a.body["nodes"][0]["x"], 0.0,
        "the first command was not written either"
    );
}

#[tokio::test]
async fn batch_status_reports_what_a_key_applied() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let none = call(
        &c,
        "batch_status",
        json!({"file": "f9", "idempotencyKey": "nope"}),
    )
    .await;
    assert_eq!(none.body["state"], "none");
    call(
        &c,
        "apply_commands",
        json!({"file": "f9", "idempotencyKey": "kk", "commands": [frame("a", "A"), frame("b", "B")]}),
    )
    .await;
    let s = call(
        &c,
        "batch_status",
        json!({"file": "f9", "idempotency_key": "kk"}),
    )
    .await;
    assert_eq!(s.body["state"], "applied");
    assert_eq!(s.body["applied"], 2);
    assert_eq!(s.body["seqs"], json!([1, 2]));
}

#[tokio::test]
async fn files_are_per_workspace_and_a_bad_file_name_is_invalid() {
    let h = start().await;
    let a = connect(&h, AGENT).await;
    let o = connect(&h, OTHER).await;
    call(
        &a,
        "apply_commands",
        json!({"file": "shared_name", "idempotencyKey": "k", "commands": [frame("a", "A")]}),
    )
    .await;
    let theirs = call(&o, "get_tree", json!({"file": "shared_name"})).await;
    assert_eq!(
        theirs.body["seq"], 0,
        "another workspace sees an empty file"
    );
    let bad = call(&a, "get_tree", json!({"file": "../x"})).await;
    assert!(bad.is_error);
    assert_eq!(bad.body["error"]["code"], "invalid");
}

#[tokio::test]
async fn a_frame_delete_asks_the_owner_and_the_retry_after_approval_is_admitted() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let add = json!({"file": "f10", "idempotencyKey": "k", "commands": [frame("a", "A")]});
    call(&c, "apply_commands", add).await;
    let delete = json!({"file": "f10", "idempotencyKey": "d", "commands": [{"type": "removeNode", "id": "a"}]});
    let r = call(&c, "apply_commands", delete.clone()).await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "approval_required");
    assert_eq!(r.body["error"]["retryable"], false);
    let id = r.body["error"]["approvalId"].as_str().unwrap().to_string();
    let owner = meldframe_host::Principal::new("ws_1", "owner:o");
    h.spy.server.decide_approval(&owner, &id, true).unwrap();
    let again = call(&c, "apply_commands", delete).await;
    assert!(!again.is_error, "{}", again.body);
}
