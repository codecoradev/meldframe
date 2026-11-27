//! An agent's request for an image of a node (finding T13 of the thin UI audit, decision 43): the app layer reads the
//! `captureRequest` frame (`Wire::Capture`), refuses at once what cannot be drawn, waits for the file to reach `min_seq`
//! until the timeout, plans again on what is there when it ends, checks what the shell reports against the plan and the
//! limits, and answers every request once, in the upload or in `Effect::FailCapture`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, Effect, Frame, Wire};
use meldframe_protocol::{Command, Node};
use serde_json::{Value, json};

const TOKEN: &str = "0123456789abcdef0123";

fn app() -> App {
    app_of(json!([
        {"id": "page", "type": "frame", "x": 100, "y": 50, "w": 200, "h": 100,
         "shadows": [{"x": 10, "y": 6, "blur": 8, "spread": 2, "color": "#000000"}]},
        {"id": "flat", "type": "rect", "x": 0, "y": 0, "w": 0, "h": 10},
        {"id": "extra", "type": "rect", "x": 0, "y": 0, "w": 5, "h": 5},
    ]))
}

/// An app whose file is the nodes of `list`.
fn app_of(list: Value) -> App {
    let nodes: Vec<Node> = serde_json::from_value(list).unwrap();
    App::new("user:me", "c0", 1, nodes, 0).unwrap()
}

fn ask(node: Option<&str>, scale: f64, min_seq: u64) -> Value {
    let mut v = json!({"type": "captureRequest", "requestId": "cap_1", "room": "file_1", "scale": scale,
        "maxPixels": 16_777_216u64, "minSeq": min_seq, "uploadToken": TOKEN, "timeoutMs": 20_000});
    if let Some(n) = node {
        v["nodeId"] = json!(n);
    }
    v
}

fn request(app: &mut App, now_ms: u64, frame: Value) -> Vec<Value> {
    let wire = Wire::Capture {
        now_ms,
        text: frame.to_string().into(),
    };
    json_of(&app.dispatch(Action::Wire { wire }))
}

fn json_of(fx: &[Effect]) -> Vec<Value> {
    fx.iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect()
}

fn failed(code: &str) -> Value {
    json!({"type": "failCapture", "requestId": "cap_1", "code": code})
}

/// The failure effects, with the sentence left out.
fn failures(fx: &[Value]) -> Vec<Value> {
    fx.iter()
        .filter(|e| e["type"] == "failCapture")
        .map(|e| json!({"type": e["type"], "requestId": e["requestId"], "code": e["code"]}))
        .collect()
}

fn drawn(app: &mut App, request: u32, w: u32, h: u32, bytes: u64) -> Vec<Value> {
    let body = json!({"width": w, "height": h, "bytes": bytes}).to_string();
    json_of(&app.dispatch(Action::Wire {
        wire: Wire::Drawn {
            request,
            status: 200,
            body,
        },
    }))
}

fn remote(app: &mut App, seq: u64, command: Value) {
    let command: Command = serde_json::from_value(command).unwrap();
    app.dispatch(Action::Frame {
        now_ms: 0,
        frame: Frame::Event {
            seq,
            actor: "agent:other".into(),
            key: Some(format!("k{seq}")),
            command,
        },
    });
}

#[path = "capture_request/limits.rs"]
mod limits;

#[test]
fn a_request_for_a_node_asks_the_shell_to_draw_it_with_its_shadow_and_upload_it() {
    let mut app = app();
    let fx = request(&mut app, 0, ask(Some("page"), 2.0, 0));
    // The shadow reaches 0 left, 4 above, 20 right and 16 below.
    assert_eq!(
        fx,
        vec![json!({"type": "raster", "request": 1,
            "rect": {"x": 100.0, "y": 46.0, "w": 220.0, "h": 120.0}, "scale": 2.0, "width": 440, "height": 240,
            "to": {"type": "upload", "requestId": "cap_1", "uploadToken": TOKEN, "seq": 0}})]
    );
}

#[test]
fn a_request_with_no_node_is_the_whole_page() {
    let mut app = app();
    let fx = request(&mut app, 0, ask(None, 1.0, 0));
    assert_eq!(
        fx[0]["rect"],
        json!({"x": 0.0, "y": 0.0, "w": 320.0, "h": 166.0})
    );
}

#[test]
fn what_cannot_be_drawn_is_refused_at_once_and_nothing_is_drawn() {
    let mut app = app();
    for scale in [0.0, -1.0, 4.5, 1e9] {
        let fx = request(&mut app, 0, ask(Some("page"), scale, 0));
        assert_eq!(failures(&fx), vec![failed("invalid_scale")], "{scale}");
        assert_eq!(fx.len(), 1);
    }
    // The server's range is above 0 and at most 4: a small scale is an image.
    assert_eq!(
        request(&mut app, 0, ask(Some("page"), 0.25, 0))[0]["type"],
        "raster"
    );
    for node in ["gone", "flat"] {
        let mut app = self::app();
        let fx = request(&mut app, 0, ask(Some(node), 1.0, 0));
        assert_eq!(failures(&fx), vec![failed("node_not_found")], "{node}");
    }
    let mut app = self::app();
    let mut over = ask(Some("page"), 1.0, 0);
    over["maxPixels"] = json!(1000);
    assert_eq!(
        failures(&request(&mut app, 0, over)),
        vec![failed("too_large")]
    );
}

#[test]
fn a_request_waits_for_the_file_to_reach_min_seq_and_plans_again_on_what_is_there() {
    let mut app = app();
    let fx = request(&mut app, 1000, ask(Some("page"), 1.0, 2));
    assert_eq!(fx, vec![json!({"type": "wakeAfter", "afterMs": 20_000})]);
    remote(&mut app, 1, json!({"type": "removeNode", "id": "extra"}));
    // The agent's own edit changes the node while the request waits: the image is of what is there when the wait ends.
    let patch = json!({"type": "updateNode", "id": "page", "patch": {"w": 300}});
    let fx = json_of(&app.dispatch(Action::Frame {
        now_ms: 0,
        frame: Frame::Event {
            seq: 2,
            actor: "agent:other".into(),
            key: Some("k2".into()),
            command: serde_json::from_value(patch).unwrap(),
        },
    }));
    let raster: Vec<_> = fx.iter().filter(|e| e["type"] == "raster").collect();
    assert_eq!(raster.len(), 1, "{fx:?}");
    assert_eq!(raster[0]["rect"]["w"], 320.0);
    assert_eq!(raster[0]["to"]["seq"], 2);
}

#[test]
fn a_request_the_file_never_reaches_is_stale_client_at_the_timeout_and_not_drawn_after() {
    let mut app = app();
    request(&mut app, 1000, ask(Some("page"), 1.0, 5));
    let early = app.dispatch(Action::Tick { now_ms: 20_999 });
    assert!(failures(&json_of(&early)).is_empty());
    let late = json_of(&app.dispatch(Action::Tick { now_ms: 21_000 }));
    assert_eq!(failures(&late), vec![failed("stale_client")]);
    // Answered once: a later tick says nothing, and the file reaching the number draws nothing.
    assert!(json_of(&app.dispatch(Action::Tick { now_ms: 99_000 })).is_empty());
    for seq in 1..=5 {
        remote(
            &mut app,
            seq,
            json!({"type": "updateNode", "id": "extra", "patch": {"w": seq}}),
        );
    }
}

#[test]
fn a_node_deleted_while_the_request_waits_is_node_not_found_when_the_wait_ends() {
    let mut app = app();
    request(&mut app, 0, ask(Some("page"), 1.0, 2));
    remote(&mut app, 1, json!({"type": "removeNode", "id": "page"}));
    let fx = json_of(&app.dispatch(Action::Frame {
        now_ms: 0,
        frame: Frame::Event {
            seq: 2,
            actor: "agent:other".into(),
            key: Some("k2".into()),
            command: serde_json::from_value(json!({"type": "removeNode", "id": "extra"})).unwrap(),
        },
    }));
    assert_eq!(failures(&fx), vec![failed("node_not_found")]);
    assert!(fx.iter().all(|e| e["type"] != "raster"));
}

#[test]
fn the_same_request_twice_is_one_request() {
    let mut app = app();
    assert_eq!(request(&mut app, 0, ask(Some("page"), 1.0, 0)).len(), 1);
    assert!(request(&mut app, 0, ask(Some("page"), 1.0, 0)).is_empty());
}

#[test]
fn an_image_over_four_mib_is_refused_as_data_and_dropped_before_any_upload() {
    let mut app = app();
    request(&mut app, 0, ask(Some("page"), 1.0, 0));
    let fx = drawn(&mut app, 1, 220, 120, (4 << 20) + 1);
    assert_eq!(
        fx[0],
        json!({"type": "finishRaster", "request": 1, "deliver": false})
    );
    assert_eq!(failures(&fx), vec![failed("too_large")]);
    // At the limit it goes on.
    let mut app = self::app();
    request(&mut app, 0, ask(Some("page"), 1.0, 0));
    assert_eq!(
        drawn(&mut app, 1, 220, 120, 4 << 20),
        vec![json!({"type": "finishRaster", "request": 1, "deliver": true})]
    );
}

#[test]
fn a_node_gone_before_the_bytes_came_back_is_answered_once() {
    let mut app = app();
    request(&mut app, 0, ask(Some("page"), 1.0, 0));
    app.dispatch(Action::Delete {
        ids: vec!["page".into()],
    });
    let fx = drawn(&mut app, 1, 220, 120, 5000);
    assert_eq!(failures(&fx), vec![failed("node_not_found")]);
    assert_eq!(
        fx[0],
        json!({"type": "finishRaster", "request": 1, "deliver": false})
    );
    assert!(
        drawn(&mut app, 1, 220, 120, 5000).is_empty(),
        "the second report finds no request"
    );
}

#[test]
fn a_node_resized_before_the_bytes_came_back_is_not_uploaded() {
    let mut app = app();
    request(&mut app, 0, ask(Some("page"), 1.0, 0));
    app.dispatch(Action::Resize {
        id: "page".into(),
        w: 300.0,
        h: 100.0,
        x: None,
        y: None,
    });
    assert_eq!(
        failures(&drawn(&mut app, 1, 220, 120, 5000)),
        vec![failed("node_changed")]
    );
}

#[test]
fn the_upload_and_the_drawing_report_their_failures_to_the_agent() {
    let mut app = app();
    request(&mut app, 0, ask(Some("page"), 1.0, 0));
    drawn(&mut app, 1, 220, 120, 5000);
    let wire = Wire::Delivered {
        request: 1,
        status: 413,
        body: r#"{"error":{"code":"too_large"}}"#.into(),
    };
    let fx = json_of(&app.dispatch(Action::Wire { wire }));
    assert_eq!(failures(&fx), vec![failed("too_large")]);
    let mut app = self::app();
    request(&mut app, 0, ask(Some("page"), 1.0, 0));
    let wire = Wire::Drawn {
        request: 1,
        status: 500,
        body: String::new(),
    };
    assert_eq!(
        failures(&json_of(&app.dispatch(Action::Wire { wire }))),
        vec![failed("render_failed")]
    );
    // A delivery that went through says nothing.
    let mut app = self::app();
    request(&mut app, 0, ask(Some("page"), 1.0, 0));
    drawn(&mut app, 1, 220, 120, 5000);
    let wire = Wire::Delivered {
        request: 1,
        status: 200,
        body: String::new(),
    };
    assert!(app.dispatch(Action::Wire { wire }).is_empty());
}

#[test]
fn a_frame_that_is_not_a_request_or_has_an_unsafe_id_is_nothing() {
    let mut app = app();
    let mut bad = vec![
        json!({"type": "heartbeat"}),
        json!("text"),
        json!({"type": "captureRequest"}),
    ];
    for (field, value) in [
        ("requestId", json!("cap/../x")),
        ("requestId", json!("")),
        ("requestId", json!("a".repeat(129))),
        ("uploadToken", json!("tok en")),
        ("uploadToken", json!("t\u{202E}ok")),
        ("scale", json!("2")),
        ("minSeq", json!(1u64 << 60)),
        ("timeoutMs", json!(-1)),
    ] {
        let mut v = ask(Some("page"), 1.0, 0);
        v[field] = value;
        bad.push(v);
    }
    for frame in bad {
        assert!(request(&mut app, 0, frame.clone()).is_empty(), "{frame}");
    }
    // A field the app does not know is ignored: the server may add one.
    let mut extra = ask(Some("page"), 1.0, 0);
    extra["priority"] = json!(3);
    assert_eq!(request(&mut app, 0, extra)[0]["type"], "raster");
}

#[test]
fn only_a_few_captures_are_made_at_once() {
    let mut app = app();
    for n in 0..8 {
        let mut v = ask(Some("page"), 1.0, 0);
        v["requestId"] = json!(format!("cap_{n}"));
        assert_eq!(request(&mut app, 0, v)[0]["type"], "raster", "{n}");
    }
    let mut v = ask(Some("page"), 1.0, 0);
    v["requestId"] = json!("cap_9");
    let fx = request(&mut app, 0, v);
    assert_eq!(fx[0]["code"], "busy");
}

fn event_at(app: &mut App, now_ms: u64, seq: u64, command: Value) -> Vec<Value> {
    let command: Command = serde_json::from_value(command).unwrap();
    let frame = Frame::Event {
        seq,
        actor: "agent:other".into(),
        key: Some(format!("k{seq}")),
        command,
    };
    json_of(&app.dispatch(Action::Frame { now_ms, frame }))
}

#[test]
fn a_halted_app_ends_every_capture_as_stale_client_and_drops_what_the_shell_holds() {
    let mut app = app();
    // One request waits for the file, another is being drawn.
    request(&mut app, 0, ask(Some("page"), 1.0, 5));
    let mut second = ask(Some("page"), 1.0, 0);
    second["requestId"] = json!("cap_2");
    assert_eq!(request(&mut app, 0, second)[0]["request"], 2);
    let fx = event_at(
        &mut app,
        0,
        1,
        json!({"type": "updateNode", "id": "ghost", "patch": {"x": 1.0}}),
    );
    assert_eq!(app.state().connection, meldframe_app::Connection::Halted);
    let codes: Vec<_> = fx
        .iter()
        .filter(|e| e["type"] == "failCapture")
        .map(|e| (e["requestId"].clone(), e["code"].clone()))
        .collect();
    assert_eq!(
        codes,
        vec![
            (json!("cap_1"), json!("stale_client")),
            (json!("cap_2"), json!("stale_client"))
        ]
    );
    assert!(fx.contains(&json!({"type": "finishRaster", "request": 2, "deliver": false})));
    // The request that waited holds no bytes: only the failure, no drop of bytes the shell does not have.
    assert!(
        !fx.iter()
            .any(|e| e["type"] == "finishRaster" && e["request"] == 1)
    );
    // Answered once: the shell's late report of the drawing, and the clock, say nothing more.
    assert!(drawn(&mut app, 2, 220, 120, 5000).is_empty());
    assert!(json_of(&app.dispatch(Action::Tick { now_ms: 99_000 })).is_empty());
    // And a request that comes to a halted app is refused at once.
    let mut third = ask(Some("page"), 1.0, 0);
    third["requestId"] = json!("cap_3");
    let fx = request(&mut app, 0, third);
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0]["code"], "stale_client");
}

#[test]
fn a_file_that_gets_there_after_the_time_is_up_draws_nothing() {
    let mut app = app();
    request(&mut app, 1000, ask(Some("page"), 1.0, 2));
    event_at(
        &mut app,
        2000,
        1,
        json!({"type": "updateNode", "id": "extra", "patch": {"w": 6}}),
    );
    // The frame that brings the file to min_seq comes after the 20 seconds, before any tick had ended the request.
    let fx = event_at(
        &mut app,
        22_000,
        2,
        json!({"type": "updateNode", "id": "extra", "patch": {"w": 7}}),
    );
    assert_eq!(failures(&fx), vec![failed("stale_client")]);
    assert!(fx.iter().all(|e| e["type"] != "raster"));
}

#[test]
fn the_upload_token_is_not_in_the_debug_output_of_the_app() {
    let mut app = app();
    request(&mut app, 0, ask(Some("page"), 1.0, 5));
    let mut second = ask(Some("page"), 1.0, 0);
    second["requestId"] = json!("cap_2");
    request(&mut app, 0, second);
    let shown = format!("{app:?}");
    assert!(
        !shown.contains(TOKEN),
        "a log of the app would carry a live upload token"
    );
    assert!(shown.contains("<redacted>"));
}

#[test]
fn a_request_dispatched_as_an_action_meets_the_same_check_as_one_from_the_wire() {
    use meldframe_app::{CaptureAsk, PngOp};
    let mut app = app();
    let ask = |id: &str, token: &str| CaptureAsk {
        request_id: id.into(),
        node_id: Some("page".into()),
        scale: 1.0,
        max_pixels: 16_777_216,
        min_seq: 0,
        upload_token: token.into(),
        timeout_ms: 20_000,
    };
    for (id, token) in [
        ("a/../b", TOKEN),
        ("cap_1", "to ken"),
        ("cap_1", "t\u{202E}oken"),
        ("", TOKEN),
    ] {
        let op = PngOp::Capture {
            now_ms: 0,
            ask: ask(id, token),
        };
        assert!(
            app.dispatch(Action::Png { op }).is_empty(),
            "{id:?} {token:?}"
        );
    }
    let op = PngOp::Capture {
        now_ms: 0,
        ask: ask("cap_1", TOKEN),
    };
    assert_eq!(
        json_of(&app.dispatch(Action::Png { op }))[0]["type"],
        "raster"
    );
}
