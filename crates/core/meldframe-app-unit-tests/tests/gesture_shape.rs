//! The wire shape of `Action::Gesture` (T8, decision 43): what a TypeScript caller can send and what is refused. What the
//! action does is tested in `gesture_actions.rs` and in `meldframe-app-layout-tests`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, CreateKind, GestureOp, Handle};
use serde_json::json;

fn draw_json() -> serde_json::Value {
    json!({"type": "gesture", "op": {"type": "draw", "kind": "ellipse",
        "from": {"x": 1.5, "y": 2.0}, "to": {"x": 10.0, "y": 20.0}}})
}

#[test]
fn a_gesture_reads_from_json_with_the_keys_left_out() {
    let a: Action = serde_json::from_value(draw_json()).unwrap();
    let Action::Gesture {
        op: GestureOp::Draw {
            kind, from, mods, ..
        },
    } = a
    else {
        panic!("a draw")
    };
    assert_eq!(kind, CreateKind::Ellipse);
    assert_eq!(from.x, 1.5);
    assert!(!(mods.shift || mods.alt || mods.ctrl || mods.meta));
    let a: Action = serde_json::from_value(json!({"type": "gesture", "op": {
        "type": "draw", "kind": "frame", "from": {"x": 0, "y": 0}, "to": {"x": 1, "y": 1},
        "mods": {"shift": true}}}))
    .unwrap();
    assert!(
        matches!(a, Action::Gesture { op: GestureOp::Draw { mods, .. } } if mods.shift && !mods.alt)
    );
}

#[test]
fn a_handle_reads_by_its_compass_name_and_every_kind_round_trips() {
    for (name, want) in [
        ("nw", Handle::Nw),
        ("n", Handle::N),
        ("ne", Handle::Ne),
        ("e", Handle::E),
        ("se", Handle::Se),
        ("s", Handle::S),
        ("sw", Handle::Sw),
        ("w", Handle::W),
    ] {
        let a: Action = serde_json::from_value(json!({"type": "gesture", "op": {
            "type": "resizeHandle", "id": "r1", "handle": name, "by": {"x": 1, "y": 2}}}))
        .unwrap();
        assert!(
            matches!(a, Action::Gesture { op: GestureOp::ResizeHandle { handle, .. } } if handle == want)
        );
    }
    assert!(
        serde_json::from_value::<Action>(json!({"type": "gesture", "op": {
            "type": "resizeHandle", "id": "r1", "handle": "NW", "by": {"x": 1, "y": 2}}}))
        .is_err()
    );
    for kind in ["frame", "rectangle", "ellipse", "text"] {
        let mut j = draw_json();
        j["op"]["kind"] = json!(kind);
        let a: Action = serde_json::from_value(j.clone()).unwrap();
        let back = serde_json::to_value(&a).unwrap();
        assert_eq!(back["op"]["kind"], j["op"]["kind"]);
        assert_eq!(back["op"]["from"], j["op"]["from"]);
        assert_eq!(
            back["op"]["mods"]["shift"],
            json!(false),
            "the keys are written out"
        );
    }
}

#[test]
fn unknown_fields_and_kinds_are_refused() {
    let mut j = draw_json();
    j["extra"] = json!(1);
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["extra"] = json!(1);
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["from"]["z"] = json!(1);
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["mods"] = json!({"super": true});
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["kind"] = json!("star");
    assert!(serde_json::from_value::<Action>(j).is_err());
    let mut j = draw_json();
    j["op"]["type"] = json!("lasso");
    assert!(serde_json::from_value::<Action>(j).is_err());
}
