//! The places of live calls and what a client is charged: pings while the live places are taken, a `seen`
//! with no place, a poll with no place, the clamp of `live_poll`, and a `subscribe` that finds no place.
//!
//! "Z" is a principal of the other workspace (BETA) whose calls are stuck on purpose to hold a place; "Y" is
//! the principal under test (ALPHA).

#[path = "../../meldframe-live-tests/tests/common/mod.rs"]
mod common;

use common::*;
use meldframe_http::Rate;
use serde_json::json;
use std::time::Duration;

fn beating() -> meldframe_server::Config {
    meldframe_server::Config {
        heartbeat_ms: 500,
        dead_after_ms: 1_000,
        ..live_cfg(8, 8)
    }
}

/// Live and `seen` places of one each, so one stuck call of Z takes the only one.
fn one_place(c: &mut Cfg) {
    c.live.max_live_in_flight = 1;
    c.live.max_live_per_principal = 1;
    c.live.max_seen_in_flight = 1;
    c.live.max_seen_per_principal = 1;
    // A stuck call must not time out (and close its socket) during the test.
    c.http.timeout = Duration::from_secs(30);
}

#[tokio::test]
async fn pings_keep_a_stream_alive_while_every_live_place_is_taken() {
    let e = env_with(beating(), 8, false, one_place).await;
    let _release = Release(e.hooks.clone());
    let mut z = connect(&e, BETA, None).await;
    let mut y = connect(&e, ALPHA, None).await;
    next(&mut z).await;
    next(&mut y).await;
    // Z's polls wait inside the source and hold the only live place.
    e.hooks.stick("ws_2", "poll");
    eventually("Z holds the live place", || {
        e.hooks.waiting_of("ws_2", "poll") >= 1
    })
    .await;
    e.now.store(1_800, std::sync::atomic::Ordering::SeqCst);
    send(&mut y, json!({"type": "ping"})).await;
    assert_eq!(next_data(&mut y).await["type"], "pong");
    // Past the dead time of the last number the server had before the ping (1000).
    e.now.store(2_500, std::sync::atomic::Ordering::SeqCst);
    e.hooks.unstick();
    assert_eq!(
        next(&mut y).await["type"],
        "heartbeat",
        "heard at 1800, so alive at 2500: the ping did not need a live place"
    );
}

#[tokio::test]
async fn a_seen_with_no_place_is_done_at_the_next_look_so_the_stream_is_not_reaped() {
    let e = env_with(beating(), 8, false, one_place).await;
    let _release = Release(e.hooks.clone());
    let mut z = connect(&e, BETA, None).await;
    let mut y = connect(&e, ALPHA, None).await;
    next(&mut z).await;
    next(&mut y).await;
    e.now.store(1_800, std::sync::atomic::Ordering::SeqCst);
    // Z's `seen` waits inside the source and holds the only place for `seen` calls.
    e.hooks.stick("ws_2", "seen");
    send(&mut z, json!({"type": "ping"})).await;
    eventually("Z's seen holds its place", || {
        e.hooks.count("seen:start") >= 1
    })
    .await;
    // Y's `seen` finds no place; the ping is still answered, and the `seen` is owed.
    send(&mut y, json!({"type": "ping"})).await;
    assert_eq!(next_data(&mut y).await["type"], "pong");
    e.hooks.unstick();
    eventually("Z's seen and Y's owed seen both ran", || {
        e.hooks.count("seen:end") >= 2
    })
    .await;
    e.now.store(2_500, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        next(&mut y).await["type"],
        "heartbeat",
        "the owed seen was recorded at 1800: alive at 2500"
    );
}

#[tokio::test]
async fn a_poll_with_no_place_is_tried_again_and_delivers_when_a_place_frees() {
    let e = env_with(live_cfg(8, 8), 8, false, one_place).await;
    let _release = Release(e.hooks.clone());
    let mut z = connect(&e, BETA, None).await;
    let mut y = connect(&e, ALPHA, None).await;
    next(&mut z).await;
    next(&mut y).await;
    e.hooks.stick("ws_2", "poll");
    eventually("Z holds the live place", || {
        e.hooks.waiting_of("ws_2", "poll") >= 1
    })
    .await;
    assert_eq!(rest(&e, ALPHA, 1, 1).await, 200);
    assert!(
        next_within(&mut y, Duration::from_millis(400))
            .await
            .is_none(),
        "no place to poll in: nothing is delivered, and nothing is lost"
    );
    e.hooks.unstick();
    assert_eq!(events(&mut y, 1).await, [1]);
}

#[tokio::test]
async fn live_poll_is_clamped_to_the_heartbeat_interval() {
    // A 5 s poll interval would send the first heartbeat after 5 s; the clamp makes it 200 ms.
    let live = meldframe_server::Config {
        heartbeat_ms: 200,
        ..live_cfg(8, 8)
    };
    let e = env_with(live, 8, true, |c| c.live.live_poll = Duration::from_secs(5)).await;
    let mut ws = connect(&e, ALPHA, None).await;
    assert_eq!(next(&mut ws).await["type"], "subscribed");
    let m = next_within(&mut ws, Duration::from_millis(1_500))
        .await
        .expect("a heartbeat within 1.5 s");
    assert_eq!(m["type"], "heartbeat");
}

/// A principal that has seen a marker on file_a and is about to subscribe: `rest` wrote 1..=6 with a queue
/// of 3, so the marker says `afterSeq` 3.
async fn at_marker(e: &Env) -> Ws {
    let mut y = connect(e, ALPHA, None).await;
    next(&mut y).await;
    assert_eq!(rest(e, ALPHA, 1, 6).await, 200);
    assert_eq!(events(&mut y, 3).await, [1, 2, 3]);
    assert_eq!(next(&mut y).await["afterSeq"], 3);
    y
}

#[tokio::test]
async fn a_subscribe_that_finds_no_place_costs_one_frame_not_four_and_says_when_to_retry() {
    // Budget of 20: connect 1, write 1, five busy subscribes 5 (1 each) = 7. Charged 4 each they would need
    // 22; after the places free, a retry that finds one costs 4 and one that does not costs 1.
    let e = env_with(live_cfg(16, 3), 8, false, |c| {
        one_place(c);
        c.http.per_principal = Rate {
            burst: 20,
            per_second: 0,
        };
    })
    .await;
    let _release = Release(e.hooks.clone());
    let mut y = at_marker(&e).await;
    let mut z = connect(&e, BETA, None).await;
    next(&mut z).await;
    e.hooks.stick("ws_2", "poll");
    // Wait for a poll of Z that is held, not for any call of Z: a poll of Z that started before the stick returns at
    // once and frees the place, and the next one comes up to `live_poll` later, so a subscribe sent in that gap
    // could find the place free (issue 236).
    eventually("Z holds the live place", || {
        e.hooks.waiting_of("ws_2", "poll") >= 1
    })
    .await;
    for n in 0..5 {
        send(
            &mut y,
            json!({"type": "subscribe", "id": format!("b{n}"), "room": "file_a"}),
        )
        .await;
        let m = next(&mut y).await;
        assert_eq!(
            (m["code"].as_str(), m["retryable"].as_bool()),
            (Some("unavailable"), Some(true)),
            "busy subscribe {n}: {m}"
        );
        assert!(m["retryAfterMs"].as_u64().is_some(), "when to retry: {m}");
    }
    e.hooks.unstick();
    eventually("Z's stuck poll ends", || e.hooks.running_of("ws_2") == 0).await;
    // Z's and Y's own short polls can still hold the single place for a moment: retry as a client would.
    for attempt in 0..6 {
        send(
            &mut y,
            json!({"type": "subscribe", "id": "ok", "room": "file_a"}),
        )
        .await;
        let m = next(&mut y).await;
        if m["type"] == "subscribed" {
            assert_eq!(events(&mut y, 3).await, [4, 5, 6]);
            return;
        }
        assert_eq!(m["code"], "unavailable", "attempt {attempt}: {m}");
    }
    panic!("no subscribe found a place");
}

#[tokio::test]
async fn a_subscribe_the_budget_cannot_pay_gets_rate_limited_with_a_wait() {
    // Connect 1 and write 1 leave 2 of 4; the subscribe's own frame takes 1, its extra 3 do not fit.
    let e = env_with(live_cfg(16, 3), 8, false, |c| {
        c.http.per_principal = Rate {
            burst: 4,
            per_second: 0,
        };
    })
    .await;
    let mut y = at_marker(&e).await;
    send(
        &mut y,
        json!({"type": "subscribe", "id": "dear", "room": "file_a"}),
    )
    .await;
    let m = next(&mut y).await;
    assert_eq!(
        (
            m["code"].as_str(),
            m["id"].as_str(),
            m["retryable"].as_bool()
        ),
        (Some("rate_limited"), Some("dear"), Some(true)),
        "{m}"
    );
    assert!(m["retryAfterMs"].as_u64().is_some_and(|ms| ms > 0), "{m}");
}
