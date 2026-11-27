//! The wire shape of the pointer data (E8.1.4, part 2): what a front end sends and reads as JSON. The machine that gives it meaning
//! is `meldframe-app-pointer`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_model::{
    Action, AppState, PointerButton, PointerCursor, PointerGesture, PointerPhase,
};
use serde_json::json;

#[test]
fn a_pointer_action_needs_only_its_phase_and_point() {
    let a: Action = serde_json::from_value(
        json!({"type": "pointer", "input": {"phase": "down", "x": 1.5, "y": 2.0}}),
    )
    .unwrap();
    let Action::Pointer { input } = a else {
        panic!("a pointer action")
    };
    assert_eq!(input.phase, PointerPhase::Down);
    assert_eq!((input.x, input.y), (1.5, 2.0));
    assert_eq!(input.button, PointerButton::Primary);
    assert!(!input.touch && !input.space && !input.modifiers.shift);
}

#[test]
fn an_unknown_field_or_phase_is_an_error() {
    for bad in [
        json!({"type": "pointer", "input": {"phase": "down", "x": 1, "y": 2, "pressure": 1}}),
        json!({"type": "pointer", "input": {"phase": "hover", "x": 1, "y": 2}}),
        json!({"type": "pointer", "input": {"phase": "down", "x": 1}}),
    ] {
        assert!(serde_json::from_value::<Action>(bad).is_err());
    }
}

#[test]
fn the_pointer_state_starts_idle_and_names_the_cursor_as_css_does() {
    let s = AppState::default();
    assert_eq!(
        serde_json::to_value(&s.pointer).unwrap(),
        json!({"cursor": "default", "gesture": {"type": "idle"}})
    );
    assert_eq!(s.pointer.gesture, PointerGesture::Idle);
    let names = [
        (PointerCursor::Grab, "grab"),
        (PointerCursor::Grabbing, "grabbing"),
        (PointerCursor::Crosshair, "crosshair"),
        (PointerCursor::NwseResize, "nwse-resize"),
        (PointerCursor::NeswResize, "nesw-resize"),
        (PointerCursor::NsResize, "ns-resize"),
        (PointerCursor::EwResize, "ew-resize"),
    ];
    for (c, name) in names {
        assert_eq!(serde_json::to_value(c).unwrap(), json!(name));
    }
}
