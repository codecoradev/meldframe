//! The flow of frames on the live WebSocket: the wake-up, a frame published during the replay, a drain of
//! more than one batch, a slow reader, a replay cut at its limit, several readers of one file, a flooding
//! client.

mod common;

use common::*;
use futures_util::SinkExt;
use meldframe_http::Rate;
use serde_json::json;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

#[tokio::test]
async fn an_accepted_op_wakes_the_connection_without_waiting_for_the_poll_interval() {
    // The poll interval is 5 s: only the wake-up can deliver within 1 s.
    let e = env_with(live_cfg(8, 8), 8, false, |c| {
        c.live.live_poll = Duration::from_secs(5);
    })
    .await;
    let mut ws = connect(&e, ALPHA, None).await;
    assert_eq!(next(&mut ws).await["type"], "subscribed");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(rest(&e, ALPHA, 1, 1).await, 200);
    let m = next_within(&mut ws, Duration::from_secs(1))
        .await
        .expect("the wake-up delivers within 1 s");
    assert_eq!(
        (m["type"].as_str(), m["seq"].as_u64()),
        (Some("event"), Some(1))
    );
}

#[tokio::test]
async fn an_op_published_during_the_replay_arrives_once_and_after_the_replay() {
    let e = env(live_cfg(2, 16), 8, false).await;
    assert_eq!(rest(&e, ALPHA, 1, 5).await, 200);
    let _release = Release(e.hooks.clone());
    e.hooks.hold_subscribe.store(true, SeqCst);
    // The stream exists (and will queue) but the handshake has not answered: ops 6 and 7 land meanwhile.
    let (ws, ()) = tokio::join!(
        async { open(&e, ALPHA, "/v1/live/file_a?after=0").await.unwrap() },
        async {
            eventually("the subscribe is held", || e.hooks.held.load(SeqCst)).await;
            assert_eq!(rest(&e, ALPHA, 6, 7).await, 200);
            e.hooks.hold_subscribe.store(false, SeqCst);
        }
    );
    let mut ws = ws;
    assert_eq!(next(&mut ws).await["type"], "subscribed");
    assert_eq!(events(&mut ws, 7).await, [1, 2, 3, 4, 5, 6, 7]);
    assert!(
        next_within(&mut ws, Duration::from_millis(300))
            .await
            .is_none(),
        "no duplicate"
    );
}

#[tokio::test]
async fn a_drain_of_more_than_one_batch_delivers_every_frame_in_order() {
    let e = env(live_cfg(256, 256), 8, false).await;
    let mut ws = connect(&e, ALPHA, None).await;
    next(&mut ws).await;
    // A call takes at most MAX_BATCH_OPS (100) ops, so 150 frames come from two calls.
    assert_eq!(rest(&e, ALPHA, 1, 100).await, 200);
    assert_eq!(rest(&e, ALPHA, 101, 150).await, 200);
    let want: Vec<u64> = (1..=150).collect();
    assert_eq!(events(&mut ws, 150).await, want);
}

#[tokio::test]
async fn a_client_that_does_not_read_is_dropped_by_the_send_timeout_and_writers_go_on() {
    let e = env_with(live_cfg(64, 1024), 8, false, |c| {
        c.http.ws_send_timeout = Duration::from_millis(300);
    })
    .await;
    let mut slow = connect_small_buffer(&e, ALPHA).await;
    assert_eq!(next(&mut slow).await["type"], "subscribed");
    // Sizing assumption: the socket buffers on both sides hold less than about 19 MB (one batch is 40 ops of
    // 19,000 characters, 0.8 MB). So this writes batches until the server has dropped the reader, up to 25
    // batches (1,000 ops, still inside the queue of 1,024), and the bound below is generous; it does not
    // depend on the exact buffer sizes.
    let big = "x".repeat(19_000);
    let mut written = 0;
    for batch in 0..25 {
        if e.shared.with_server(|s| s.live_stream_count()) == 0 {
            break;
        }
        let ops = (1..=40).map(|i| add_text(batch * 40 + i, &big)).collect();
        assert_eq!(rest_ops(&e, ALPHA, ops).await, 200, "writers go on");
        written = (batch + 1) * 40;
    }
    eventually("the slow reader was dropped by the send timeout", || {
        e.shared.with_server(|s| s.live_stream_count()) == 0
    })
    .await;
    let mut fresh = connect(&e, ALPHA, Some(written - 2)).await;
    next(&mut fresh).await;
    assert_eq!(events(&mut fresh, 2).await, [written - 1, written]);
}

#[tokio::test]
async fn a_replay_cut_at_its_limit_ends_with_a_marker_and_the_client_resumes_on_the_socket() {
    let e = env(live_cfg(2, 8), 8, false).await;
    for batch in 0..11 {
        assert_eq!(
            rest(&e, ALPHA, batch * 100 + 1, batch * 100 + 100).await,
            200
        );
    }
    let mut ws = connect(&e, ALPHA, Some(0)).await;
    assert_eq!(next(&mut ws).await["type"], "subscribed");
    let first = events(&mut ws, 1024).await;
    assert_eq!((first[0], first[1023]), (1, 1024));
    let marker = next(&mut ws).await;
    assert_eq!(
        marker,
        json!({"type": "resync", "room": "file_a", "afterSeq": 1024, "latestSeq": 1100})
    );
    send(
        &mut ws,
        json!({"type": "subscribe", "id": "more", "room": "file_a", "sinceSeq": 1024}),
    )
    .await;
    assert_eq!(next(&mut ws).await["type"], "subscribed");
    let rest_of: Vec<u64> = (1025..=1100).collect();
    assert_eq!(events(&mut ws, 76).await, rest_of);
}

#[tokio::test]
async fn two_clients_of_one_file_receive_the_same_ops_in_the_same_order() {
    let e = env(live_cfg(8, 8), 8, false).await;
    let mut a = connect(&e, ALPHA, None).await;
    let mut b = connect(&e, ALPHA, None).await;
    next(&mut a).await;
    next(&mut b).await;
    assert_eq!(rest(&e, ALPHA, 1, 3).await, 200);
    assert_eq!(rest(&e, ALPHA, 4, 4).await, 200);
    assert_eq!(events(&mut a, 4).await, [1, 2, 3, 4]);
    assert_eq!(events(&mut b, 4).await, [1, 2, 3, 4]);
}

#[tokio::test]
async fn a_flooding_client_is_closed_and_other_principals_are_not_affected() {
    let e = env_with(live_cfg(8, 8), 8, false, |c| {
        c.http.per_principal = Rate {
            burst: 5,
            per_second: 1,
        };
        c.http.ws_max_limited = 3;
    })
    .await;
    let mut flood = connect(&e, ALPHA, None).await;
    let mut calm = connect(&e, BETA, None).await;
    next(&mut flood).await;
    next(&mut calm).await;
    for _ in 0..60 {
        let ping = tokio_tungstenite::tungstenite::Message::text(r#"{"type":"ping"}"#);
        if flood.send(ping).await.is_err() {
            break;
        }
    }
    assert!(is_closed(&mut flood).await, "the flood must end in a close");
    assert_eq!(rest(&e, BETA, 1, 1).await, 200);
    assert_eq!(events(&mut calm, 1).await, [1]);
    send(&mut calm, json!({"type": "ping"})).await;
    assert_eq!(next(&mut calm).await["type"], "pong");
}
