//! The PNG export and an agent's capture after the shell has been asked to draw (finding T13 of the thin UI audit, decision
//! 43): one job for each request, the shell's report of the drawing (`Wire::Drawn`) checked against the plan and the limits,
//! the node gone or changed meanwhile, the delivery of the bytes (`Wire::Delivered`), and the answer each request gets once.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App, Connection, Effect, Event, Frame, PngOp, PngSink, Wire};
use meldframe_protocol::{Command, Node};
use serde_json::{Value, json};

fn nodes(list: Value) -> Vec<Node> {
    serde_json::from_value(list).unwrap()
}

/// An app whose file is the frames of `list`, with `id` selected.
fn app_with(list: Value, id: &str) -> App {
    let mut app = App::new("user:me", "c0", 1, nodes(list), 0).unwrap();
    app.dispatch(Action::Select {
        ids: vec![id.into()],
    });
    app.take_events();
    app
}

fn frame(name: &str, w: u32, h: u32) -> Value {
    json!([{"id": "f", "type": "frame", "name": name, "x": 100, "y": 50, "w": w, "h": h}])
}

fn png(app: &mut App, op: PngOp) -> Vec<Effect> {
    app.dispatch(Action::Png { op })
}

fn export(app: &mut App) -> Vec<Effect> {
    png(
        app,
        PngOp::Export {
            id: "f".into(),
            to: PngSink::Download,
        },
    )
}

fn effects_json(fx: &[Effect]) -> Vec<Value> {
    fx.iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect()
}

fn refusals(app: &mut App) -> Vec<String> {
    app.take_events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

#[test]
fn a_new_export_replaces_the_one_still_being_made_and_the_shell_drops_its_bytes() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    export(&mut app);
    let fx = png(
        &mut app,
        PngOp::Export {
            id: "f".into(),
            to: PngSink::Clipboard,
        },
    );
    let e = &effects_json(&fx);
    // The new export replaces the one still being made: the shell drops the bytes it holds for the first.
    assert_eq!(
        e[0],
        json!({"type": "finishRaster", "request": 1, "deliver": false})
    );
    assert_eq!(e[1]["to"], json!({"type": "clipboard"}));
    assert_eq!(e[1]["request"], 2);
}

fn drawn(app: &mut App, request: u32, body: Value) -> Vec<Value> {
    let wire = Wire::Drawn {
        request,
        status: 200,
        body: body.to_string(),
    };
    effects_json(&app.dispatch(Action::Wire { wire }))
}

fn size(w: u32, h: u32, bytes: u64) -> Value {
    json!({"width": w, "height": h, "bytes": bytes})
}

#[test]
fn what_the_shell_drew_is_delivered_once_and_its_delivery_is_read_by_the_app() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    export(&mut app);
    assert_eq!(
        drawn(&mut app, 1, size(200, 100, 5000)),
        vec![json!({"type": "finishRaster", "request": 1, "deliver": true})]
    );
    // A repeated answer finds no request at that stage: nothing is delivered twice.
    assert!(drawn(&mut app, 1, size(200, 100, 5000)).is_empty());
    let done = |app: &mut App, status: u16, body: &str| {
        let wire = Wire::Delivered {
            request: 1,
            status,
            body: body.into(),
        };
        app.dispatch(Action::Wire { wire });
        refusals(app)
    };
    assert!(done(&mut app, 200, "").is_empty());
    // And one that arrives late finds nothing.
    assert!(done(&mut app, 500, "").is_empty());
    export(&mut app);
    drawn(&mut app, 2, size(200, 100, 5000));
    assert_eq!(
        done_for(&mut app, 2, 507, r#"{"error":{"code":"quota_exceeded"}}"#),
        ["quota_exceeded"]
    );
}

fn done_for(app: &mut App, request: u32, status: u16, body: &str) -> Vec<String> {
    let wire = Wire::Delivered {
        request,
        status,
        body: body.into(),
    };
    app.dispatch(Action::Wire { wire });
    refusals(app)
}

#[test]
fn a_drawing_that_is_over_the_limit_or_not_the_planned_size_is_refused_as_data_and_dropped() {
    let drop = json!({"type": "finishRaster", "request": 1, "deliver": false});
    for (report, code) in [
        (size(4097, 4096, 5000), "too_large"),
        (size(200, 100, 0), "render_failed"),
        (size(199, 100, 5000), "render_failed"),
        (size(0, 0, 5000), "render_failed"),
    ] {
        let mut app = app_with(frame("Hero", 200, 100), "f");
        export(&mut app);
        app.take_events();
        assert_eq!(
            drawn(&mut app, 1, report.clone()),
            vec![drop.clone()],
            "{report}"
        );
        assert_eq!(refusals(&mut app), [code], "{report}");
        // The request is answered: a second report of it is nothing.
        assert!(drawn(&mut app, 1, size(200, 100, 5000)).is_empty());
    }
}

#[test]
fn a_report_that_is_not_the_shells_is_a_failed_drawing_with_a_code_made_safe() {
    let cases = [
        (200, "<html>proxy</html>", "render_failed"),
        (200, r#"{"width":-1,"height":1,"bytes":1}"#, "render_failed"),
        (500, "", "render_failed"),
        (500, r#"{"error":{"code":"canvas_lost"}}"#, "canvas_lost"),
    ];
    for (status, body, want) in cases {
        let mut app = app_with(frame("Hero", 200, 100), "f");
        export(&mut app);
        app.take_events();
        let wire = Wire::Drawn {
            request: 1,
            status,
            body: body.into(),
        };
        assert!(app.dispatch(Action::Wire { wire }).is_empty());
        assert_eq!(refusals(&mut app), [want], "{body}");
    }
    // A code with a hidden character is made safe before it is an event, and a failure for no request is nothing.
    let mut app = app_with(frame("Hero", 200, 100), "f");
    export(&mut app);
    app.take_events();
    png(
        &mut app,
        PngOp::DrawFailed {
            request: 99,
            code: "x".into(),
        },
    );
    assert!(refusals(&mut app).is_empty());
    png(
        &mut app,
        PngOp::DrawFailed {
            request: 1,
            code: "bad\u{202E}code".into(),
        },
    );
    assert_eq!(refusals(&mut app), ["badcode"]);
}

#[test]
fn a_frame_that_is_gone_or_changed_before_the_bytes_came_back_ends_the_request_once() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    export(&mut app);
    app.dispatch(Action::Delete {
        ids: vec!["f".into()],
    });
    app.take_events();
    assert_eq!(
        drawn(&mut app, 1, size(200, 100, 5000)),
        vec![json!({"type": "finishRaster", "request": 1, "deliver": false})]
    );
    assert_eq!(refusals(&mut app), ["node_not_found"]);
    assert!(
        drawn(&mut app, 1, size(200, 100, 5000)).is_empty(),
        "answered once"
    );

    let mut app = app_with(frame("Hero", 200, 100), "f");
    export(&mut app);
    app.dispatch(Action::Resize {
        id: "f".into(),
        w: 300.0,
        h: 100.0,
        x: None,
        y: None,
    });
    app.take_events();
    assert_eq!(drawn(&mut app, 1, size(200, 100, 5000)).len(), 1);
    assert_eq!(refusals(&mut app), ["node_changed"]);
}

#[test]
fn a_frame_moved_or_restyled_while_the_shell_draws_is_not_delivered_either() {
    // The TypeScript saved the PNG; the app asks again, because the rectangle the shell drew is not the frame's any more.
    let edits = [
        Action::Move {
            id: "f".into(),
            x: 400.0,
            y: 50.0,
        },
        // A shadow reaches beyond the frame: the rectangle to draw grows.
        Action::SetProperty {
            id: "f".into(),
            property: meldframe_app::Property::AddShadow {},
        },
    ];
    for edit in edits {
        let mut app = app_with(frame("Hero", 200, 100), "f");
        export(&mut app);
        app.dispatch(edit.clone());
        app.take_events();
        let fx = drawn(&mut app, 1, size(200, 100, 5000));
        assert_eq!(
            fx,
            vec![json!({"type": "finishRaster", "request": 1, "deliver": false})],
            "{edit:?}"
        );
        assert_eq!(refusals(&mut app), ["node_changed"], "{edit:?}");
    }
}

#[test]
fn an_export_in_flight_when_the_app_halts_is_dropped_and_never_delivered() {
    let mut app = app_with(frame("Hero", 200, 100), "f");
    export(&mut app);
    // An event for a node the file does not hold: the view no longer fits the server's.
    let command: Command =
        serde_json::from_value(json!({"type": "updateNode", "id": "ghost", "patch": {"x": 1.0}}))
            .unwrap();
    app.dispatch(Action::Frame {
        now_ms: 0,
        frame: Frame::Event {
            seq: 1,
            actor: "agent:other".into(),
            key: Some("k1".into()),
            command,
        },
    });
    assert_eq!(app.state().connection, Connection::Halted);
    app.take_events();
    // The shell's report of the drawing finds a halted app: the bytes are dropped, the person is told, once.
    assert_eq!(
        drawn(&mut app, 1, size(200, 100, 5000)),
        vec![json!({"type": "finishRaster", "request": 1, "deliver": false})]
    );
    assert_eq!(refusals(&mut app), ["out_of_sync"]);
    assert!(drawn(&mut app, 1, size(200, 100, 5000)).is_empty());
}
