//! `upload_asset`: an agent sends an image as base64 and gets its id back, through a real MCP client.

#[path = "../../meldframe-mcp-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_http::{DevTokens, router_with_mcp_config};
use meldframe_server::{Server, SharedServer};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

/// The harness of `meldframe-mcp-tests` with the tool over an in-memory store with no storage limit. The tool has a
/// state of its own (the limits are those of `config`), as the server's has.
async fn start() -> Harness {
    start_with(
        Some(meldframe_module::Policy::Auto),
        Arc::new(|_| (0, None)),
    )
    .await
}

/// The same with a policy for the tool and what the workspace holds besides assets: `(bytes of ops, limit)`.
async fn start_with(
    policy: Option<meldframe_module::Policy>,
    ops_storage: meldframe_http_assets::OpsStorage,
) -> Harness {
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
    let blobs: Arc<dyn assets::BlobStore> = Arc::new(assets::MemoryBlobs::default());
    let upload = meldframe_http_assets::Assets::new(
        meldframe_http::shared(tokens.clone(), spy.clone(), config()),
        blobs.clone(),
        assets::Usage::new(blobs),
        ops_storage,
        meldframe_http_assets::Options::default(),
    );
    let mcp = meldframe_http::mcp::McpConfig {
        allowed_hosts: Vec::new(),
        extra: vec![upload.tool(policy)],
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

/// A png: the signature is all the store looks at.
fn png(tail: &[u8]) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    v.extend_from_slice(tail);
    v
}

fn b64(bytes: &[u8]) -> String {
    use std::fmt::Write;
    // The standard alphabet, written by hand so this crate needs no base64 dependency.
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.write_char('=').unwrap();
            }
        }
    }
    out
}

#[tokio::test]
async fn the_tool_is_listed_as_a_change_that_is_not_read_only() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let t = tools.iter().find(|t| t.name == "upload_asset").unwrap();
    assert_eq!(
        t.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(false)
    );
}

#[tokio::test]
async fn an_image_is_stored_and_named_by_its_bytes_and_the_same_bytes_are_the_same_asset() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let bytes = png(b"pixels");
    let first = call(&c, "upload_asset", json!({"data": b64(&bytes)})).await;
    assert!(!first.is_error, "{}", first.body);
    assert_eq!(first.body["type"], "image/png");
    assert_eq!(first.body["bytes"], bytes.len());
    let id = first.body["id"].as_str().unwrap();
    assert_eq!(id.len(), 64);
    assert_eq!(first.body["use"]["image"]["assetId"], id);
    let again = call(&c, "upload_asset", json!({"data": b64(&bytes)})).await;
    assert_eq!(again.body["id"], first.body["id"]);
}

#[tokio::test]
async fn what_is_not_an_image_or_not_base64_is_refused_with_a_code() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let svg = call(
        &c,
        "upload_asset",
        json!({"data": b64(b"<svg xmlns='http://www.w3.org/2000/svg'/>")}),
    )
    .await;
    assert!(svg.is_error);
    assert_eq!(svg.body["error"]["code"], "asset_type_refused");
    let bad = call(&c, "upload_asset", json!({"data": "this is !! not base64"})).await;
    assert!(bad.is_error);
    assert_eq!(bad.body["error"]["code"], "invalid");
}

#[tokio::test]
async fn a_call_is_logged_under_the_tool_name_with_its_outcome() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    call(&c, "upload_asset", json!({"data": b64(&png(b"logged"))})).await;
    call(&c, "upload_asset", json!({"data": b64(b"plain text")})).await;
    let entries: Vec<_> = h
        .spy
        .server
        .calls(50)
        .into_iter()
        .filter(|e| e.tool == "upload_asset")
        .map(|e| e.outcome)
        .collect();
    assert_eq!(entries, ["ok", "asset_type_refused"]);
}

#[tokio::test]
async fn a_policy_that_does_not_allow_it_refuses_the_call_and_logs_it() {
    let h = start_with(None, Arc::new(|_| (0, None))).await;
    let c = connect(&h, AGENT).await;
    let r = call(&c, "upload_asset", json!({"data": b64(&png(b"x"))})).await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "forbidden");
    let outcomes: Vec<_> = h
        .spy
        .server
        .calls(10)
        .into_iter()
        .map(|e| e.outcome)
        .collect();
    assert_eq!(outcomes, ["forbidden"]);
}

#[tokio::test]
async fn a_new_image_that_does_not_fit_the_storage_limit_is_quota_storage() {
    // The workspace holds 90 bytes of ops and may hold 100 in all: a png of 20 bytes does not fit.
    let h = start_with(
        Some(meldframe_module::Policy::Auto),
        Arc::new(|_| (90, Some(100))),
    )
    .await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "upload_asset",
        json!({"data": b64(&png(b"0123456789ab"))}),
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], "quota_storage");
    assert_eq!(r.body["error"]["retryable"], false);
}

#[tokio::test]
async fn base64_with_line_breaks_or_without_padding_is_accepted() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let bytes = png(b"pixels!");
    let text = b64(&bytes);
    let wrapped: String = text
        .as_bytes()
        .chunks(8)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let unpadded = text.trim_end_matches('=').to_owned();
    let plain = call(&c, "upload_asset", json!({"data": text})).await;
    for variant in [wrapped, unpadded] {
        let r = call(&c, "upload_asset", json!({"data": variant})).await;
        assert!(!r.is_error, "{}", r.body);
        assert_eq!(r.body["id"], plain.body["id"]);
    }
}
