//! The capture relay: pending requests, capability registration, workspace checks, size limit,
//! timeouts and the session binding. Pure and synchronous: the clock is the `now` argument.

use capture::{Begin, Binding, CaptureError, Config, Relay, Token, ct_eq};
use meldframe_protocol::ServerMsg;

/// A PNG header of `w` x `h` padded to `len` bytes.
fn png_of(w: u32, h: u32, len: usize) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    v.extend(w.to_be_bytes());
    v.extend(h.to_be_bytes());
    v.resize(len.max(33), 0);
    v
}

fn png() -> Vec<u8> {
    png_of(10, 10, 40)
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

#[test]
fn no_client_is_no_capable_client_at_once() {
    let mut r = Relay::<u32>::new(cfg());
    let e = r.begin(0, begin("a", "w", "f")).unwrap_err();
    assert_eq!(e.code(), "no_capable_client");
    r.set_capabilities("w", "f", 1, &["touch".to_string()])
        .unwrap();
    assert_eq!(
        r.begin(0, begin("a", "w", "f")).unwrap_err(),
        CaptureError::NoCapableClient
    );
    assert_eq!(r.pending(), 0);
}

#[test]
fn a_client_of_another_file_or_workspace_is_not_capable_for_this_one() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w2", "f", 1);
    capable(&mut r, "w", "other", 2);
    assert_eq!(
        r.begin(0, begin("a", "w", "f")).unwrap_err(),
        CaptureError::NoCapableClient
    );
}

#[test]
fn the_request_goes_to_a_capable_client_as_an_ephemeral_frame_body() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 3);
    let (client, msg) = r.begin(100, begin("a", "w", "f")).unwrap();
    assert_eq!(client, 3);
    let ServerMsg::CaptureRequest {
        request_id,
        room,
        node_id,
        scale,
        max_pixels,
        min_seq,
        upload_token,
        timeout_ms,
    } = msg
    else {
        panic!("not a capture request")
    };
    assert_eq!((request_id.as_str(), room.as_str()), ("a", "f"));
    assert_eq!(node_id.as_deref(), Some("n1"));
    assert_eq!((scale, max_pixels, min_seq), (1.0, 1_000_000, 7));
    assert_eq!(
        (upload_token.as_str(), timeout_ms),
        (tok("a").as_str(), 1000)
    );
    assert_eq!(r.pending(), 1);
}

#[test]
fn capability_can_be_withdrawn() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 3);
    r.set_capabilities("w", "f", 3, &[]).unwrap();
    assert!(r.begin(0, begin("a", "w", "f")).is_err());
}

#[test]
fn the_least_loaded_client_is_chosen() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    capable(&mut r, "w", "f", 2);
    let first = r.begin(0, begin("a", "w", "f")).unwrap().0;
    let second = r.begin(0, begin("b", "w", "f")).unwrap().0;
    assert_ne!(first, second);
}

#[test]
fn a_request_id_has_one_outstanding_request() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    let e = r.begin(0, begin("a", "w", "f")).unwrap_err();
    assert_eq!(e.code(), "invalid");
    assert_eq!(r.pending(), 1);
}

#[test]
fn a_bad_scale_is_invalid() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    for s in [0.0, -1.0, f64::NAN, f64::INFINITY, 9.0] {
        let mut b = begin("a", "w", "f");
        b.scale = s;
        assert_eq!(r.begin(0, b).unwrap_err().code(), "invalid", "{s}");
    }
}

#[test]
fn an_upload_completes_the_request_once() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    let done = r.upload("w", "a", &tok("a"), 7, &png()).unwrap();
    assert_eq!(done.request_id, "a");
    assert_eq!(done.result.unwrap(), png());
    assert_eq!(r.pending(), 0);
    assert!(r.upload("w", "a", &tok("a"), 7, &png()).is_none());
}

#[test]
fn an_answer_without_a_matching_request_is_ignored() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    assert!(r.upload("w", "nope", &tok("a"), 7, &png()).is_none());
    assert!(r.upload("w", "a", "wrong", 7, &png()).is_none());
    assert!(r.upload("w2", "a", &tok("a"), 7, &png()).is_none());
    assert!(r.failed("w", 2, "a", "render_failed", "x").is_none());
    assert_eq!(r.pending(), 1);
}

#[test]
fn a_bad_image_fails_the_request_with_a_code() {
    let cases: [(Vec<u8>, u64, &str); 8] = [
        (vec![0x89; 65], 7, "too_large"),
        (b"not a png".to_vec(), 7, "render_failed"),
        (png(), 6, "stale_client"),
        (png_of(60000, 60000, 70), 7, "too_large"),
        (png_of(0, 10, 40), 7, "render_failed"),
        (png_of(10, 0, 40), 7, "render_failed"),
        (png()[..32].to_vec(), 7, "render_failed"),
        (
            {
                let mut p = png();
                p[12..16].copy_from_slice(b"XXXX");
                p
            },
            7,
            "render_failed",
        ),
    ];
    for (body, seq, code) in cases {
        let mut r = Relay::<u32>::new(cfg());
        capable(&mut r, "w", "f", 1);
        r.begin(0, begin("a", "w", "f")).unwrap();
        let done = r.upload("w", "a", &tok("a"), seq, &body).unwrap();
        assert_eq!(done.result.unwrap_err().code(), code);
        assert_eq!(r.pending(), 0);
    }
}

#[test]
fn pixels_and_bytes_are_limited_at_the_boundary() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    for (id, w, h, len, ok) in [
        ("a", 1000, 1000, 40, true),
        ("b", 1000, 1001, 40, false),
        ("c", 10, 10, 64, true),
        ("d", 10, 10, 65, false),
    ] {
        r.begin(0, begin(id, "w", "f")).unwrap();
        let d = r.upload("w", id, &tok(id), 7, &png_of(w, h, len)).unwrap();
        assert_eq!(d.result.is_ok(), ok, "{id}");
    }
    assert_eq!(r.max_bytes(), 64);
}

#[test]
fn the_client_can_refuse_and_only_it_is_heard() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    assert!(r.failed("w", 9, "a", "node_not_found", "x").is_none());
    let done = r.failed("w", 1, "a", "node_not_found", "no n1").unwrap();
    assert_eq!(done.result.unwrap_err().code(), "node_not_found");
    r.begin(0, begin("b", "w", "f")).unwrap();
    let done = r.failed("w", 1, "b", "made_up", "x").unwrap();
    assert_eq!(done.result.unwrap_err().code(), "render_failed");
}

#[test]
fn a_request_times_out() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(100, begin("a", "w", "f")).unwrap();
    assert!(r.expire(1099).is_empty());
    let done = r.expire(1100);
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].result.as_ref().unwrap_err().code(), "timeout");
    assert!(r.upload("w", "a", &tok("a"), 7, &png()).is_none());
}

#[test]
fn a_disconnect_fails_its_pending_requests_and_drops_its_capability() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    let done = r.disconnect(1);
    assert_eq!(done.len(), 1);
    assert_eq!(
        done[0].result.as_ref().unwrap_err().code(),
        "client_disconnected"
    );
    assert!(r.begin(0, begin("b", "w", "f")).is_err());
}

#[test]
fn the_session_binds_one_file_and_refuses_another() {
    let mut b = Binding::default();
    assert_eq!(b.resolve(None).unwrap_err().code(), "no_file_selected");
    b.bind("f1").unwrap();
    assert_eq!(b.resolve(None).unwrap(), "f1");
    assert_eq!(b.resolve(Some("f1")).unwrap(), "f1");
    let e = b.resolve(Some("f2")).unwrap_err();
    assert_eq!(e.code(), "room_mismatch");
    assert_eq!(b.resolve(None).unwrap(), "f1");
}

#[test]
fn constant_time_compare() {
    assert!(ct_eq(b"abc", b"abc"));
    assert!(!ct_eq(b"abc", b"abd"));
    assert!(!ct_eq(b"abc", b"abcd"));
    assert!(ct_eq(b"", b""));
}

#[test]
fn a_short_token_is_refused() {
    assert!(Token::new("").is_err());
    assert!(Token::new("0123456789abcde").is_err());
    assert!(Token::new("0123456789abcdef").is_ok());
}

#[test]
fn debug_output_hides_tokens_and_image_bytes() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    let b = begin("a", "w", "f");
    assert!(!format!("{b:?}").contains(&tok("a")));
    r.begin(0, b).unwrap();
    assert!(!format!("{r:?}").contains(&tok("a")));
    let body = png_of(1, 1, 40);
    let done = r.upload("w", "a", &tok("a"), 7, &body).unwrap();
    let dbg = format!("{done:?}");
    assert!(!dbg.contains("137") && !dbg.contains("[0,"), "{dbg}");
    assert!(dbg.contains("40"));
}

#[test]
fn the_same_request_id_may_be_pending_in_two_workspaces() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w1", "f", 1);
    capable(&mut r, "w2", "f", 2);
    r.begin(0, begin("a", "w1", "f")).unwrap();
    r.begin(0, begin("a", "w2", "f")).unwrap();
    assert!(r.upload("w1", "a", &tok("a"), 7, &png()).is_some());
    assert_eq!(r.pending(), 1);
}

#[test]
fn pending_and_client_counts_are_capped() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    r.begin(0, begin("b", "w", "f")).unwrap();
    assert_eq!(r.begin(0, begin("c", "w", "f")).unwrap_err().code(), "busy");
    capable(&mut r, "w", "f", 2);
    capable(&mut r, "w", "f", 3);
    r.begin(0, begin("c", "w", "f")).unwrap();
    r.begin(0, begin("d", "w", "f")).unwrap();
    assert_eq!(r.begin(0, begin("e", "w", "f")).unwrap_err().code(), "busy");
    let e = r.set_capabilities("w", "f", 4, &["capture".to_string()]);
    assert_eq!(e.unwrap_err().code(), "busy");
    // a known client may re-advertise at the cap
    capable(&mut r, "w", "f", 3);
}

#[test]
fn a_failure_message_is_sanitised_and_truncated() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    let msg = format!("bad\u{1b}[31m\n{}", "é".repeat(300));
    let done = r.failed("w", 1, "a", "render_failed", &msg).unwrap();
    let CaptureError::RenderFailed(m) = done.result.unwrap_err() else {
        panic!()
    };
    assert!(m.len() <= 200 && !m.chars().any(char::is_control), "{m}");
    assert!(m.starts_with("bad[31m"));
}

#[test]
fn binding_refuses_a_second_file_and_documents_no_rebind() {
    let mut b = Binding::default();
    b.bind("f1").unwrap();
    b.bind("f1").unwrap();
    assert_eq!(b.bind("f2").unwrap_err().code(), "room_mismatch");
    assert_eq!(b.resolve(None).unwrap(), "f1");
}

#[test]
fn disconnect_fails_every_pending_request_of_the_client() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    capable(&mut r, "w", "g", 2);
    r.begin(0, begin("a", "w", "f")).unwrap();
    r.begin(0, begin("b", "w", "f")).unwrap();
    r.begin(0, begin("c", "w", "g")).unwrap();
    assert_eq!(r.disconnect(1).len(), 2);
    assert_eq!(r.pending(), 1);
}

#[test]
fn a_withdrawn_capability_still_answers_and_expires() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    r.begin(0, begin("b", "w", "f")).unwrap();
    r.set_capabilities("w", "f", 1, &[]).unwrap();
    assert!(
        r.upload("w", "a", &tok("a"), 7, &png())
            .unwrap()
            .result
            .is_ok()
    );
    assert_eq!(r.expire(5000).len(), 1);
}

#[test]
fn a_token_is_bound_to_its_request() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    capable(&mut r, "w", "f", 2);
    r.begin(0, begin("a", "w", "f")).unwrap();
    r.begin(0, begin("b", "w", "f")).unwrap();
    assert!(r.upload("w", "b", &tok("a"), 7, &png()).is_none());
    assert_eq!(r.pending(), 2);
    assert!(r.upload("w", "b", &tok("b"), 7, &png()).is_some());
}

#[test]
fn an_oversize_upload_stopped_by_the_transport_fails_the_request() {
    let mut r = Relay::<u32>::new(cfg());
    capable(&mut r, "w", "f", 1);
    r.begin(0, begin("a", "w", "f")).unwrap();
    assert!(r.refuse_too_large("w", "a", "wrong").is_none());
    assert!(r.refuse_too_large("w2", "a", &tok("a")).is_none());
    let done = r.refuse_too_large("w", "a", &tok("a")).unwrap();
    assert_eq!(done.result.unwrap_err().code(), "too_large");
    assert_eq!(r.pending(), 0);
}
