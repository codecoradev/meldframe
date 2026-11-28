//! `make_variant`: a copy of a frame and its subtree beside it, one atomic call, safe to retry.

#[path = "../../../../apps/server/tests/mcp_common/mod.rs"]
mod common;
use common::*;
use meldframe_http::{DevTokens, router_with_mcp_config};
use meldframe_module::RiskTier;
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;

/// The harness of `mcp_common` with the tool of its own crate mounted, as the server does with the
/// `variant` feature (the tool shares the state of the routes, so it spends the same slots and limits).
async fn start() -> Harness {
    let spy = Arc::new(Spy {
        server: Arc::new(SharedServer::new(Server::assemble())),
        submits: Default::default(),
        atomic_submits: Default::default(),
        reads: Default::default(),
    });
    let tokens = Arc::new(
        DevTokens::parse(&format!(
            "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma"
        ))
        .unwrap(),
    );
    let state = meldframe_http::shared(tokens.clone(), spy.clone(), config());
    let mcp = meldframe_http::mcp::McpConfig {
        allowed_hosts: Vec::new(),
        extra: vec![meldframe_http_variant::tool(state, spy.clone())],
    };
    let app = router_with_mcp_config(tokens, spy.clone(), spy.clone(), config(), mcp);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    Harness { addr, spy }
}

/// Every `Extra` call is also logged by the MCP route, so a write has two entries of its own tool (the gate's and the
/// call's) and a refusal before the write has one, with its code: the same as `import_tokens`.
#[tokio::test]
async fn the_call_log_has_the_entry_of_the_gate_and_the_entry_of_the_call() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let before = h.spy.server.calls(100).len();
    let ok = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "v"}),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
    let refused = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "title", "idempotencyKey": "w"}),
    )
    .await;
    assert_eq!(refused.body["error"]["code"], "E_NOT_A_FRAME");
    let log = h.spy.server.calls(100);
    let mine: Vec<_> = log[before..]
        .iter()
        .filter(|e| e.tool == "make_variant")
        .collect();
    assert_eq!(mine.len(), 3, "{log:?}");
    assert!(
        mine[..2]
            .iter()
            .all(|e| e.file == "f" && e.outcome == "ok" && e.risk == Some(RiskTier::Change))
    );
    assert_eq!(mine[2].outcome, "E_NOT_A_FRAME");
}

/// A copy needs the nodes and the variables of the file together, and reads them as one picture of it: one read, under the
/// one lock, so a write between two reads cannot tear it. None of the reads the tool makes for itself (the picture of the
/// file, and the look at which keys are written) is an entry of the call log, or a dependency on the policy of another
/// tool (`get_variables`, `batch_status`), as `head` and `written` are not: the log of a copy, and of its retry, holds the
/// entries of the call itself and nothing else.
#[tokio::test]
async fn a_copy_reads_the_file_once_and_adds_nothing_but_its_own_entries_to_the_log() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let reads = h.spy.reads.load(std::sync::atomic::Ordering::SeqCst);
    let before = h.spy.server.calls(100).len();
    let ok = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "one-read"}),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
    let made = h.spy.reads.load(std::sync::atomic::Ordering::SeqCst) - reads;
    assert_eq!(made, 1, "the nodes and the variables are one read");
    let log = h.spy.server.calls(100);
    let unasked: Vec<_> = log[before..]
        .iter()
        .filter(|e| e.tool != "make_variant")
        .collect();
    assert!(
        unasked.is_empty(),
        "entries of calls the agent did not make: {unasked:?}"
    );

    // A retry with the same key is answered from the file and is the same on the log: no read entries of its own either.
    let before_retry = h.spy.server.calls(100).len();
    let again = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "one-read"}),
    )
    .await;
    assert_eq!(again.body["replayed"], true, "{}", again.body);
    let retry_log = h.spy.server.calls(100);
    let retry_unasked: Vec<_> = retry_log[before_retry..]
        .iter()
        .filter(|e| e.tool != "make_variant")
        .collect();
    assert!(
        retry_unasked.is_empty(),
        "a retry logs calls the agent did not make: {retry_unasked:?}"
    );
}

fn roots(tree: &Value) -> Vec<&Value> {
    tree["roots"].as_array().unwrap().iter().collect()
}

async fn hero(c: &Client) {
    let r = call(
        c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "seed", "commands": [
            frame("hero", "Hero"), text("title", "hero", "Hello"), text("sub", "hero", "World")]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn a_variant_copies_the_frame_and_its_children_beside_it() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "v1"}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["nodes"], 3);
    assert_eq!(r.body["replayed"], false);
    assert_eq!(r.body["seq"], 6);
    let root = r.body["root"].as_str().unwrap().to_owned();

    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    let rs = roots(&t.body);
    assert_eq!(rs.len(), 2, "the original and its variant");
    let copy = rs.iter().find(|n| n["id"] == root.as_str()).unwrap();
    assert_eq!(copy["name"], "Hero variant");
    let texts: Vec<&str> = copy["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["name"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(texts, ["title", "sub"], "the children came along, in order");
    let x = |n: &Value| n["x"].as_f64().unwrap();
    let orig = rs.iter().find(|n| n["id"] == "hero").unwrap();
    assert!(x(copy) > x(orig), "to the right of the original");
}

/// A frame whose fill and radius are bound to variables is copied with the bindings: the variables live in the file,
/// not in the frame, so the copy needs none of its own (issue 223: the check of the copy ran without the variables
/// and refused it as `E_VARIABLE_NOT_FOUND`).
#[tokio::test]
async fn a_frame_bound_to_variables_is_copied_with_its_bindings() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let seed = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "seed", "commands": [
            {"type": "addCollection", "collection": {"id": "c1", "name": "brand",
                "modes": [{"id": "light", "name": "Light"}, {"id": "dark", "name": "Dark"}]}},
            {"type": "addVariable", "variable": {"id": "v-primary", "collection": "c1", "name": "color/primary",
                "type": "color", "values": {"light": "#112233", "dark": "#aabbcc"}}},
            {"type": "addVariable", "variable": {"id": "v-round", "collection": "c1", "name": "radius/card",
                "type": "number", "values": {"light": 8, "dark": 12}}},
            {"type": "addNode", "node": {"id": "hero", "type": "frame", "name": "Hero", "w": 800, "h": 400,
                "fill": {"var": "v-primary"}, "radius": {"var": "v-round"}}},
            {"type": "addNode", "node": {"id": "card", "type": "rect", "name": "card", "parentId": "hero",
                "w": 100, "h": 50, "fill": {"var": "v-primary"}}}]}),
    )
    .await;
    assert!(!seed.is_error, "{}", seed.body);

    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "v1"}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["nodes"], 2, "the frame and its child");
    let root = r.body["root"].as_str().unwrap().to_owned();

    let copy = call(&c, "get_nodes", json!({"file": "f", "ids": [root]})).await;
    let n = &copy.body["nodes"][0];
    assert_eq!(n["fill"], json!({"var": "v-primary"}), "{}", copy.body);
    assert_eq!(n["radius"], json!({"var": "v-round"}), "{}", copy.body);
}

#[tokio::test]
async fn a_retry_with_the_same_key_makes_no_second_copy() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let args = json!({"file": "f", "id": "hero", "idempotency_key": "v2"});
    let first = call(&c, "make_variant", args.clone()).await;
    let again = call(&c, "make_variant", args).await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(again.body["replayed"], true);
    assert_eq!(again.body["root"], first.body["root"]);
    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(roots(&t.body).len(), 2);
}

#[tokio::test]
async fn only_a_frame_that_exists_can_be_copied() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "title", "idempotencyKey": "a"}),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "E_NOT_A_FRAME");
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "nope", "idempotencyKey": "b"}),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "E_NODE_NOT_FOUND");
    let r = call(&c, "make_variant", json!({"file": "f", "id": "hero"})).await;
    assert_eq!(r.body["error"]["code"], "invalid", "a key is needed");
    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(roots(&t.body).len(), 1, "nothing was written");
}

#[tokio::test]
async fn a_copy_of_more_than_a_batch_is_refused_whole() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let mut cmds = vec![frame("big", "Big")];
    for i in 0..60 {
        cmds.push(text(&format!("t{i}"), "big", "x"));
    }
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "g", "idempotencyKey": "s1", "commands": cmds}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let mut more = vec![];
    for i in 60..120 {
        more.push(text(&format!("t{i}"), "big", "x"));
    }
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "g", "idempotencyKey": "s2", "commands": more}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let r = call(
        &c,
        "make_variant",
        json!({"file": "g", "id": "big", "idempotencyKey": "v"}),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "invalid");
    let t = call(&c, "get_tree", json!({"file": "g"})).await;
    assert_eq!(roots(&t.body).len(), 1, "nothing was written");
}

#[tokio::test]
async fn a_key_used_for_another_frame_or_by_apply_commands_is_not_a_replay() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    let two = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "mk", "commands": [frame("other", "Other")]}),
    )
    .await;
    assert!(!two.is_error, "{}", two.body);
    // `mk` was used by apply_commands: not a replay, and nothing is copied.
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "mk"}),
    )
    .await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "invalid");
    // The same key for another frame after a copy: also refused, not a second copy and not a false replay.
    let ok = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "mv"}),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
    let r = call(
        &c,
        "make_variant",
        json!({"file": "f", "id": "other", "idempotencyKey": "mv"}),
    )
    .await;
    assert!(r.is_error, "{}", r.body);
    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(roots(&t.body).len(), 3, "hero, other and one copy of hero");
}

#[tokio::test]
async fn another_actor_with_the_same_key_gets_a_copy_of_its_own() {
    let h = start().await;
    let a = connect(&h, AGENT).await;
    hero(&a).await;
    let p = connect(&h, PERSON).await;
    let one = call(
        &a,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "same"}),
    )
    .await;
    let two = call(
        &p,
        "make_variant",
        json!({"file": "f", "id": "hero", "idempotencyKey": "same"}),
    )
    .await;
    assert!(!one.is_error && !two.is_error, "{} {}", one.body, two.body);
    assert_ne!(one.body["root"], two.body["root"]);
    assert_eq!(two.body["replayed"], false);
}

/// U9: the key of `make_variant` is held to the one rule of keys (no control, bidirectional or zero-width character),
/// and nothing is written.
#[tokio::test]
async fn a_key_with_a_hidden_character_is_refused_and_no_copy_is_made() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    hero(&c).await;
    for ch in ['\u{202E}', '\u{200B}', '\u{2028}', '\u{200D}', '\u{034F}'] {
        let r = call(
            &c,
            "make_variant",
            json!({"file": "f", "id": "hero", "idempotencyKey": format!("v{ch}")}),
        )
        .await;
        assert!(r.is_error, "U+{:04X}: {}", ch as u32, r.body);
        assert_eq!(r.body["error"]["code"], "invalid", "U+{:04X}", ch as u32);
    }
    let t = call(&c, "get_tree", json!({"file": "f"})).await;
    assert_eq!(roots(&t.body).len(), 1, "a refused key made a copy");
}
