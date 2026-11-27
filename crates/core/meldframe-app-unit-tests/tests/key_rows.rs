//! The key table against the app layer (`Action::Key`, audit finding T7a): every row the app answers is a key that does something in
//! the state it applies to, and the action is JSON that a front end can send.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::keys::*;
use common::*;
use meldframe_app::{Action, App, Direction, Effect, Modifiers, Overlay, OverlayOp};
use meldframe_app_rules::keys::{Combo, Held, Makes, TABLE};
use serde_json::json;

#[test]
fn the_action_is_json_and_the_flags_may_be_left_out_but_not_the_focus() {
    let a: Action = serde_json::from_value(json!({
        "type": "key",
        "input": {"key": "z", "code": "KeyZ", "modifiers": {"ctrl": true}, "focus": "page"}
    }))
    .unwrap();
    let mut l = live();
    l.send(Action::Move {
        id: "r1".into(),
        x: 40.0,
        y: 0.0,
    });
    l.send(a);
    assert_eq!(get(&l.app, "r1").x, 0.0);
    let spelled = serde_json::to_value(key("v")).unwrap();
    assert_eq!(
        spelled,
        json!({"type": "key", "input": {"key": "v", "code": "", "modifiers": {"ctrl": false, "meta": false, "alt": false, "shift": false},
               "repeat": false, "composing": false, "focus": "page"}})
    );
    for bad in [
        json!({"type": "key", "input": {"key": "v", "focus": "page", "extra": 1}}),
        json!({"type": "key", "input": {"key": "v"}}),
        json!({"type": "key", "input": {"key": "v", "focus": "nowhere"}}),
        json!({"type": "key", "input": {"code": "KeyV", "focus": "page"}}),
        json!({"type": "key", "key": "v"}),
    ] {
        assert!(serde_json::from_value::<Action>(bad).is_err());
    }
}

/// A file and the selection in which the key of `makes` has something to act on: the setup of the one test below.
fn prepared(makes: Makes) -> App {
    let mut nodes = doc();
    nodes.extend(serde_json::from_value::<Vec<_>>(json!([
        {"id": "col", "type": "frame", "pos": "a1", "w": 200, "h": 300, "layout": "column"},
        {"id": "a", "type": "rect", "parentId": "col", "pos": "a0", "w": 50, "h": 20},
        {"id": "b", "type": "rect", "parentId": "col", "pos": "a1", "w": 50, "h": 20},
        {"id": "row", "type": "frame", "pos": "a2", "x": 400, "w": 300, "h": 100, "layout": "row"},
        {"id": "c", "type": "rect", "parentId": "row", "pos": "a0", "w": 40, "h": 40},
        {"id": "d", "type": "rect", "parentId": "row", "pos": "a1", "w": 40, "h": 40},
    ]))
    .unwrap());
    let mut app = App::new("user:me", "c0", 1, nodes, 0).unwrap();
    app.set_layout_on_canvas(true);
    app.dispatch(Action::Connected { now_ms: 0 });
    let select = |app: &mut App, id: &str| {
        app.dispatch(Action::Select { ids: ids(&[id]) });
    };
    match makes {
        Makes::SetTool(_) | Makes::TogglePanel(_) | Makes::ToggleOverlay(_) | Makes::Host => {}
        Makes::Undo | Makes::Redo => {
            app.dispatch(Action::Move {
                id: "r1".into(),
                x: 40.0,
                y: 0.0,
            });
            if makes == Makes::Redo {
                app.dispatch(Action::Undo {});
            }
        }
        Makes::DeleteSelection => select(&mut app, "r1"),
        Makes::BeginTextEdit => select(&mut app, "t1"),
        Makes::Cancel => {
            app.dispatch(Action::Overlay {
                op: OverlayOp::Open {
                    overlay: Overlay::Help,
                },
            });
        }
        // The key that has a neighbour to swap with: the first child going down or right, the last going up or left.
        Makes::Nudge(Direction::Down) => select(&mut app, "a"),
        Makes::Nudge(Direction::Up) => select(&mut app, "b"),
        Makes::Nudge(Direction::Right) => select(&mut app, "c"),
        Makes::Nudge(Direction::Left) => select(&mut app, "d"),
    }
    app.take_events();
    app
}

#[test]
fn every_row_of_the_table_that_the_app_answers_is_a_key_that_does_something() {
    let mut seen = 0;
    for b in TABLE.iter().flat_map(|g| g.rows).flat_map(|r| r.bindings) {
        if b.makes == Makes::Host {
            continue;
        }
        let Combo {
            key,
            code,
            command,
            alt,
            shift,
        } = b.combo;
        let m = Modifiers {
            ctrl: command,
            meta: false,
            alt: alt == Held::Yes,
            shift: shift == Held::Yes,
        };
        let mut app = prepared(b.makes);
        let fx = app.dispatch(press(key, code.unwrap_or(""), m));
        let events = app.take_events();
        assert!(
            handled(&events),
            "{key}: {:?} made nothing in the state it applies to",
            b.makes
        );
        let sent = fx.iter().any(|e| matches!(e, Effect::SendOps { .. }));
        match b.makes {
            Makes::Undo | Makes::Redo | Makes::DeleteSelection | Makes::Nudge(_) => {
                assert!(sent, "{key}: {:?} writes to the file", b.makes);
            }
            _ => assert!(!sent, "{key}: {:?} writes nothing", b.makes),
        }
        seen += 1;
    }
    assert_eq!(
        seen,
        5 + 1 + 2 + 2 + 4 + 1 + 1 + 2 + 2,
        "the count of the bindings the app answers"
    );
}
