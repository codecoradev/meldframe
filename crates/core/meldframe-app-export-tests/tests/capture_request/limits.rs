//! The limits and the clock of an agent's capture (finding T13 of the thin UI audit, decision 43): the secret the request
//! carries is not in a log.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::*;
use meldframe_app::PngOp;

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
