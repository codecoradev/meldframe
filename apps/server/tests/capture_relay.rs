//! E1.7: an agent captures a node through an open editor; the relay's refusals as the agent sees them.

#[path = "capture_common/mod.rs"]
mod common;
use common::*;
use serde_json::json;
use std::time::{Duration, Instant};

#[tokio::test]
async fn an_agent_gets_the_png_an_open_editor_uploads() {
    let h = start().await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    let asked = tokio::spawn(async move {
        call(
            &a,
            "capture",
            json!({"file": "f1", "nodeId": "n1", "scale": 2}),
        )
        .await
    });
    let req = editor.request().await;
    assert_eq!(
        (req["nodeId"].as_str(), req["scale"].as_f64()),
        (Some("n1"), Some(2.0))
    );
    assert!(req["uploadToken"].as_str().unwrap().len() >= 16);
    assert_eq!(upload_ok(&h, PERSON, &req, valid_png()).await, 200);
    let got = asked.await.unwrap();
    assert_eq!(image(&got), valid_png());
}

#[tokio::test]
async fn without_an_editor_the_call_fails_at_once() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    let t = Instant::now();
    let r = call(&a, "capture", json!({"file": "f1"})).await;
    assert_eq!(code(&r), "no_capable_client");
    assert!(t.elapsed() < Duration::from_secs(2));
    // An editor that did not announce capture does not count either.
    let _quiet = Editor::open(&h, PERSON, "f1", &["touch"]).await;
    let r = call(&a, "capture", json!({"file": "f1"})).await;
    assert_eq!(code(&r), "no_capable_client");
}

#[tokio::test]
async fn the_session_file_is_bound_once_an_editor_is_found_and_another_is_refused() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    let r = call(&a, "capture", json!({})).await;
    assert_eq!(code(&r), "no_file_selected");
    // The message tells the caller what to do, not only what went wrong.
    let message = r.structured_content.as_ref().unwrap()["error"]["message"]
        .as_str()
        .unwrap_or_default();
    assert!(
        message.contains("\"file\""),
        "the message names the argument: {message}"
    );
    // A failed first call (a typo, no editor yet) binds nothing: the next call may name another file.
    let r = call(&a, "capture", json!({"file": "typo"})).await;
    assert_eq!(code(&r), "no_capable_client");
    let r = call(&a, "capture", json!({"file": "f2"})).await;
    assert_eq!(code(&r), "no_capable_client");
    // With an editor on f1 the capture binds the session to f1.
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let b = agent(&h, AGENT).await;
    let asked = tokio::spawn(async move { call(&b, "capture", json!({"file": "f1"})).await });
    let req = editor.request().await;
    assert_eq!(upload_ok(&h, PERSON, &req, valid_png()).await, 200);
    assert_eq!(image(&asked.await.unwrap()), valid_png());
    let r = call(&a, "capture", json!({"file": "f2"})).await;
    assert_eq!(code(&r), "room_mismatch");
    // With no file named, the bound one is used.
    let asked = tokio::spawn(async move { call(&a, "capture", json!({})).await });
    let req = editor.request().await;
    assert_eq!(req["room"], "f1");
    assert_eq!(upload_ok(&h, PERSON, &req, valid_png()).await, 200);
    assert!(!asked.await.unwrap().is_error.unwrap_or(false));
}

#[tokio::test]
async fn an_editor_of_another_workspace_neither_serves_nor_answers() {
    let h = start().await;
    let mut foreign = Editor::open(&h, OTHER, "f1", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    let r = call(&a, "capture", json!({"file": "f1"})).await;
    assert_eq!(code(&r), "no_capable_client");
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let asked = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    let req = editor.request().await;
    // The foreign client knows the request id and the token: neither its upload nor its refusal counts.
    assert_eq!(upload_ok(&h, OTHER, &req, png(100)).await, 404);
    foreign.fail(&req, "node_not_found").await;
    assert_eq!(upload_ok(&h, PERSON, &req, png(100)).await, 200);
    assert!(!asked.await.unwrap().is_error.unwrap_or(false));
}

#[tokio::test]
async fn an_oversize_image_is_refused() {
    let h = start().await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    let asked = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    let req = editor.request().await;
    assert_eq!(upload_ok(&h, PERSON, &req, png(MAX_BYTES + 1)).await, 413);
    assert_eq!(code(&asked.await.unwrap()), "too_large");
}

#[tokio::test]
async fn a_stale_editor_is_refused() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    let w = call(
        &a,
        "apply_commands",
        json!({"file": "f1", "idempotencyKey": "k1", "commands": [
            {"type": "addNode", "node": {"id": "n1", "type": "frame", "name": "n1", "w": 10, "h": 10}}]}),
    )
    .await;
    assert_ne!(w.is_error, Some(true), "{w:?}");
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let asked = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    let req = editor.request().await;
    assert_eq!(req["minSeq"], 1);
    assert_eq!(
        upload(
            &h,
            PERSON,
            &req,
            req["uploadToken"].as_str().unwrap(),
            0,
            png(100)
        )
        .await,
        422
    );
    assert_eq!(code(&asked.await.unwrap()), "stale_client");
}

#[tokio::test]
async fn a_wrong_token_is_ignored_and_the_request_times_out() {
    let h = start_with(400).await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    let asked = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    let req = editor.request().await;
    assert_eq!(
        upload(&h, PERSON, &req, "0123456789abcdef-wrong", 0, png(100)).await,
        404
    );
    assert_eq!(code(&asked.await.unwrap()), "timeout");
}

#[tokio::test]
async fn the_editor_can_refuse_and_a_closed_editor_fails_the_call() {
    let h = start().await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    let b = agent(&h, AGENT).await;
    let first = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    let req = editor.request().await;
    editor.fail(&req, "node_not_found").await;
    assert_eq!(code(&first.await.unwrap()), "node_not_found");
    let second = tokio::spawn(async move { call(&b, "capture", json!({})).await });
    let _req = editor.request().await;
    drop(editor);
    assert_eq!(code(&second.await.unwrap()), "client_disconnected");
}

#[tokio::test]
async fn an_upload_needs_a_valid_login() {
    let h = start().await;
    let req = json!({"requestId": "cap_x"});
    assert_eq!(
        upload(&h, "bad-token-zzzzzzzzz", &req, "t", 0, png(10)).await,
        401
    );
}

const CATALOGUE: &str = include_str!("../../../docs/mcp-tools.json");

#[tokio::test]
async fn the_catalogue_lists_the_read_tier_capture_tool() {
    let h = start().await;
    let a = agent(&h, AGENT).await;
    let listed = a.list_all_tools().await.unwrap();
    let t = listed.iter().find(|t| t.name == "capture").expect("listed");
    let file: serde_json::Value = serde_json::from_str(CATALOGUE).unwrap();
    let entry = file["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "capture")
        .expect("in the catalogue");
    assert_eq!(entry["description"], t.description.as_deref().unwrap());
    assert_eq!(
        entry["inputSchema"],
        serde_json::to_value(&t.input_schema).unwrap()
    );
    let server = meldframe_server::Server::assemble();
    let reg = server.registry().agent_tool("capture").unwrap();
    assert_eq!((reg.module, reg.tier.as_str()), ("capture", "read"));
    assert_eq!(entry["tier"], "read");
    assert_eq!(entry["description"], reg.description);
    assert_eq!(
        t.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(true)
    );
}

/// A chunked upload has no Content-Length: the limit is enforced while the body streams in.
#[tokio::test]
async fn a_chunked_oversize_upload_is_stopped_while_streaming() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let h = start().await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    let asked = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    let req = editor.request().await;
    let (id, token) = (
        req["requestId"].as_str().unwrap(),
        req["uploadToken"].as_str().unwrap(),
    );
    let mut s = tokio::net::TcpStream::connect(h.addr).await.unwrap();
    let head = format!(
        "POST /captures/{id} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {PERSON}\r\nX-Capture-Token: {token}\r\nX-Capture-Seq: 0\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    s.write_all(head.as_bytes()).await.unwrap();
    for _ in 0..3 {
        let chunk = png(MAX_BYTES);
        let _ = s
            .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
            .await;
        let _ = s.write_all(&chunk).await;
        let _ = s.write_all(b"\r\n").await;
    }
    let _ = s.write_all(b"0\r\n\r\n").await;
    let mut out = String::new();
    let _ = s.read_to_string(&mut out).await;
    assert!(out.starts_with("HTTP/1.1 413"), "{out}");
    assert_eq!(code(&asked.await.unwrap()), "too_large");
}
