//! The acceptance test of E8.1.4: select by click, move by drag (one undo step), marquee, resize by a handle, a drop above a
//! sibling that reorders, a pan and a zoom to the pointer, and every undo step back, in one flow with no front end. The flow is
//! `scripts/pointer_flow.script`, with its checks after each step; the harness sends the pointer as actions and reads
//! `AppState` and the preview, and holds no rule of its own.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::{files, node};
use meldframe_headless::run;
use serde_json::json;

const FLOW: &str = include_str!("scripts/pointer_flow.script");

#[test]
fn the_whole_pointer_flow_goes_through_and_undoes_back_to_the_start() {
    let r = run(FLOW, &files, true);
    assert_eq!(r.failure, None, "{}", r.out);
    assert!(r.refusals.is_empty(), "{:?}", r.refusals);
    // The file is as it was opened: every number the file gave is back, and the siblings are in their first order.
    let opened: serde_json::Value = serde_json::from_str(&files("flow.json").unwrap()).unwrap();
    for n in opened.as_array().unwrap() {
        let now = node(&r, n["id"].as_str().unwrap());
        for field in ["x", "y", "w", "h"] {
            if let Some(was) = n[field].as_f64() {
                assert_eq!(now[field].as_f64(), Some(was), "{} {field}", n["id"]);
            }
        }
        assert_eq!(now["pos"], n["pos"], "{} pos", n["id"]);
    }
    let s = r.shell.as_ref().expect("a file is open");
    // The camera is view state: the zoom is still there, and nothing of it went to the file.
    assert!(s.state()["camera"]["zoom"].as_f64().unwrap() > 1.2);
    assert_eq!(s.state()["selection"], json!(["b"]));
}
