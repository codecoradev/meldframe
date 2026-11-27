//! What is under a point crosses the boundary (T9, decision 43): `Bridge::hit` answers a plain id, or nothing, for a world
//! point, and the shared fixtures of the render crate (`tests/fixtures/hit.json`) give the same ids through it. The call
//! changes nothing and raises no event.

use meldframe_wasm::Bridge;
use serde_json::Value;

const FIXTURES: &str = include_str!("../../../core/meldframe-render/tests/fixtures/hit.json");

#[test]
fn the_bridge_answers_every_probe_of_the_shared_fixtures() {
    let cases: Vec<Value> = serde_json::from_str(FIXTURES).unwrap();
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        // A file whose parent is missing is refused by the app when it loads; only the render crate reads it.
        if case["renderOnly"].as_bool().unwrap_or(false) {
            continue;
        }
        let layout_case = case["layout"].as_bool().unwrap_or(false);
        let flags: &[bool] = if layout_case { &[true] } else { &[false, true] };
        for &layout in flags {
            let mut b = Bridge::new("usr_me", "web", 7, &case["nodes"].to_string(), 0, None)
                .expect("the file loads");
            b.set_layout_on_canvas(layout);
            let state = b.state_json();
            for p in case["probes"].as_array().unwrap() {
                let (x, y) = (p["at"][0].as_f64().unwrap(), p["at"][1].as_f64().unwrap());
                assert_eq!(
                    b.hit(x, y).as_deref(),
                    p["hit"].as_str(),
                    "{name}, layout {layout}, at ({x}, {y})"
                );
            }
            assert_eq!(b.state_json(), state, "{name}: asking changes no state");
            assert_eq!(b.take_events_json(), "[]", "{name}: asking raises no event");
        }
    }
}

#[test]
fn a_point_that_is_not_a_number_is_over_nothing() {
    let b = Bridge::new(
        "usr_me",
        "web",
        7,
        r#"[{"id":"a","type":"rect","pos":"a0","x":0,"y":0,"w":10,"h":10}]"#,
        0,
        None,
    )
    .unwrap();
    assert_eq!(b.hit(f64::NAN, 1.0), None);
    assert_eq!(b.hit(1.0, f64::INFINITY), None);
    assert_eq!(b.hit(1.0, 1.0).as_deref(), Some("a"));
}
