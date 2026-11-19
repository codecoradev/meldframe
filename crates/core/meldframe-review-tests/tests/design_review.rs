//! `design_review`: the critique list of a file through a real MCP client. The workspace and the actor come from the
//! token; the list is not part of the file.

#[path = "../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
use common::*;
use design_review::MemoryReviews;
use meldframe_http::{DevTokens, router_with_mcp_config};
use meldframe_module::Policy;
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;

async fn start() -> Harness {
    start_with(Some(Policy::Auto), Some(Policy::Auto)).await
}

/// The harness of `meldframe-mcp-tests` with the tool over an in-memory store and the policies of the two tiers it
/// serves: `read` for a list, `change` for the rest.
async fn start_with(read: Option<Policy>, change: Option<Policy>) -> Harness {
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
    let tool = meldframe_http_review::tool(
        meldframe_http::shared(tokens.clone(), spy.clone(), config()),
        spy.clone(),
        Arc::new(MemoryReviews::default()),
        read,
        change,
    );
    let mcp = meldframe_http::mcp::McpConfig {
        allowed_hosts: Vec::new(),
        extra: vec![tool],
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

async fn review(c: &Client, args: Value) -> Called {
    call(c, "design_review", args).await
}

async fn add(c: &Client, file: &str, items: &[&str]) -> Called {
    review(c, json!({"file": file, "action": "add", "items": items})).await
}

fn code(r: &Called) -> &str {
    assert!(r.is_error, "{}", r.body);
    r.body["error"]["code"].as_str().unwrap()
}

#[tokio::test]
async fn the_tool_is_listed_as_a_change_that_is_not_read_only() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let t = tools.iter().find(|t| t.name == "design_review").unwrap();
    assert_eq!(
        t.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(false)
    );
}

#[tokio::test]
async fn items_are_numbered_per_file_and_carry_the_actor_of_the_token() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;
    let person = connect(&h, PERSON).await;
    let first = add(&agent, "f", &["Hero is cramped", "Low contrast on the CTA"]).await;
    assert!(!first.is_error, "{}", first.body);
    assert_eq!(first.body["items"][1]["n"], 2);
    let second = add(&person, "f", &["Footer links are tiny"]).await;
    assert_eq!(second.body["items"][0]["n"], 3);
    assert_eq!(second.body["total"], 3);
    assert_eq!(
        add(&agent, "g", &["Other file"]).await.body["items"][0]["n"],
        1
    );
    let list = review(&agent, json!({"file": "f", "action": "list"})).await;
    assert!(!list.is_error, "{}", list.body);
    assert_eq!(
        (list.body["open"].clone(), list.body["total"].clone()),
        (json!(3), json!(3))
    );
    let items = list.body["items"].as_array().unwrap();
    assert_eq!(items[0]["text"], "Hero is cramped");
    assert_eq!(
        (items[0]["by"].clone(), items[2]["by"].clone()),
        (json!("agent:alpha"), json!("user:bea"))
    );
    assert_eq!(
        (items[0]["status"].clone(), items[0]["batches"].clone()),
        (json!("open"), json!([]))
    );
}

#[tokio::test]
async fn an_item_links_a_batch_the_caller_wrote_and_is_marked_addressed() {
    let h = start().await;
    let agent = connect(&h, AGENT).await;
    let person = connect(&h, PERSON).await;
    add(&agent, "f", &["Hero is cramped"]).await;
    let w = call(
        &agent,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "fix1",
        "commands": [frame("hero", "Hero")]}),
    )
    .await;
    assert!(!w.is_error, "{}", w.body);
    let link = review(
        &agent,
        json!({"file": "f", "action": "link", "item": 1, "batch": "fix1"}),
    )
    .await;
    assert!(!link.is_error, "{}", link.body);
    assert_eq!(link.body["item"]["batches"], json!(["fix1"]));
    let again = review(
        &agent,
        json!({"file": "f", "action": "link", "item": 1, "batch": "fix1"}),
    )
    .await;
    assert_eq!(
        again.body["item"]["batches"],
        json!(["fix1"]),
        "linking again changes nothing"
    );
    // A batch nobody wrote, and a batch another actor wrote, are not the caller's.
    let none = review(
        &agent,
        json!({"file": "f", "action": "link", "item": 1, "batch": "nope"}),
    )
    .await;
    assert_eq!(code(&none), "batch_not_found");
    call(
        &person,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "bea1",
        "commands": [frame("foot", "Footer")]}),
    )
    .await;
    let theirs = review(
        &agent,
        json!({"file": "f", "action": "link", "item": 1, "batch": "bea1"}),
    )
    .await;
    assert_eq!(code(&theirs), "batch_not_found");
    let done = review(
        &agent,
        json!({"file": "f", "action": "mark", "item": 1, "status": "addressed"}),
    )
    .await;
    assert_eq!(done.body["item"]["status"], "addressed");
    let list = review(&person, json!({"file": "f", "action": "list"})).await;
    assert_eq!(
        (
            list.body["open"].clone(),
            list.body["items"][0]["batches"].clone()
        ),
        (json!(0), json!(["fix1"]))
    );
    let reopened = review(
        &agent,
        json!({"file": "f", "action": "mark", "item": 1, "status": "open"}),
    )
    .await;
    assert_eq!(reopened.body["item"]["status"], "open");
}

#[tokio::test]
async fn bad_input_is_refused_with_a_code_and_writes_nothing() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let long = "x".repeat(501);
    for (args, want) in [
        (json!({"file": "f", "action": "add"}), "invalid"),
        (
            json!({"file": "f", "action": "add", "items": []}),
            "invalid",
        ),
        (
            json!({"file": "f", "action": "add", "items": [long]}),
            "invalid",
        ),
        (
            json!({"file": "f", "action": "add", "items": ["ok", " "]}),
            "invalid",
        ),
        (json!({"file": "f", "action": "link", "item": 1}), "invalid"),
        (
            json!({"file": "f", "action": "link", "batch": "k"}),
            "invalid",
        ),
        (json!({"file": "f", "action": "mark", "item": 1}), "invalid"),
        (
            json!({"file": "f", "action": "mark", "item": 1, "status": "done"}),
            "invalid",
        ),
        (json!({"file": "a/b", "action": "list"}), "invalid"),
        (
            json!({"file": "f", "action": "list", "items": ["extra"]}),
            "invalid",
        ),
        (
            json!({"file": "f", "action": "list", "workspace": "ws_2"}),
            "bad_request",
        ),
        (json!({"file": "f", "action": "frobnicate"}), "bad_request"),
        (
            json!({"file": "f", "action": "mark", "item": 7, "status": "addressed"}),
            "review_item_not_found",
        ),
        (
            json!({"file": "f", "action": "link", "item": 0, "batch": "k"}),
            "invalid",
        ),
        (
            json!({"file": "f", "action": "mark", "item": 0, "status": "open"}),
            "invalid",
        ),
        (
            json!({"file": "f", "action": "add", "items": ["pay\u{202e}ment"]}),
            "invalid",
        ),
        (json!({"file": "f\u{200b}", "action": "list"}), "invalid"),
    ] {
        let r = review(&c, args.clone()).await;
        assert_eq!(code(&r), want, "{args}");
    }
    let list = review(&c, json!({"file": "f", "action": "list"})).await;
    assert_eq!(list.body["total"], 0);
}

#[tokio::test]
async fn a_file_holds_200_items_and_the_next_add_is_review_full() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let twenty: Vec<String> = (0..20).map(|i| format!("item {i}")).collect();
    let twenty: Vec<&str> = twenty.iter().map(String::as_str).collect();
    for _ in 0..10 {
        assert!(!add(&c, "f", &twenty).await.is_error);
    }
    let over = add(&c, "f", &["one more"]).await;
    assert_eq!(code(&over), "review_full");
    assert_eq!(over.body["error"]["retryable"], false);
    assert_eq!(
        review(&c, json!({"file": "f", "action": "list"}))
            .await
            .body["total"],
        200
    );
    // Another workspace has its own list, and another file of this one too.
    assert!(
        !add(&connect(&h, OTHER).await, "f", &["fine"])
            .await
            .is_error
    );
    assert!(!add(&c, "g", &["fine"]).await.is_error);
}

#[tokio::test]
async fn the_list_of_a_workspace_is_invisible_to_another_even_by_item_number() {
    let h = start().await;
    let a = connect(&h, AGENT).await;
    let other = connect(&h, OTHER).await;
    add(&a, "f", &["Secret critique"]).await;
    let theirs = review(&other, json!({"file": "f", "action": "list"})).await;
    assert_eq!(theirs.body["total"], 0);
    let link = review(
        &other,
        json!({"file": "f", "action": "link", "item": 1, "batch": "k"}),
    )
    .await;
    assert!(["review_item_not_found", "batch_not_found"].contains(&code(&link)));
    let mark = review(
        &other,
        json!({"file": "f", "action": "mark", "item": 1, "status": "addressed"}),
    )
    .await;
    assert_eq!(code(&mark), "review_item_not_found");
    let mine = review(&a, json!({"file": "f", "action": "list"})).await;
    assert_eq!(mine.body["items"][0]["status"], "open");
}

#[tokio::test]
async fn the_list_is_not_part_of_the_file() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let before = call(&c, "get_tree", json!({"file": "f"})).await.body["seq"].clone();
    add(&c, "f", &["x"]).await;
    review(
        &c,
        json!({"file": "f", "action": "mark", "item": 1, "status": "addressed"}),
    )
    .await;
    let after = call(&c, "get_tree", json!({"file": "f"})).await.body["seq"].clone();
    assert_eq!(before, after, "no op was written");
}

#[tokio::test]
async fn the_policy_of_each_tier_decides_and_every_call_is_logged_without_its_text() {
    let h = start_with(Some(Policy::Auto), None).await;
    let c = connect(&h, AGENT).await;
    assert_eq!(code(&add(&c, "f", &["SECRET text"]).await), "forbidden");
    assert!(
        !review(&c, json!({"file": "f", "action": "list"}))
            .await
            .is_error
    );
    let h2 = start_with(None, Some(Policy::Auto)).await;
    let c2 = connect(&h2, AGENT).await;
    assert!(!add(&c2, "f", &["SECRET text"]).await.is_error);
    assert_eq!(
        code(&review(&c2, json!({"file": "f", "action": "list"})).await),
        "forbidden"
    );
    let log: Vec<_> = h
        .spy
        .server
        .calls(20)
        .into_iter()
        .filter(|e| e.tool == "design_review")
        .collect();
    assert_eq!(
        log.iter().map(|e| e.outcome.as_str()).collect::<Vec<_>>(),
        ["forbidden", "ok"]
    );
    assert!(
        log.iter()
            .all(|e| e.file == "f" && e.actor == "agent:alpha" && e.workspace == "ws_1")
    );
    assert!(
        !format!("{log:?}").contains("SECRET"),
        "the log holds no text"
    );
}

#[tokio::test]
async fn a_workspace_holds_100_files_with_a_list_even_when_the_files_do_not_exist() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    for i in 0..100 {
        assert!(!add(&c, &format!("ghost{i}"), &["x"]).await.is_error);
    }
    let over = add(&c, "ghost100", &["x"]).await;
    assert_eq!(code(&over), "review_full");
    assert_eq!(over.body["error"]["retryable"], false);
    assert!(
        !add(&c, "ghost0", &["y"]).await.is_error,
        "a file with a list still takes items"
    );
    assert!(
        !add(&connect(&h, OTHER).await, "ghost100", &["x"])
            .await
            .is_error
    );
}

/// The schema a client gets equals the one in `docs/mcp-tools.json` and the tool's own function; the catalogue test of
/// `meldframe-mcp-tests` leaves this tool out because its server does not wire it.
#[tokio::test]
async fn the_input_schema_equals_the_catalogue_and_the_registry_tool() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let listed = c.list_all_tools().await.unwrap();
    let live = listed.iter().find(|t| t.name == "design_review").unwrap();
    let live_schema = serde_json::to_value(&live.input_schema).unwrap();
    let catalogue: Value =
        serde_json::from_str(include_str!("../../../../docs/mcp-tools.json")).unwrap();
    let entry = catalogue["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "design_review")
        .expect("in the catalogue");
    assert_eq!(
        live_schema, entry["inputSchema"],
        "docs/mcp-tools.json is out of date"
    );
    assert_eq!(live_schema, meldframe_http_review::input_schema());
    let reg = meldframe_server::Server::assemble();
    let reg = reg.registry().agent_tool("design_review").unwrap();
    assert_eq!(entry["description"], reg.description);
    assert_eq!(live.description.as_deref(), Some(reg.description));
    assert_eq!(entry["tier"], reg.tier.as_str());
}
