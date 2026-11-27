//! The limits and the clock of an agent's capture (finding T13 of the thin UI audit, decision 43): a pixel limit the request
//! asks for is never more than the export's, a wait is never longer than the app allows, the deadline of a request follows one
//! clock that never goes back, every frame and every tick ends a request that is out of time, and the secret the request
//! carries is not in a log.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use meldframe_app::PngOp;

fn page(w: u32, h: u32) -> Value {
    json!([{"id": "page", "type": "frame", "x": 0, "y": 0, "w": w, "h": h}])
}

fn tick(app: &mut App, now_ms: u64) -> Vec<Value> {
    json_of(&app.dispatch(Action::Tick { now_ms }))
}

fn the_drop(request: u32) -> Value {
    json!({"type": "finishRaster", "request": request, "deliver": false})
}

#[test]
fn a_pixel_limit_the_request_asks_for_is_never_more_than_the_export_limit() {
    let max_safe = (1u64 << 53) - 1;
    // 80,000 by 80,000 pixels: the request says anything goes, the app does not draw it.
    let mut app = app_of(page(20_000, 20_000));
    let mut greedy = ask(Some("page"), 4.0, 0);
    greedy["maxPixels"] = json!(max_safe);
    let fx = request(&mut app, 0, greedy);
    assert_eq!(failures(&fx), vec![failed("too_large")]);
    assert_eq!(fx.len(), 1, "nothing is drawn: {fx:?}");
    // 4096 by 4096 is the limit, as for an export; one pixel more is not drawn.
    for (w, h, drawn) in [(4096, 4096, true), (4097, 4096, false), (4096, 4097, false)] {
        let mut app = app_of(page(w, h));
        let mut v = ask(Some("page"), 1.0, 0);
        v["maxPixels"] = json!(max_safe);
        let fx = request(&mut app, 0, v);
        assert_eq!(fx[0]["type"] == "raster", drawn, "{w}x{h}: {fx:?}");
    }
    // A limit under that of the export still holds.
    let mut app = app_of(page(100, 100));
    let mut v = ask(Some("page"), 1.0, 0);
    v["maxPixels"] = json!(9_999);
    assert_eq!(
        failures(&request(&mut app, 0, v)),
        vec![failed("too_large")]
    );
    // The same limit holds for what the shell reports: a drawing over it is dropped, whatever the request said.
    let mut app = app_of(page(100, 100));
    let mut v = ask(Some("page"), 1.0, 0);
    v["maxPixels"] = json!(max_safe);
    request(&mut app, 0, v);
    let fx = drawn(&mut app, 1, 5000, 5000, 5000);
    assert_eq!(fx[0], the_drop(1));
    assert_eq!(failures(&fx), vec![failed("too_large")]);
}

#[test]
fn a_wait_is_at_most_a_minute_whatever_the_request_asks() {
    let mut app = app();
    let mut patient = ask(Some("page"), 1.0, 5);
    patient["timeoutMs"] = json!(10_000_000u64);
    assert_eq!(
        request(&mut app, 0, patient),
        vec![json!({"type": "wakeAfter", "afterMs": 60_000})]
    );
    assert!(failures(&tick(&mut app, 59_999)).is_empty());
    assert_eq!(
        failures(&tick(&mut app, 60_000)),
        vec![failed("stale_client")]
    );
}

#[test]
fn a_request_that_is_out_of_time_while_being_drawn_or_delivered_is_dropped_by_the_tick() {
    let mut app = app();
    let mut short = |id: &str| {
        let mut v = ask(Some("page"), 1.0, 0);
        v["requestId"] = json!(id);
        v["timeoutMs"] = json!(1_000);
        request(&mut app, 0, v)
    };
    assert_eq!(short("cap_1")[0]["request"], 1);
    assert_eq!(short("cap_2")[0]["request"], 2);
    // Request 1 is being drawn, request 2 has its bytes on their way to the upload.
    assert_eq!(
        drawn(&mut app, 2, 220, 120, 5000),
        vec![json!({"type": "finishRaster", "request": 2, "deliver": true})]
    );
    assert!(tick(&mut app, 999).is_empty());
    let fx = tick(&mut app, 1_000);
    for (id, request) in [("cap_1", 1), ("cap_2", 2)] {
        assert!(fx.contains(&the_drop(request)), "{fx:?}");
        let mut one = failed("render_failed");
        one["requestId"] = json!(id);
        assert!(failures(&fx).contains(&one), "{fx:?}");
    }
    // Answered once.
    assert!(drawn(&mut app, 1, 220, 120, 5000).is_empty());
    let later = tick(&mut app, 99_000);
    assert!(failures(&later).is_empty() && !later.contains(&the_drop(1)));
}

#[test]
fn a_frame_that_is_not_an_event_ends_a_request_that_is_out_of_time_and_nothing_is_delivered_after()
{
    let mut app = app();
    let mut short = ask(Some("page"), 1.0, 0);
    short["timeoutMs"] = json!(1_000);
    assert_eq!(request(&mut app, 0, short)[0]["type"], "raster");
    // No tick came: a heartbeat of the socket is the clock that moves.
    let fx = json_of(&app.dispatch(Action::Frame {
        now_ms: 1_000,
        frame: Frame::Heartbeat {},
    }));
    assert_eq!(fx[0], the_drop(1));
    assert_eq!(failures(&fx), vec![failed("render_failed")]);
    // The shell's report comes after the time was up: the request is over, nothing is delivered.
    assert!(drawn(&mut app, 1, 220, 120, 5000).is_empty());
    // A frame that comes before the time is up ends nothing.
    let mut short = ask(Some("page"), 1.0, 0);
    short["requestId"] = json!("cap_2");
    short["timeoutMs"] = json!(1_000);
    request(&mut app, 5_000, short);
    let fx = json_of(&app.dispatch(Action::Frame {
        now_ms: 5_999,
        frame: Frame::Heartbeat {},
    }));
    assert!(fx.is_empty(), "{fx:?}");
}

#[test]
fn a_request_for_the_file_is_timed_by_one_clock_that_never_goes_back() {
    let mut app = app();
    // The shell's clock reads 500 seconds for the first request and 1 second for the next: the deadline of the second is
    // 20 seconds after the latest the app has seen, not 20 seconds after a time that is already past.
    assert_eq!(
        request(&mut app, 500_000, ask(Some("page"), 1.0, 0))[0]["type"],
        "raster"
    );
    let mut second = ask(Some("page"), 1.0, 1);
    second["requestId"] = json!("cap_2");
    assert_eq!(
        request(&mut app, 1_000, second),
        vec![json!({"type": "wakeAfter", "afterMs": 20_000})]
    );
    let fx = event_at(
        &mut app,
        1_500,
        1,
        json!({"type": "updateNode", "id": "extra", "patch": {"w": 6}}),
    );
    assert!(failures(&fx).is_empty(), "{fx:?}");
    assert_eq!(
        fx.iter().filter(|e| e["type"] == "raster").count(),
        1,
        "{fx:?}"
    );
    // And it ends at the deadline on that clock.
    let mut third = ask(Some("page"), 1.0, 9);
    third["requestId"] = json!("cap_3");
    request(&mut app, 2_000, third);
    assert!(failures(&tick(&mut app, 519_999)).is_empty());
    let late = tick(&mut app, 520_000);
    let mut gone = failed("stale_client");
    gone["requestId"] = json!("cap_3");
    assert!(failures(&late).contains(&gone), "{late:?}");
}

#[test]
fn the_upload_token_is_in_no_debug_output_of_what_carries_it() {
    let mut app = app();
    let text = ask(Some("page"), 1.0, 0).to_string();
    let wire = Wire::Capture {
        now_ms: 0,
        text: text.clone().into(),
    };
    let action = Action::Wire { wire: wire.clone() };
    let fx = app.dispatch(action.clone());
    let op = serde_json::from_value::<PngOp>(json!({"type": "capture", "nowMs": 0, "ask": {
        "requestId": "cap_9", "scale": 1.0, "maxPixels": 16_777_216u64, "minSeq": 0, "uploadToken": TOKEN, "timeoutMs": 20_000}}))
    .unwrap();
    for shown in [
        format!("{fx:?}"),
        format!("{wire:?}"),
        format!("{action:?}"),
        format!("{op:?}"),
        format!("{:?}", Action::Png { op: op.clone() }),
    ] {
        assert!(!shown.contains(TOKEN), "{shown}");
        assert!(shown.contains("<redacted>"), "{shown}");
    }
    // The wire shape is what it was: the token and the text are plain strings.
    assert_eq!(
        serde_json::to_value(&fx[0]).unwrap()["to"]["uploadToken"],
        TOKEN
    );
    assert_eq!(
        serde_json::to_value(&wire).unwrap(),
        json!({"kind": "capture", "nowMs": 0, "text": text})
    );
    assert_eq!(
        serde_json::to_value(&op).unwrap()["ask"]["uploadToken"],
        TOKEN
    );
}
