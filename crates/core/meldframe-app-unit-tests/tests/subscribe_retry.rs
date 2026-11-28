//! A refused `subscribe` on the live socket is answered by the app layer (audit finding T14, issue 324): after a marker or a
//! gap the app asks the shell to subscribe again, the server may refuse it, and the app decides whether to send it
//! again (after the wait the server named, else one second, three times at most), to wait for the marker, or to give the
//! connection up. The shell hands over the text of the `error` frame and keeps no count and no rule.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, Connection, Effect, Frame};
use meldframe_app_model::Wire;
use serde_json::{Value, json};

mod common;
use common::{Out, doc};

const T0: u64 = 1_000;

/// An app that is live on the socket, at sequence number `seq`.
fn live_at(seq: u64) -> App {
    let mut app = App::new("user:me", "c0", 1, doc(), seq).unwrap();
    app.dispatch(Action::Start { now_ms: T0 });
    app.dispatch(Action::Connected { now_ms: T0 });
    assert_eq!(app.state().connection, Connection::Live);
    app
}

fn live() -> App {
    live_at(0)
}

/// What the shell sends when the server answers the last subscribe with an `error` frame.
fn refused(now_ms: u64, frame: Value) -> Action {
    Action::Wire {
        wire: Wire::SubscribeRefused {
            now_ms,
            text: frame.to_string(),
        },
    }
}

fn error_frame(code: &str, retry_after_ms: Option<u64>) -> Value {
    let mut f =
        json!({"type": "error", "id": "s1", "message": "m", "retryable": true, "code": code});
    if let Some(ms) = retry_after_ms {
        f["retryAfterMs"] = json!(ms);
    }
    f
}

/// The server's marker: the stream stopped and waits for a subscribe.
fn marker(app: &mut App, now_ms: u64, after_seq: u64) -> Vec<Effect> {
    app.dispatch(Action::Frame {
        now_ms,
        frame: Frame::Resync {
            after_seq,
            latest_seq: after_seq + 3,
        },
    })
}

fn tick(app: &mut App, now_ms: u64) -> Vec<Effect> {
    app.dispatch(Action::Tick { now_ms })
}

fn subscribes(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Subscribe { after_seq } => Some(*after_seq),
            _ => None,
        })
        .collect()
}

fn wakes(fx: &[Effect]) -> Vec<u64> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::WakeAfter { after_ms } => Some(*after_ms),
            _ => None,
        })
        .collect()
}

fn reconnects(fx: &[Effect]) -> usize {
    fx.iter()
        .filter(|e| matches!(e, Effect::Reconnect { .. }))
        .count()
}

#[test]
fn a_subscribe_refused_as_rate_limited_is_sent_again_after_the_wait_three_times_then_the_app_goes_offline()
 {
    let mut app = live();
    assert_eq!(subscribes(&marker(&mut app, 2_000, 0)), [0]);
    let mut now = 2_000;
    for _ in 1..=3 {
        let fx = app.dispatch(refused(now, error_frame("rate_limited", Some(2_000))));
        assert_eq!(wakes(&fx), [2_000]);
        assert!(subscribes(&fx).is_empty() && reconnects(&fx) == 0);
        assert!(
            subscribes(&tick(&mut app, now + 1_999)).is_empty(),
            "not before the wait is over"
        );
        now += 2_000;
        assert_eq!(
            subscribes(&tick(&mut app, now)),
            [0],
            "after the wait, from the same number"
        );
        assert_eq!(app.state().connection, Connection::Live);
    }
    let fx = app.dispatch(refused(now, error_frame("rate_limited", Some(2_000))));
    assert_eq!(app.state().connection, Connection::Offline);
    assert_eq!(reconnects(&fx), 1);
    assert!(subscribes(&fx).is_empty() && wakes(&fx).is_empty());
}

#[test]
fn a_retry_goes_out_once() {
    let mut app = live();
    marker(&mut app, 2_000, 0);
    app.dispatch(refused(2_000, error_frame("unavailable", Some(500))));
    assert_eq!(subscribes(&tick(&mut app, 2_500)), [0]);
    assert!(subscribes(&tick(&mut app, 2_501)).is_empty());
    assert!(subscribes(&tick(&mut app, 9_000)).is_empty());
}

#[test]
fn unavailable_without_a_wait_is_sent_again_after_one_second() {
    let mut app = live();
    marker(&mut app, 2_000, 0);
    for frame in [
        error_frame("unavailable", None),
        // a null wait is none
        json!({"type": "error", "id": "s1", "message": "m", "retryable": true, "code": "unavailable", "retryAfterMs": null}),
    ] {
        let fx = app.dispatch(refused(5_000, frame));
        assert_eq!(wakes(&fx), [1_000]);
        assert_eq!(subscribes(&tick(&mut app, 6_000)), [0]);
        // a new marker starts the count again and cancels nothing that is left
        marker(&mut app, 7_000, 0);
    }
}

#[test]
fn bad_request_waits_for_the_marker_and_sends_nothing() {
    let mut app = live();
    marker(&mut app, 2_000, 0);
    for now in [2_100, 2_200, 2_300] {
        let fx = app.dispatch(refused(now, error_frame("bad_request", None)));
        assert!(fx.is_empty());
        assert_eq!(app.state().connection, Connection::Live);
    }
    assert!(subscribes(&tick(&mut app, 3_000)).is_empty());
    assert_eq!(app.state().connection, Connection::Live);
}

#[test]
fn any_other_code_gives_the_connection_up() {
    for code in [
        "forbidden",
        "not_found",
        "internal",
        "rate_limited_x",
        "RATE_LIMITED",
    ] {
        let mut app = live();
        marker(&mut app, 2_000, 0);
        let fx = app.dispatch(refused(2_100, error_frame(code, Some(100))));
        assert_eq!(app.state().connection, Connection::Offline, "{code}");
        assert_eq!(reconnects(&fx), 1, "{code}");
    }
}

#[test]
fn a_refusal_the_app_cannot_read_gives_the_connection_up() {
    let frames = [
        json!({"type": "error", "id": "s1", "message": "m", "retryable": true}),
        json!({"type": "error", "id": "s1", "code": 5}),
        json!({"type": "error", "id": "s1", "code": null}),
        json!({"type": "error", "id": "s1", "code": "x".repeat(65)}),
        json!({"type": "error", "id": "s1", "code": "rate_limited", "retryAfterMs": "2000"}),
        json!({"type": "error", "id": "s1", "code": "rate_limited", "retryAfterMs": -1}),
        json!({"type": "error", "id": "s1", "code": "rate_limited", "retryAfterMs": 1500.5}),
        json!({"type": "error", "id": "s1", "code": "rate_limited", "retryAfterMs": 9_007_199_254_740_992u64}),
    ];
    for frame in frames {
        let mut app = live();
        marker(&mut app, 2_000, 0);
        let fx = app.dispatch(refused(2_100, frame.clone()));
        assert_eq!(app.state().connection, Connection::Offline, "{frame}");
        assert_eq!(reconnects(&fx), 1, "{frame}");
    }
}

#[test]
fn a_refusal_is_read_with_the_fields_it_has_and_the_ones_the_app_does_not_know_are_ignored() {
    let mut app = live();
    marker(&mut app, 2_000, 0);
    let frame = json!({"type": "error", "id": "s9", "message": "m", "retryable": false, "code": "rate_limited", "retryAfterMs": 9_007_199_254_740_991u64, "extra": [1]});
    assert_eq!(wakes(&app.dispatch(refused(2_100, frame))), [30_000]);
}

#[test]
fn a_second_refusal_while_a_retry_waits_is_nothing() {
    let mut app = live();
    marker(&mut app, 2_000, 0);
    assert_eq!(
        wakes(&app.dispatch(refused(2_000, error_frame("unavailable", Some(500))))),
        [500]
    );
    // the same subscribe answered again, or the answer of a frame that crossed it: no second wait, no count, no effect
    for code in ["unavailable", "rate_limited", "forbidden"] {
        let fx = app.dispatch(refused(2_100, error_frame(code, Some(500))));
        assert!(fx.is_empty(), "{code}: {fx:?}");
        assert_eq!(app.state().connection, Connection::Live, "{code}");
    }
    assert!(subscribes(&tick(&mut app, 2_499)).is_empty());
    assert_eq!(
        subscribes(&tick(&mut app, 2_500)),
        [0],
        "exactly one subscribe goes out"
    );
    assert!(subscribes(&tick(&mut app, 9_000)).is_empty());
    // the refusals that were ignored did not count: two retries are left, then the app goes offline
    let mut now = 10_000;
    for _ in 0..2 {
        assert_eq!(
            wakes(&app.dispatch(refused(now, error_frame("unavailable", Some(100))))),
            [100]
        );
        now += 100;
        assert_eq!(subscribes(&tick(&mut app, now)), [0]);
    }
    let fx = app.dispatch(refused(now, error_frame("unavailable", Some(100))));
    assert_eq!(app.state().connection, Connection::Offline);
    assert_eq!(reconnects(&fx), 1);
}

#[test]
fn a_wait_the_server_names_is_cut_at_thirty_seconds() {
    // the reader takes a wait up to 2^53 - 1; the rule never waits longer than 30 s
    for (named, waited) in [
        (9_007_199_254_740_991u64, 30_000u64),
        (u64::from(u32::MAX), 30_000),
        (30_001, 30_000),
        (30_000, 30_000),
        (5_000, 5_000),
        (0, 0),
    ] {
        let mut app = live();
        marker(&mut app, 2_000, 0);
        let fx = app.dispatch(refused(2_000, error_frame("rate_limited", Some(named))));
        assert_eq!(wakes(&fx), [waited], "{named}");
        if waited > 0 {
            assert!(
                subscribes(&tick(&mut app, 2_000 + waited - 1)).is_empty(),
                "{named}: not before"
            );
        }
        assert_eq!(
            subscribes(&tick(&mut app, 2_000 + waited)),
            [0],
            "{named}: on time"
        );
    }
}

#[test]
fn only_a_refusal_that_answers_a_subscribe_that_is_out_counts() {
    let gone = |app: &mut App| {
        let fx = app.dispatch(refused(5_000, error_frame("rate_limited", Some(100))));
        assert!(fx.is_empty());
        assert_eq!(app.state().connection, Connection::Live);
    };
    // none was sent
    gone(&mut live());
    // the subscribe was answered: `subscribed` makes the connection live again
    let mut app = live();
    marker(&mut app, 2_000, 0);
    app.dispatch(Action::Connected { now_ms: 2_100 });
    gone(&mut app);
    // a frame was applied since: the subscribe did its work
    let mut app = live();
    marker(&mut app, 2_000, 0);
    app.dispatch(Action::Frame {
        now_ms: 2_100,
        frame: Frame::Event {
            seq: 1,
            actor: "agent:a".into(),
            key: None,
            command: meldframe_protocol::Command::RemoveNode { id: "t1".into() },
        },
    });
    gone(&mut app);
    // offline
    let mut app = live();
    marker(&mut app, 2_000, 0);
    app.dispatch(Action::Disconnected {});
    let fx = app.dispatch(refused(5_000, error_frame("rate_limited", Some(100))));
    assert!(fx.is_empty());
    assert_eq!(app.state().connection, Connection::Offline);
}

#[test]
fn a_new_marker_cancels_a_retry_that_waits_and_counts_again() {
    let mut app = live();
    marker(&mut app, 2_000, 0);
    app.dispatch(refused(2_000, error_frame("rate_limited", Some(5_000))));
    // a second marker: its subscribe goes at once, and the old wait goes with it
    assert_eq!(subscribes(&marker(&mut app, 3_000, 0)), [0]);
    assert!(
        subscribes(&tick(&mut app, 7_000)).is_empty(),
        "the old wait is cancelled"
    );
    // the count is from the new subscribe: three retries are left
    let mut now = 7_000;
    for _ in 1..=3 {
        app.dispatch(refused(now, error_frame("unavailable", Some(10))));
        now += 10;
        assert_eq!(subscribes(&tick(&mut app, now)), [0]);
    }
    app.dispatch(refused(now, error_frame("unavailable", Some(10))));
    assert_eq!(app.state().connection, Connection::Offline);
}

#[test]
fn the_count_starts_again_when_a_subscribe_has_worked() {
    let mut app = live();
    let mut now = 2_000;
    for round in 0..2 {
        marker(&mut app, now, 0);
        for _ in 1..=3 {
            app.dispatch(refused(now, error_frame("rate_limited", Some(10))));
            now += 10;
            assert_eq!(subscribes(&tick(&mut app, now)), [0], "round {round}");
        }
        app.dispatch(Action::Connected { now_ms: now });
        now += 100;
    }
    assert_eq!(app.state().connection, Connection::Live);
}

#[test]
fn a_retry_that_waits_is_forgotten_when_the_connection_drops() {
    let mut app = live();
    marker(&mut app, 2_000, 0);
    app.dispatch(refused(2_000, error_frame("rate_limited", Some(5_000))));
    app.dispatch(Action::Disconnected {});
    app.dispatch(Action::Reconnect { now_ms: 3_000 });
    app.dispatch(Action::Connected { now_ms: 3_100 });
    assert!(subscribes(&tick(&mut app, 9_000)).is_empty());
    assert_eq!(app.state().connection, Connection::Live);
}

#[test]
fn a_retry_asks_again_from_the_number_the_marker_named() {
    // The app is at 5, the marker says 3 (it must not skip ops the app has not got): every retry says 3.
    let mut app = live_at(5);
    assert_eq!(subscribes(&marker(&mut app, 2_000, 3)), [3]);
    app.dispatch(refused(2_000, error_frame("rate_limited", Some(10))));
    assert_eq!(subscribes(&tick(&mut app, 2_010)), [3]);
    app.dispatch(refused(2_010, error_frame("rate_limited", Some(10))));
    assert_eq!(subscribes(&tick(&mut app, 2_020)), [3]);
}

#[test]
fn a_refusal_is_a_frame_heard_from_the_server() {
    // The marker was the last frame at 41 s; the refusal at 60 s is the server speaking, so the 45 s limit counts from it.
    let mut app = live();
    marker(&mut app, 41_000, 0);
    app.dispatch(refused(60_000, error_frame("bad_request", None)));
    tick(&mut app, 100_000);
    assert_eq!(app.state().connection, Connection::Live);
    tick(&mut app, 105_001);
    assert_eq!(app.state().connection, Connection::Offline);
}

#[test]
fn what_the_shell_hands_over_is_read_as_a_frame_and_a_text_that_is_not_an_error_frame_is_nothing() {
    use meldframe_app::parts::answers::action_of;
    let wire = |text: &str| Wire::SubscribeRefused {
        now_ms: 7,
        text: text.into(),
    };
    let frame = |code: Option<&str>, retry_after_ms: Option<u64>| {
        Out(Some(Action::Frame {
            now_ms: 7,
            frame: Frame::SubscribeRefused {
                code: code.map(String::from),
                retry_after_ms,
            },
        }))
    };
    let read = |text: &str| Out(action_of(wire(text)));
    assert_eq!(
        read(&error_frame("rate_limited", Some(250)).to_string()),
        frame(Some("rate_limited"), Some(250))
    );
    assert_eq!(
        read(&error_frame("bad_request", None).to_string()),
        frame(Some("bad_request"), None)
    );
    assert_eq!(read(r#"{"type":"error"}"#), frame(None, None));
    for text in [
        "",
        "not json",
        "null",
        "[]",
        r#"{"type":"pong"}"#,
        r#"{"type":5}"#,
        r#"{"code":"rate_limited"}"#,
    ] {
        assert_eq!(read(text), Out(None), "{text}");
    }
}
