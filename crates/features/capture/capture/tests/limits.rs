//! The relay's limits and cleanup: aborts, per-workspace and per-principal caps, refused editors, closed
//! streams, and the peek the upload route makes before it reads a body.

use capture::{Begin, CaptureError, Config, Relay, Token};

fn png() -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    v.extend(10u32.to_be_bytes());
    v.extend(10u32.to_be_bytes());
    v.resize(40, 0);
    v
}

fn tok(id: &str) -> String {
    format!("token-{id}-0123456789")
}

fn cfg() -> Config {
    Config {
        max_bytes: 64,
        max_pixels: 1_000_000,
        timeout_ms: 1000,
        max_pending: 4,
        max_pending_per_client: 2,
        max_clients: 3,
        max_pending_per_workspace: 4,
        max_pending_per_principal: 4,
        max_clients_per_workspace: 3,
    }
}

fn begin(id: &str, ws: &str, file: &str) -> Begin {
    Begin {
        workspace: ws.into(),
        actor: "agent".into(),
        file: file.into(),
        node_id: Some("n1".into()),
        scale: 1.0,
        request_id: id.into(),
        upload_token: Token::new(&tok(id)).unwrap(),
        min_seq: 7,
    }
}

fn capable(r: &mut Relay<u32>, ws: &str, file: &str, c: u32) {
    r.set_capabilities(ws, file, c, &["capture".to_string()])
        .unwrap();
}

fn begin_as(id: &str, ws: &str, file: &str, actor: &str) -> Begin {
    Begin {
        actor: actor.into(),
        ..begin(id, ws, file)
    }
}

#[test]
fn an_abort_fails_only_its_request_and_frees_the_client() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin_as("a", "w", "f", "x")).unwrap();
    r.begin(0, begin_as("b", "w", "f", "y")).unwrap();
    assert!(r.abort("w2", "a", CaptureError::Timeout).is_none());
    let done = r.abort("w", "a", CaptureError::ClientDisconnected).unwrap();
    assert_eq!(done.result.unwrap_err().code(), "client_disconnected");
    assert!(r.abort("w", "a", CaptureError::Timeout).is_none());
    assert_eq!(r.pending(), 1);
    // the client is still capable and its load slot is free again
    r.begin(0, begin("c", "w", "f")).unwrap();
    assert!(r.upload("w", "b", &tok("b"), 7, &png()).is_some());
}

#[test]
fn pending_requests_are_capped_per_workspace_and_per_principal() {
    let mut c = cfg();
    c.max_pending_per_workspace = 2;
    c.max_pending_per_principal = 1;
    c.max_pending_per_client = 9;
    let mut r = Relay::<u32>::new(c);
    capable(&mut r, "w", "f", 1);
    capable(&mut r, "v", "f", 2);
    r.begin(0, begin_as("a", "w", "f", "x")).unwrap();
    let e = r.begin(0, begin_as("b", "w", "f", "x")).unwrap_err();
    assert_eq!(e.code(), "busy");
    r.begin(0, begin_as("b", "w", "f", "y")).unwrap();
    assert_eq!(
        r.begin(0, begin_as("c", "w", "f", "z")).unwrap_err().code(),
        "busy"
    );
    // another workspace is not affected
    r.begin(0, begin_as("c", "v", "f", "x")).unwrap();
}

#[test]
fn capable_clients_are_capped_per_workspace_and_the_agent_hears_busy() {
    let mut c = cfg();
    c.max_clients_per_workspace = 1;
    let mut r = Relay::<u32>::new(c);
    capable(&mut r, "w", "f", 1);
    let e = r
        .set_capabilities("w", "g", 2, &["capture".to_string()])
        .unwrap_err();
    assert_eq!(e.code(), "busy");
    capable(&mut r, "v", "g", 3);
    // the cap, not absence, is why file g has no editor
    assert_eq!(r.begin(0, begin("a", "w", "g")).unwrap_err().code(), "busy");
    assert_eq!(
        r.begin(0, begin("a", "w", "h")).unwrap_err().code(),
        "no_capable_client"
    );
    // once the refused client is gone it is absence again
    r.disconnect(2);
    assert_eq!(
        r.begin(0, begin("a", "w", "g")).unwrap_err().code(),
        "no_capable_client"
    );
}

#[test]
fn a_closed_stream_cannot_register_again() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.disconnect(1);
    capable(&mut r, "w", "f", 1);
    assert_eq!(
        r.begin(0, begin("a", "w", "f")).unwrap_err().code(),
        "no_capable_client"
    );
}

#[test]
fn forbidden_is_a_capture_code() {
    assert_eq!(CaptureError::Forbidden.code(), "forbidden");
}

#[test]
fn is_pending_peeks_without_changing_anything() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    assert!(r.is_pending("w", "a", &tok("a")));
    assert!(!r.is_pending("w", "a", "wrong-token-0123456"));
    assert!(!r.is_pending("w2", "a", &tok("a")));
    assert!(!r.is_pending("w", "b", &tok("a")));
    assert_eq!(r.pending(), 1);
    assert!(r.upload("w", "a", &tok("a"), 7, &png()).is_some());
    assert!(!r.is_pending("w", "a", &tok("a")));
}
