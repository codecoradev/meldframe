//! The live WebSocket (`GET /v1/live/{file}?after={seq}`) against the real server and a real socket:
//! ops submitted over REST arrive in order with their numbers, a reconnect gets the missed frames, a
//! stalled client gets the resync marker and recovers, ephemeral frames are not replayed, the stream cap,
//! heartbeats and reaping by the timer loop (injected clock, and the real one), and workspace isolation.

mod common;

use common::*;
use futures_util::SinkExt;
use meldframe_host::Principal;
use meldframe_protocol::ServerMsg;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn an_op_submitted_over_rest_arrives_in_order_with_its_sequence_number() {
    let e = env(live_cfg(8, 8), 8, false).await;
    let mut ws = connect(&e, ALPHA, None).await;
    assert_eq!(next(&mut ws).await["type"], "subscribed");
    assert_eq!(rest(&e, ALPHA, 1, 2).await, 200);
    assert_eq!(rest(&e, ALPHA, 3, 3).await, 200);
    let first = next(&mut ws).await;
    assert_eq!(
        (
            first["type"].as_str(),
            first["seq"].as_u64(),
            first["actor"].as_str()
        ),
        (Some("event"), Some(1), Some("agent:alpha"))
    );
    assert_eq!(first["command"]["type"], "addNode");
    assert_eq!(
        first["key"], "k1",
        "the event carries the op's idempotency key"
    );
    assert_eq!(events(&mut ws, 2).await, [2, 3]);
    // The same key again writes nothing, so nothing is sent; the next new op is 4.
    assert_eq!(rest(&e, ALPHA, 3, 4).await, 200);
    assert_eq!(events(&mut ws, 1).await, [4]);
}

#[tokio::test]
async fn a_reconnect_with_the_last_number_gets_exactly_the_missed_frames_from_store_and_window() {
    let e = env(live_cfg(2, 8), 8, false).await;
    let mut a = connect(&e, ALPHA, None).await;
    assert_eq!(rest(&e, ALPHA, 1, 5).await, 200);
    next(&mut a).await; // subscribed
    assert_eq!(events(&mut a, 3).await, [1, 2, 3]);
    drop(a);
    assert_eq!(rest(&e, ALPHA, 6, 7).await, 200);
    let mut b = connect(&e, ALPHA, Some(3)).await;
    assert_eq!(next(&mut b).await["type"], "subscribed");
    assert_eq!(events(&mut b, 4).await, [4, 5, 6, 7]);
    assert!(
        next_within(&mut b, Duration::from_millis(300))
            .await
            .is_none()
    );
    // Caught up already: nothing is replayed.
    let mut c = connect(&e, ALPHA, Some(7)).await;
    next(&mut c).await;
    assert!(
        next_within(&mut c, Duration::from_millis(300))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn a_stalled_client_gets_the_resync_marker_recovers_and_the_writer_is_not_held_up() {
    let e = env(live_cfg(16, 3), 8, false).await;
    let mut slow = connect(&e, ALPHA, None).await;
    next(&mut slow).await;
    assert_eq!(
        rest(&e, ALPHA, 1, 6).await,
        200,
        "the room never waits for a client"
    );
    assert_eq!(events(&mut slow, 3).await, [1, 2, 3]);
    let marker = next(&mut slow).await;
    assert_eq!(
        marker,
        json!({"type": "resync", "room": "file_a", "afterSeq": 3, "latestSeq": 6})
    );
    assert!(
        next_within(&mut slow, Duration::from_millis(200))
            .await
            .is_none()
    );
    send(
        &mut slow,
        json!({"type": "subscribe", "id": "r1", "room": "file_a", "sinceSeq": 3}),
    )
    .await;
    assert_eq!(
        next(&mut slow).await,
        json!({"type": "subscribed", "id": "r1", "room": "file_a"})
    );
    assert_eq!(events(&mut slow, 3).await, [4, 5, 6]);
    assert_eq!(rest(&e, ALPHA, 7, 7).await, 200);
    assert_eq!(events(&mut slow, 1).await, [7]);
}

#[tokio::test]
async fn ephemeral_frames_have_no_number_and_are_not_replayed() {
    let e = env(live_cfg(8, 8), 8, false).await;
    let mut a = connect(&e, ALPHA, None).await;
    next(&mut a).await;
    let ask = ServerMsg::CaptureRequest {
        request_id: "c1".into(),
        room: "file_a".into(),
        node_id: None,
        scale: 1.0,
        max_pixels: 1000,
        min_seq: 0,
        upload_token: "t".into(),
        timeout_ms: 1000,
    };
    let who = Principal::new("ws_1", "agent:alpha");
    assert_eq!(e.shared.publish_ephemeral(&who, "file_a", None, ask), 1);
    let m = next(&mut a).await;
    assert_eq!(m["type"], "captureRequest");
    assert!(m.get("seq").is_none(), "{m}");
    assert_eq!(rest(&e, ALPHA, 1, 1).await, 200);
    assert_eq!(events(&mut a, 1).await, [1]);
    let mut b = connect(&e, ALPHA, Some(0)).await;
    next(&mut b).await;
    assert_eq!(events(&mut b, 1).await, [1]);
    assert!(
        next_within(&mut b, Duration::from_millis(300))
            .await
            .is_none(),
        "no capture request"
    );
}

#[tokio::test]
async fn streams_are_capped_and_a_closed_connection_frees_its_slot() {
    let e = env(live_cfg(8, 8), 1, false).await;
    let a = connect(&e, ALPHA, None).await;
    let err = open(&e, ALPHA, "/v1/live/file_a").await.unwrap_err();
    assert!(err.to_string().contains("503"), "{err}");
    drop(a);
    let end = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match open(&e, ALPHA, "/v1/live/file_a").await {
            Ok(_) => break,
            Err(_) if tokio::time::Instant::now() < end => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(err) => panic!("the slot was never freed: {err}"),
        }
    }
}

#[tokio::test]
async fn the_timer_loop_sends_heartbeats_and_reaps_silent_clients_with_an_injected_clock() {
    let live = meldframe_server::Config {
        heartbeat_ms: 500,
        dead_after_ms: 1_000,
        ..live_cfg(8, 8)
    };
    let e = env(live, 2, false).await;
    let mut quiet = connect(&e, ALPHA, None).await;
    let mut chatty = connect(&e, ALPHA, None).await;
    next(&mut quiet).await;
    next(&mut chatty).await;
    assert!(
        open(&e, ALPHA, "/v1/live/file_a").await.is_err(),
        "the cap of 2 is full"
    );
    e.now.store(1_600, Ordering::SeqCst);
    assert_eq!(next(&mut quiet).await["type"], "heartbeat");
    assert_eq!(next(&mut chatty).await["type"], "heartbeat");
    send(&mut chatty, json!({"type": "ping"})).await;
    assert_eq!(next_data(&mut chatty).await["type"], "pong");
    e.now.store(2_300, Ordering::SeqCst);
    assert!(
        is_closed(&mut quiet).await,
        "silent since 1000: reaped at 2300"
    );
    assert_eq!(
        next(&mut chatty).await["type"],
        "heartbeat",
        "pinged at 1600: still alive"
    );
    // The reaped client's slot is free again.
    let end = tokio::time::Instant::now() + Duration::from_secs(5);
    while open(&e, ALPHA, "/v1/live/file_a").await.is_err() {
        assert!(
            tokio::time::Instant::now() < end,
            "the reaped slot was never freed"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn heartbeats_and_reaping_work_against_the_real_clock() {
    let live = meldframe_server::Config {
        heartbeat_ms: 50,
        dead_after_ms: 3_000,
        ..live_cfg(8, 8)
    };
    let e = env(live, 8, true).await;
    let mut quiet = connect(&e, ALPHA, None).await;
    let mut chatty = connect(&e, ALPHA, None).await;
    assert_eq!(next(&mut quiet).await["type"], "subscribed");
    assert_eq!(next(&mut quiet).await["type"], "heartbeat");
    // The chatty client pings every 100 ms through more than the dead time (3 s) and stays, with a wide
    // margin for a slow machine; the quiet one is reaped. The injected-clock test above checks the exact
    // thresholds: this one only shows that the real clock drives the same loop.
    for _ in 0..35 {
        send(&mut chatty, json!({"type": "ping"})).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(is_closed(&mut quiet).await);
    send(&mut chatty, json!({"type": "ping"})).await;
    loop {
        if next(&mut chatty).await["type"] == "pong" {
            break;
        }
    }
}

#[tokio::test]
async fn a_client_of_another_workspace_cannot_see_this_workspaces_file() {
    let e = env(live_cfg(8, 8), 8, false).await;
    let mut alpha = connect(&e, ALPHA, None).await;
    let mut beta = connect(&e, BETA, None).await;
    next(&mut alpha).await;
    next(&mut beta).await;
    assert_eq!(rest(&e, ALPHA, 1, 2).await, 200);
    assert_eq!(events(&mut alpha, 2).await, [1, 2]);
    assert!(
        next_within(&mut beta, Duration::from_millis(300))
            .await
            .is_none(),
        "the same file name in another workspace is another file"
    );
    assert_eq!(rest(&e, BETA, 1, 1).await, 200);
    assert_eq!(events(&mut beta, 1).await, [1]);
    assert!(
        next_within(&mut alpha, Duration::from_millis(200))
            .await
            .is_none()
    );
    // Nor can it start from alpha's numbers: its file has only one op.
    let mut ahead = connect(&e, BETA, Some(2)).await;
    next(&mut ahead).await;
    let m = next(&mut ahead).await;
    assert_eq!(m["type"], "resync", "{m}");
}

#[tokio::test]
async fn the_handshake_is_refused_without_a_token_with_a_bad_file_or_a_bad_cursor() {
    let e = env(live_cfg(8, 8), 8, false).await;
    let plain = tokio_tungstenite::connect_async(format!("ws://{}/v1/live/file_a", e.addr)).await;
    assert!(plain.unwrap_err().to_string().contains("401"));
    let bad = open(&e, "nope-nope-nope-nope", "/v1/live/file_a").await;
    assert!(bad.unwrap_err().to_string().contains("401"));
    let slash = open(&e, ALPHA, "/v1/live/a%2Fb").await;
    assert!(slash.unwrap_err().to_string().contains("400"));
    let cursor = open(&e, ALPHA, "/v1/live/file_a?after=abc").await;
    assert!(cursor.unwrap_err().to_string().contains("400"));
}

#[tokio::test]
async fn a_live_socket_answers_other_client_messages_with_an_error_and_stays_open() {
    let e = env(live_cfg(8, 8), 8, false).await;
    let mut ws = connect(&e, ALPHA, None).await;
    next(&mut ws).await;
    send(&mut ws, json!({"type": "unsubscribe", "room": "other"})).await;
    let m = next(&mut ws).await;
    assert_eq!(
        (m["type"].as_str(), m["code"].as_str()),
        (Some("error"), Some("bad_request")),
        "{m}"
    );
    ws.send(Message::text("not json")).await.unwrap();
    assert_eq!(next(&mut ws).await["code"], "bad_request");
    send(
        &mut ws,
        json!({"type": "subscribe", "id": "x", "room": "other", "sinceSeq": 0}),
    )
    .await;
    let m = next(&mut ws).await;
    assert_eq!(
        (m["code"].as_str(), m["id"].as_str()),
        (Some("bad_request"), Some("x")),
        "{m}"
    );
    send(&mut ws, json!({"type": "ping"})).await;
    assert_eq!(next(&mut ws).await["type"], "pong");
}
