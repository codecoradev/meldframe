//! The live WebSocket under failure: a poll that outlives its timeout, a stuck server lock, `subscribe`
//! without a marker, a stream dropped before its socket registers, a failing resubscribe, and a ping that
//! the rate limit refused.

mod common;

use common::*;
use meldframe_http::Rate;
use serde_json::json;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

fn wide(window: usize, queue: usize) -> meldframe_server::Config {
    meldframe_server::Config {
        max_streams: 32,
        ..live_cfg(window, queue)
    }
}

#[tokio::test]
async fn a_poll_that_outlives_its_timeout_closes_the_socket_and_a_reconnect_gets_what_it_took() {
    let e = env_with(live_cfg(16, 16), 8, false, |c| {
        c.http.timeout = Duration::from_millis(300);
    })
    .await;
    let mut ws = connect(&e, ALPHA, None).await;
    assert_eq!(next(&mut ws).await["type"], "subscribed");
    assert_eq!(rest(&e, ALPHA, 1, 1).await, 200);
    assert_eq!(events(&mut ws, 1).await, [1]);
    // The next poll stalls past the timeout and then pops the frames of 2 and 3: they are lost to this
    // socket, so it must close rather than go on with a gap.
    e.hooks.stall_next_poll_ms.store(800, SeqCst);
    assert_eq!(rest(&e, ALPHA, 2, 3).await, 200);
    let end = tokio::time::Instant::now() + Duration::from_secs(5);
    while let Some(m) = next_within(
        &mut ws,
        end.saturating_duration_since(tokio::time::Instant::now()),
    )
    .await
    {
        assert_eq!(m["type"], "heartbeat", "no event may follow the gap: {m}");
    }
    assert!(
        is_closed(&mut ws).await,
        "the socket must close, not go on silently"
    );
    // Once the stalled job is over, reconnecting from the last number it saw gets exactly the missed frames
    // (a new socket's first poll would otherwise queue behind the stalled one and time out too).
    eventually("the stalled poll ends", || {
        e.hooks.running.load(SeqCst) == 0
    })
    .await;
    let mut again = connect(&e, ALPHA, Some(1)).await;
    assert_eq!(next(&mut again).await["type"], "subscribed");
    assert_eq!(events(&mut again, 2).await, [2, 3]);
    assert!(
        next_within(&mut again, Duration::from_millis(300))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn stuck_live_calls_stay_within_their_caps_per_principal_and_in_all() {
    // Per principal 2, in all 5: with two principals the per-principal cap binds first (2 + 2 < 5), so a
    // missing per-principal limit would show as a peak of 5 for one of them.
    let e = env_with(wide(8, 8), 32, false, |c| {
        c.http.timeout = Duration::from_millis(400);
        c.live.max_live_in_flight = 5;
        c.live.max_live_per_principal = 2;
    })
    .await;
    let mut sockets = Vec::new();
    for token in [ALPHA, BETA] {
        for _ in 0..10 {
            let mut ws = connect(&e, token, None).await;
            next(&mut ws).await;
            sockets.push(ws);
        }
    }
    let _release = Release(e.hooks.clone());
    // Every live call now waits inside the source, as it would behind a stuck server lock.
    e.hooks.stuck.store(true, SeqCst);
    // Long enough for the calls to time out (400 ms) and for every socket to try again many times.
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    // The server itself is not stuck here, so REST answers: this shows that REST does not wait for live
    // places, not that a stuck lock can be survived.
    let began = tokio::time::Instant::now();
    assert_eq!(rest(&e, ALPHA, 1, 1).await, 200);
    assert!(began.elapsed() < Duration::from_secs(1));
    let (alpha, beta, all) = (
        e.hooks.peak_of("ws_1"),
        e.hooks.peak_of("ws_2"),
        e.hooks.peak.load(SeqCst),
    );
    assert!(
        (1..=2).contains(&alpha),
        "alpha ran {alpha} live calls at once"
    );
    assert!(
        (1..=2).contains(&beta),
        "beta ran {beta} live calls at once"
    );
    assert!(all <= 4, "{all} live calls at once");
    e.hooks.unstick();
    eventually("the stuck jobs end", || e.hooks.running.load(SeqCst) == 0).await;
    let mut fresh = connect(&e, ALPHA, Some(0)).await;
    next(&mut fresh).await;
    assert_eq!(events(&mut fresh, 1).await, [1]);
}

#[tokio::test]
async fn subscribe_is_refused_before_a_resync_marker_and_replays_nothing() {
    let e = env(live_cfg(16, 16), 8, false).await;
    let mut ws = connect(&e, ALPHA, None).await;
    next(&mut ws).await;
    assert_eq!(rest(&e, ALPHA, 1, 3).await, 200);
    assert_eq!(events(&mut ws, 3).await, [1, 2, 3]);
    send(
        &mut ws,
        json!({"type": "subscribe", "id": "s1", "room": "file_a", "sinceSeq": 0}),
    )
    .await;
    let m = next(&mut ws).await;
    assert_eq!(
        (m["code"].as_str(), m["id"].as_str()),
        (Some("bad_request"), Some("s1")),
        "{m}"
    );
    send(
        &mut ws,
        json!({"type": "subscribe", "id": "s2", "room": "file_a"}),
    )
    .await;
    assert_eq!(next(&mut ws).await["code"], "bad_request");
    assert!(
        next_within(&mut ws, Duration::from_millis(300))
            .await
            .is_none(),
        "nothing was replayed"
    );
    send(&mut ws, json!({"type": "ping"})).await;
    assert_eq!(next(&mut ws).await["type"], "pong");
}

#[tokio::test]
async fn subscribe_after_a_marker_starts_at_the_marker_and_cannot_go_back_or_repeat() {
    for since in [None, Some(0)] {
        let e = env(live_cfg(16, 3), 8, false).await;
        let mut ws = connect(&e, ALPHA, None).await;
        next(&mut ws).await;
        assert_eq!(rest(&e, ALPHA, 1, 6).await, 200);
        assert_eq!(events(&mut ws, 3).await, [1, 2, 3]);
        assert_eq!(next(&mut ws).await["afterSeq"], 3);
        let mut ask = json!({"type": "subscribe", "id": "r1", "room": "file_a"});
        if let Some(s) = since {
            ask["sinceSeq"] = json!(s);
        }
        send(&mut ws, ask).await;
        assert_eq!(next(&mut ws).await["type"], "subscribed");
        // Whatever the client asked (nothing, or the start of the file), it starts at the marker.
        assert_eq!(events(&mut ws, 3).await, [4, 5, 6], "since {since:?}");
        // The marker is used up: a second subscribe needs a new one.
        send(
            &mut ws,
            json!({"type": "subscribe", "id": "r2", "room": "file_a", "sinceSeq": 0}),
        )
        .await;
        assert_eq!(next(&mut ws).await["code"], "bad_request");
    }
}

#[tokio::test]
async fn a_stream_the_timer_dropped_before_the_socket_registered_closes_with_an_error() {
    let live = meldframe_server::Config {
        heartbeat_ms: 500,
        dead_after_ms: 1_000,
        ..live_cfg(8, 8)
    };
    let e = env(live, 8, false).await;
    let _release = Release(e.hooks.clone());
    e.hooks.hold_subscribe.store(true, SeqCst);
    // The handshake waits inside `subscribe`; meanwhile the clock passes the dead time and the timer drops
    // the silent stream, which has no connection yet to tell.
    let (ws, ()) = tokio::join!(
        async { open(&e, ALPHA, "/v1/live/file_a").await.unwrap() },
        async {
            eventually("the subscribe is held", || e.hooks.held.load(SeqCst)).await;
            e.now.store(5_000, SeqCst);
            eventually("the stream is dropped", || {
                e.shared.with_server(|s| s.live_stream_count()) == 0
            })
            .await;
            e.hooks.hold_subscribe.store(false, SeqCst);
        }
    );
    let mut ws = ws;
    let m = next(&mut ws).await;
    assert_eq!(
        (
            m["type"].as_str(),
            m["code"].as_str(),
            m["retryable"].as_bool()
        ),
        (Some("error"), Some("unavailable"), Some(true)),
        "{m}"
    );
    assert!(is_closed(&mut ws).await, "no zombie socket");
}

#[tokio::test]
async fn a_failing_resubscribe_sends_a_fixed_error_frame_and_closes_without_leaking_detail() {
    let e = env(live_cfg(16, 3), 8, false).await;
    let mut ws = connect(&e, ALPHA, None).await;
    next(&mut ws).await;
    assert_eq!(rest(&e, ALPHA, 1, 6).await, 200);
    assert_eq!(events(&mut ws, 3).await, [1, 2, 3]);
    assert_eq!(next(&mut ws).await["type"], "resync");
    e.hooks.fail_resubscribe.store(true, SeqCst);
    send(
        &mut ws,
        json!({"type": "subscribe", "id": "r1", "room": "file_a", "sinceSeq": 3}),
    )
    .await;
    let m = next(&mut ws).await;
    assert_eq!(
        (m["type"].as_str(), m["code"].as_str(), m["id"].as_str()),
        (Some("error"), Some("unavailable"), Some("r1")),
        "{m}"
    );
    assert_eq!(m["retryable"], true);
    let text = m["message"].as_str().unwrap();
    assert!(
        !text.contains("secret") && !text.contains("Unavailable("),
        "no detail from the store: {text}"
    );
    assert!(is_closed(&mut ws).await);
}

#[tokio::test]
async fn a_ping_the_rate_limit_refused_still_counts_as_proof_of_life() {
    let live = meldframe_server::Config {
        heartbeat_ms: 500,
        dead_after_ms: 1_000,
        ..live_cfg(8, 8)
    };
    // One token: the handshake spends it, so every frame after it is limited.
    let e = env_with(live, 8, false, |c| {
        c.http.per_principal = Rate {
            burst: 1,
            per_second: 0,
        };
    })
    .await;
    let mut ws = connect(&e, ALPHA, None).await;
    next(&mut ws).await;
    e.now.store(1_600, SeqCst);
    assert_eq!(next(&mut ws).await["type"], "heartbeat");
    send(&mut ws, json!({"type": "ping"})).await;
    // The refused ping gets no reply; its `seen` call is what shows it was taken in.
    eventually("the ping's seen ran", || e.hooks.count("seen:end") >= 1).await;
    e.now.store(2_300, SeqCst);
    assert_eq!(
        next(&mut ws).await["type"],
        "heartbeat",
        "heard at 1600, so alive at 2300 (reaped only if silent since 1000)"
    );
}
