//! The wire shape of actions, effects, events and state (standard rule U1: plain serialisable data).

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, AppState, Connection, Effect, Event};
use serde_json::json;

#[test]
fn actions_read_from_json_and_refuse_unknown_fields() {
    let a: Action = serde_json::from_value(
        json!({"type":"createNode","kind":"rectangle","x":1.0,"y":2.0,"parent":null}),
    )
    .unwrap();
    assert!(matches!(a, Action::CreateNode { .. }));
    let a = serde_json::from_value::<Action>(
        json!({"type":"setProperty","id":"r1","property":"fill","value":"#fff"}),
    );
    assert!(
        a.is_err(),
        "the property is one field with a tag and a value"
    );
    let a: Action = serde_json::from_value(
        json!({"type":"setProperty","id":"r1","property":{"property":"fill","value":"#fff"}}),
    )
    .unwrap();
    assert!(matches!(a, Action::SetProperty { .. }));
    // The size of a new node and the origin of a resize are optional: the shapes without them still read.
    let a: Action = serde_json::from_value(
        json!({"type":"createNode","kind":"frame","x":0.0,"y":0.0,"parent":null,"w":300.0,"h":200.0}),
    )
    .unwrap();
    assert!(
        matches!(a, Action::CreateNode { w: Some(w), h: Some(h), .. } if w == 300.0 && h == 200.0)
    );
    let a: Action =
        serde_json::from_value(json!({"type":"resize","id":"r1","w":3.0,"h":4.0})).unwrap();
    assert!(matches!(
        a,
        Action::Resize {
            x: None,
            y: None,
            ..
        }
    ));
    let a: Action =
        serde_json::from_value(json!({"type":"resize","id":"r1","w":3.0,"h":4.0,"x":1.0,"y":2.0}))
            .unwrap();
    assert!(matches!(
        a,
        Action::Resize {
            x: Some(_),
            y: Some(_),
            ..
        }
    ));
    assert!(serde_json::from_value::<Action>(json!({"type":"undo","x":1})).is_err());
    assert!(serde_json::from_value::<Action>(json!({"type":"select","ids":[],"x":1})).is_err());
}

#[test]
fn state_effects_and_events_serialise_and_the_state_round_trips() {
    let mut app = app();
    app.dispatch(Action::Connected { now_ms: 0 });
    app.dispatch(Action::Select { ids: ids(&["r1"]) });
    let fx = app.dispatch(Action::Move {
        id: "r1".into(),
        x: 1.0,
        y: 2.0,
    });
    let v = serde_json::to_value(&fx).unwrap();
    assert_eq!(v[1]["type"], "sendOps");
    assert_eq!(v[0], json!({"type":"render","nodes":["r1"]}));
    let state = serde_json::to_value(app.state()).unwrap();
    assert_eq!(state["connection"], json!({"type":"live"}));
    let back: AppState = serde_json::from_value(state).unwrap();
    assert_eq!(&back, app.state());
    let e = serde_json::to_value(Event::ConnectionChanged {
        connection: Connection::Reconnecting { attempt: 2 },
    })
    .unwrap();
    assert_eq!(e["connection"], json!({"type":"reconnecting","attempt":2}));
    let _: Vec<Effect> = serde_json::from_value(serde_json::to_value(&fx).unwrap()).unwrap();
}
