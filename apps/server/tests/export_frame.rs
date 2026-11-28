//! `export_frame` (E4.1) over MCP against the real server: a frame with variables comes back as HTML and CSS, the
//! report and the stable codes are as documented, the call log has one entry without content, and the catalogue lists
//! the tool.

// The shared helpers of the MCP tests, `mcp_common`.
#[path = "mcp_common/mod.rs"]
mod common;
use common::*;
use meldframe_http::mcp::McpConfig;
use meldframe_http::{DevTokens, Shared};
use meldframe_module::RiskTier;
use meldframe_server::{Server, SharedServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

const CATALOGUE: &str = include_str!("../../../docs/mcp-tools.json");
const CODES: &str = include_str!("../../../docs/error-codes.md");

/// The server of `start`, with the export tool among the extras, as the binary composes it.
async fn start() -> Harness {
    let server = Server::assemble();
    let policy = server.registry().policy_for(export::TOOL);
    let spy = Arc::new(Spy {
        server: Arc::new(SharedServer::new(server)),
        submits: AtomicUsize::new(0),
        atomic_submits: AtomicUsize::new(0),
        reads: AtomicUsize::new(0),
    });
    let tokens = DevTokens::parse(&format!(
        "{AGENT}:ws_1:agent:alpha,{PERSON}:ws_1:user:bea,{OTHER}:ws_2:agent:gamma"
    ))
    .unwrap();
    let shared: Arc<Shared> = meldframe_http::shared(Arc::new(tokens), spy.clone(), config());
    let tool = meldframe_http::export::tool(shared.clone(), spy.server.clone(), policy);
    let mcp = McpConfig {
        extra: vec![tool],
        ..Default::default()
    };
    let app = meldframe_http::routes(shared.clone()).merge(meldframe_http::mcp::routes(
        shared,
        spy.clone(),
        mcp,
    ));
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

/// A file with an `appearance` collection (light and dark), a colour variable, and a card bound to it.
async fn seed(c: &Client) {
    let r = call(
        c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "seed", "commands": [
            {"type": "addCollection", "collection": {"id": "c1", "name": "appearance",
                "modes": [{"id": "light", "name": "Light"}, {"id": "dark", "name": "Dark"}]}},
            {"type": "addVariable", "variable": {"id": "v1", "collection": "c1", "name": "color/brand",
                "type": "color", "values": {"light": "#112233", "dark": "#aabbcc"}}},
            {"type": "addNode", "node": {"id": "card", "type": "frame", "name": "Card", "w": 300, "h": 120,
                "layout": "column", "fill": {"var": "v1"}}},
            text("title", "card", "Hello")]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

#[tokio::test]
async fn a_frame_with_variables_is_exported_as_html_and_css() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let r = call(&c, "export_frame", json!({"file": "f", "frame": "card"})).await;
    assert!(!r.is_error, "{}", r.body);
    let (html, css) = (
        r.body["html"].as_str().unwrap(),
        r.body["css"].as_str().unwrap(),
    );
    assert!(
        html.contains("<div data-node=\"card\" data-name=\"Card\""),
        "{html}"
    );
    assert!(html.contains("<p data-node=\"title\""), "{html}");
    assert!(
        css.contains("background-color:var(--appearance-color-brand)"),
        "{css}"
    );
    assert!(
        css.contains(":root{--appearance-color-brand:#112233}"),
        "{css}"
    );
    assert!(
        css.contains(":root[data-mode-appearance=\"dark\"]{--appearance-color-brand:#aabbcc}"),
        "{css}"
    );
    assert_eq!(r.body["format"], "css");
    assert_eq!(r.body["textMeasurer"], "stub");
    assert!(r.body["seq"].as_u64().unwrap() > 0);
    assert_eq!(r.body["report"]["assets"], json!([]));
}

#[tokio::test]
async fn the_format_and_the_mode_are_arguments() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let r = call(
        &c,
        "export_frame",
        json!({"file": "f", "frame": "card", "format": "tailwind", "modes": {"c1": "dark"}}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let html = r.body["html"].as_str().unwrap();
    assert!(
        html.contains("<html lang=\"en\" data-mode-appearance=\"dark\">"),
        "{html}"
    );
    assert!(
        html.contains("bg-[color:var(--appearance-color-brand)]"),
        "{html}"
    );
    assert!(
        r.body["css"]
            .as_str()
            .unwrap()
            .starts_with("@layer theme, base, utilities;")
    );
}

#[tokio::test]
async fn the_report_names_what_the_export_could_not_carry() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "s", "commands": [
            {"type": "addNode", "node": {"id": "box", "type": "frame", "name": "Box", "w": 200, "h": 100}},
            {"type": "addNode", "node": {"id": "sh", "type": "shader", "name": "Glow", "parentId": "box", "w": 50, "h": 50}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let r = call(&c, "export_frame", json!({"file": "f", "frame": "box"})).await;
    let un = &r.body["report"]["unsupported"];
    assert_eq!(un[0]["code"], "shader_approximated", "{}", r.body["report"]);
    assert_eq!(un[0]["nodeId"], "sh");
}

#[tokio::test]
async fn a_missing_frame_a_node_that_is_not_a_frame_and_a_bad_call_have_stable_codes() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    for (args, want) in [
        (json!({"file": "f", "frame": "nope"}), "E_NODE_NOT_FOUND"),
        (json!({"file": "f", "frame": "title"}), "E_NOT_A_FRAME"),
        (
            json!({"file": "f", "frame": "card", "format": "pdf"}),
            "invalid",
        ),
        (json!({"file": "a/b", "frame": "card"}), "invalid"),
        (json!({"file": "f", "frame": "a b"}), "invalid"),
        (json!({"file": "f", "frame": "x".repeat(65)}), "invalid"),
        (json!({"file": "f", "frame": "</style>"}), "invalid"),
        (json!({"file": "f"}), "bad_request"),
    ] {
        let r = call(&c, "export_frame", args.clone()).await;
        assert!(r.is_error, "{args}");
        assert_eq!(r.body["error"]["code"], want, "{args}: {}", r.body);
    }
}

#[tokio::test]
async fn an_export_over_the_size_limit_is_refused_with_its_own_code() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let big = "x".repeat(20_000);
    let mut commands = vec![frame("page", "Page")];
    for i in 0..14 {
        commands.push(text(&format!("t{i}"), "page", &big));
    }
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "big", "commands": commands}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let r = call(&c, "export_frame", json!({"file": "f", "frame": "page"})).await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "E_EXPORT_TOO_LARGE", "{}", r.body);
    assert_eq!(r.body["error"]["retryable"], false);
    assert!(
        r.body.get("html").is_none(),
        "no text comes back with the refusal"
    );
}

#[tokio::test]
async fn another_workspace_does_not_read_the_frame() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let o = connect(&h, OTHER).await;
    let r = call(&o, "export_frame", json!({"file": "f", "frame": "card"})).await;
    assert_eq!(
        r.body["error"]["code"], "E_NODE_NOT_FOUND",
        "the token decides the workspace: {}",
        r.body
    );
}

#[tokio::test]
async fn the_call_log_has_one_read_entry_and_no_content() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let before = h.spy.server.calls(100).len();
    let _ = call(&c, "export_frame", json!({"file": "f", "frame": "card"})).await;
    let _ = call(&c, "export_frame", json!({"file": "f", "frame": "nope"})).await;
    let log = h.spy.server.calls(100);
    assert_eq!(log.len(), before + 2, "{log:?}");
    let (ok, bad) = (&log[before], &log[before + 1]);
    assert_eq!(
        (ok.tool.as_str(), ok.file.as_str(), ok.outcome.as_str()),
        ("export_frame", "f", "ok")
    );
    assert_eq!(bad.outcome, "E_NODE_NOT_FOUND");
    assert_eq!(ok.risk, Some(RiskTier::Read));
    assert!(
        !format!("{ok:?}").contains("Hello"),
        "an entry has no content"
    );
}

#[tokio::test]
async fn the_catalogue_lists_the_tool_as_a_client_sees_it_and_the_codes_are_documented() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let listed = c.list_all_tools().await.unwrap();
    let live = listed
        .iter()
        .find(|t| t.name == "export_frame")
        .expect("listed");
    assert_eq!(
        live.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(true)
    );
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    let entry = file["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "export_frame")
        .expect("in docs/mcp-tools.json");
    assert_eq!(entry["tier"], "read");
    assert_eq!(entry["description"], live.description.as_deref().unwrap());
    assert_eq!(
        entry["inputSchema"],
        serde_json::to_value(&live.input_schema).unwrap()
    );
    for code in ["E_EXPORT_TOO_LARGE", "E_NODE_NOT_FOUND", "E_NOT_A_FRAME"] {
        assert!(
            CODES.contains(&format!("| `{code}` |")),
            "{code} is in docs/error-codes.md"
        );
    }
}

#[tokio::test]
async fn an_export_changes_nothing_and_a_bad_name_is_not_logged() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    let seq = call(&c, "export_frame", json!({"file": "f", "frame": "card"}))
        .await
        .body["seq"]
        .clone();
    for _ in 0..2 {
        let r = call(
            &c,
            "export_frame",
            json!({"file": "f", "frame": "card", "format": "tailwind"}),
        )
        .await;
        assert_eq!(r.body["seq"], seq, "an export does not move the file");
    }
    let before = h.spy.server.calls(100).len();
    let _ = call(
        &c,
        "export_frame",
        json!({"file": "a/b\u{2028}c", "frame": "card"}),
    )
    .await;
    let _ = call(
        &c,
        "export_frame",
        json!({"file": "f", "frame": "</style>"}),
    )
    .await;
    let log = h.spy.server.calls(100);
    assert_eq!(log.len(), before + 2, "{log:?}");
    assert_eq!(
        log[before].file, "",
        "a name that is not a file name is not logged: {log:?}"
    );
    assert_eq!(log[before].outcome, "invalid");
    assert_eq!(log[before + 1].file, "f");
    assert!(
        !format!("{log:?}").contains("style"),
        "the bad frame id is not logged: {log:?}"
    );
}

/// The probe of issue 241: two hug texts, one wide and one narrow, in a row.
async fn seed_hug_text(c: &Client) {
    let hug = |id: &str, text: &str| {
        json!({"type": "addNode", "node": {"id": id, "type": "text", "parentId": "row", "text": text,
            "fontSize": 24, "sw": "hug", "sh": "hug"}})
    };
    let r = call(
        c,
        "apply_commands",
        json!({"file": "g", "idempotencyKey": "seed-hug", "commands": [
            {"type": "addNode", "node": {"id": "row", "type": "frame", "w": 400, "h": 100,
                "layout": "row", "gap": 12, "padding": 12}},
            hug("wide", "WWWWWWWWWW MMMMMMMM"), hug("narrow", "illiiiliiil lll")]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
}

/// The server has no fonts, so the pinned sizes are the stub's; the result must say so where an agent reads it.
#[tokio::test]
async fn a_hug_text_is_reported_as_an_estimate_and_a_frame_without_one_is_not() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    seed(&c).await;
    seed_hug_text(&c).await;
    let r = call(&c, "export_frame", json!({"file": "g", "frame": "row"})).await;
    assert!(!r.is_error, "{}", r.body);
    let warnings = r.body["report"]["warnings"].as_array().unwrap();
    let est: Vec<&Value> = warnings
        .iter()
        .filter(|w| w["code"] == "text_size_estimated")
        .collect();
    assert_eq!(est.len(), 1, "{warnings:?}");
    let message = est[0]["message"].as_str().unwrap();
    assert!(
        message.contains("wide") && message.contains("narrow"),
        "{message}"
    );
    // A frame whose only text has a size of its own has nothing to estimate.
    let r = call(&c, "export_frame", json!({"file": "f", "frame": "card"})).await;
    assert!(!r.is_error, "{}", r.body);
    let warnings = r.body["report"]["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().all(|w| w["code"] != "text_size_estimated"),
        "{warnings:?}"
    );
}
