//! The relay under pressure: what the upload route reads, what a failed call leaves behind, limits and
//! who can see or answer what.

mod common;
use common::*;
use meldframe_host::Principal;
use meldframe_http_capture::Options;
use meldframe_module::Policy;
use serde_json::json;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

async fn pending(
    h: &Harness,
    editor: &mut Editor,
    who: &'static str,
) -> (
    serde_json::Value,
    tokio::task::JoinHandle<rmcp::model::CallToolResult>,
) {
    let a = agent(h, who).await;
    let t = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    (editor.request().await, t)
}

#[tokio::test]
async fn an_unknown_id_or_a_wrong_token_never_has_its_body_read() {
    let h = start().await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let (req, asked) = pending(&h, &mut editor, AGENT).await;
    // The body never arrives; the answer comes at once all the same.
    let mut s = raw_upload(&h, PERSON, "cap_unknown", "0123456789abcdef0123", CHUNKED).await;
    let out = response(&mut s, Duration::from_secs(2)).await;
    assert!(out.starts_with("HTTP/1.1 404"), "{out}");
    let id = req["requestId"].as_str().unwrap();
    let mut s = raw_upload(&h, PERSON, id, "wrong-token-0123456789", CHUNKED).await;
    let out = response(&mut s, Duration::from_secs(2)).await;
    assert!(out.starts_with("HTTP/1.1 404"), "{out}");
    // The request is still pending and the right upload completes it.
    assert_eq!(upload_ok(&h, PERSON, &req, valid_png()).await, 200);
    assert!(!asked.await.unwrap().is_error.unwrap_or(false));
}

#[tokio::test]
async fn uploads_over_the_place_cap_are_refused() {
    let h = start_setup(|s| {
        s.options = Options {
            uploads_per_principal: 1,
            ..Options::default()
        }
    })
    .await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let (r1, _a) = pending(&h, &mut editor, AGENT).await;
    let (r2, _b) = pending(&h, &mut editor, DELTA).await;
    let tok = |r: &serde_json::Value| r["uploadToken"].as_str().unwrap().to_string();
    // The first upload holds its place while its body is slow.
    let mut slow = raw_upload(
        &h,
        PERSON,
        r1["requestId"].as_str().unwrap(),
        &tok(&r1),
        CHUNKED,
    )
    .await;
    slow.write_all(b"1\r\nx\r\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(upload_ok(&h, PERSON, &r2, valid_png()).await, 503);
    drop(slow);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(upload_ok(&h, PERSON, &r2, valid_png()).await, 200);
}

#[tokio::test]
async fn a_second_upload_and_an_upload_after_expiry_are_ignored() {
    let h = start_with(300).await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let (r1, a1) = pending(&h, &mut editor, AGENT).await;
    assert_eq!(upload_ok(&h, PERSON, &r1, valid_png()).await, 200);
    assert_eq!(upload_ok(&h, PERSON, &r1, valid_png()).await, 404);
    assert!(!a1.await.unwrap().is_error.unwrap_or(false));
    let (r2, a2) = pending(&h, &mut editor, AGENT).await;
    assert_eq!(code(&a2.await.unwrap()), "timeout");
    assert_eq!(upload_ok(&h, PERSON, &r2, valid_png()).await, 404);
}

#[tokio::test]
async fn a_lying_content_length_and_a_slow_body_fail_the_upload_only() {
    let h = start_setup(|s| s.http.body_timeout = Duration::from_millis(400)).await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let (req, asked) = pending(&h, &mut editor, AGENT).await;
    let (id, token) = (
        req["requestId"].as_str().unwrap(),
        req["uploadToken"].as_str().unwrap(),
    );
    // Says 100 bytes, sends 10 and goes away: a body error, 400.
    let mut s = raw_upload(&h, PERSON, id, token, "Content-Length: 100").await;
    s.write_all(&[7u8; 10]).await.unwrap();
    s.shutdown().await.unwrap();
    assert!(
        response(&mut s, Duration::from_secs(3))
            .await
            .starts_with("HTTP/1.1 400")
    );
    // Sends a little and then nothing: the body timeout, 408.
    let mut s = raw_upload(&h, PERSON, id, token, CHUNKED).await;
    s.write_all(b"1\r\nx\r\n").await.unwrap();
    assert!(
        response(&mut s, Duration::from_secs(3))
            .await
            .starts_with("HTTP/1.1 408")
    );
    // Neither touched the request.
    assert_eq!(upload_ok(&h, PERSON, &req, valid_png()).await, 200);
    assert!(!asked.await.unwrap().is_error.unwrap_or(false));
}

#[tokio::test]
async fn a_publish_that_fails_fails_only_that_call_and_the_editor_stays_capable() {
    let h = start().await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let (other, theirs) = pending(&h, &mut editor, DELTA).await;
    h.source.fail_publish.store(true, SeqCst);
    let a = agent(&h, AGENT).await;
    let r = call(&a, "capture", json!({"file": "f1"})).await;
    assert_eq!(code(&r), "client_disconnected");
    h.source.fail_publish.store(false, SeqCst);
    // Another principal's pending capture is unaffected, and the editor still serves.
    assert_eq!(upload_ok(&h, PERSON, &other, valid_png()).await, 200);
    assert!(!theirs.await.unwrap().is_error.unwrap_or(false));
    let (req, mine) = pending(&h, &mut editor, AGENT).await;
    assert_eq!(upload_ok(&h, PERSON, &req, valid_png()).await, 200);
    assert!(!mine.await.unwrap().is_error.unwrap_or(false));
}

#[tokio::test]
async fn a_dropped_call_frees_the_editors_place_at_once() {
    let h = start_setup(|s| s.relay.max_pending_per_client = 1).await;
    let mut editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let tool = h.cap.tool();
    let who = Principal::new("ws_1", "agent:alpha");
    let args = json!({"file": "f1"}).as_object().cloned().unwrap();
    let first = tokio::time::timeout(
        Duration::from_millis(400),
        tool.call(who.clone(), args.clone()),
    )
    .await;
    assert!(first.is_err(), "no answer yet: the call was dropped");
    let _ = editor.request().await;
    // The place is free: the next call is sent, not refused as busy.
    let again = tokio::time::timeout(Duration::from_millis(400), tool.call(who, args)).await;
    assert!(again.is_err());
    let _ = editor.request().await;
}

#[tokio::test]
async fn closing_an_editor_fails_only_its_own_requests() {
    let h = start().await;
    let mut one = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let mut two = Editor::open(&h, PERSON, "f2", &["capture"]).await;
    let (_r1, first) = pending(&h, &mut one, AGENT).await;
    let b = agent(&h, DELTA).await;
    let second = tokio::spawn(async move { call(&b, "capture", json!({"file": "f2"})).await });
    let req2 = two.request().await;
    drop(one);
    assert_eq!(code(&first.await.unwrap()), "client_disconnected");
    assert_eq!(upload_ok(&h, PERSON, &req2, valid_png()).await, 200);
    assert!(!second.await.unwrap().is_error.unwrap_or(false));
}

#[tokio::test]
async fn an_editor_of_another_file_is_never_asked_and_a_second_socket_never_sees_the_token() {
    let h = start().await;
    let mut other_file = Editor::open(&h, PERSON, "f2", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    assert_eq!(
        code(&call(&a, "capture", json!({"file": "f1"})).await),
        "no_capable_client"
    );
    let mut first = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let mut second = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let asked = tokio::spawn(async move { call(&a, "capture", json!({"file": "f1"})).await });
    let req = first.request().await;
    // Neither the other file's editor nor the second socket of this file is sent anything.
    for e in [&mut other_file, &mut second] {
        let quiet = tokio::time::timeout(Duration::from_millis(400), e.request()).await;
        assert!(quiet.is_err(), "a capture request reached the wrong socket");
    }
    assert_eq!(upload_ok(&h, PERSON, &req, valid_png()).await, 200);
    assert!(!asked.await.unwrap().is_error.unwrap_or(false));
}

#[tokio::test]
async fn too_many_editors_in_a_workspace_are_told_busy_and_the_agent_hears_busy() {
    let h = start_setup(|s| s.relay.max_clients_per_workspace = 1).await;
    let _first = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let second = Editor::open(&h, PERSON, "f2", &["capture"]).await;
    let told = second
        .early
        .iter()
        .find(|v| v["type"] == "error")
        .expect("told")
        .clone();
    assert_eq!(
        (told["type"].as_str(), told["code"].as_str()),
        (Some("error"), Some("busy"))
    );
    let a = agent(&h, AGENT).await;
    assert_eq!(
        code(&call(&a, "capture", json!({"file": "f2"})).await),
        "busy"
    );
}

#[tokio::test]
async fn a_policy_that_is_not_auto_refuses_the_tool() {
    let h = start_setup(|s| s.options.policy = Some(Policy::Deny)).await;
    let _editor = Editor::open(&h, PERSON, "f1", &["capture"]).await;
    let a = agent(&h, AGENT).await;
    assert_eq!(
        code(&call(&a, "capture", json!({"file": "f1"})).await),
        "forbidden"
    );
}
