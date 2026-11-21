//! The text measurer crosses the boundary: what a host answer is allowed to be (`GuardedMeasurer`, the part of the
//! JavaScript-backed measurer that is plain Rust) and what `Bridge::set_measurer` does with one. The call into
//! JavaScript itself is checked through the real wasm by `tools/wasm-smoke.mjs`.

use meldframe_layout::{StubMeasurer, TextMeasurer};
use meldframe_protocol::Node;
use meldframe_wasm::{Bridge, GuardedMeasurer, MAX_MEASURE};
use serde_json::{Value, json};

fn text(s: &str, size: f64) -> Node {
    serde_json::from_value(
        json!({"id": "t", "type": "text", "text": s, "fontSize": size, "fontWeight": 700}),
    )
    .unwrap()
}

#[test]
fn an_answer_the_host_gives_is_used_as_it_is() {
    let m = GuardedMeasurer::new(|_| Some((123.0, 17.0)));
    assert_eq!(m.measure(&text("hello", 16.0), None), (123.0, 17.0));
    assert_eq!(m.measure(&text("hello", 16.0), Some(50.0)), (123.0, 17.0));
}

#[test]
fn the_host_gets_the_text_the_size_the_weight_and_the_wrap_width_and_nothing_else() {
    let seen = std::cell::RefCell::new(Vec::new());
    let m = GuardedMeasurer::new(|q| {
        seen.borrow_mut()
            .push((q.text.to_owned(), q.font_size, q.font_weight, q.wrap_width));
        Some((1.0, 1.0))
    });
    m.measure(&text("a b", 20.0), None);
    m.measure(&text("a b", 20.0), Some(33.5));
    assert_eq!(
        *seen.borrow(),
        vec![
            ("a b".to_owned(), 20.0, 700, None),
            ("a b".to_owned(), 20.0, 700, Some(33.5))
        ]
    );
}

#[test]
fn an_answer_that_is_not_a_size_falls_back_to_the_stub() {
    let n = text("hello world", 16.0);
    let stub = StubMeasurer.measure(&n, Some(40.0));
    let bad = [
        None,
        Some((f64::NAN, 10.0)),
        Some((10.0, f64::NAN)),
        Some((f64::INFINITY, 10.0)),
        Some((10.0, f64::NEG_INFINITY)),
        Some((-1.0, 10.0)),
        Some((10.0, -0.5)),
        Some((MAX_MEASURE + 1.0, 10.0)),
        Some((10.0, 1e300)),
    ];
    for answer in bad {
        let m = GuardedMeasurer::new(move |_| answer);
        assert_eq!(m.measure(&n, Some(40.0)), stub, "{answer:?}");
    }
}

#[test]
fn the_edges_of_what_is_allowed_are_allowed() {
    let n = text("x", 16.0);
    for answer in [(0.0, 0.0), (MAX_MEASURE, MAX_MEASURE)] {
        let m = GuardedMeasurer::new(move |_| Some(answer));
        assert_eq!(m.measure(&n, None), answer);
    }
}

fn bridge() -> Bridge {
    let nodes = json!([
        {"id": "f", "type": "frame", "w": 400, "h": 100, "layout": "row", "padding": 0, "gap": 0},
        {"id": "t", "type": "text", "parentId": "f", "pos": "a", "text": "Hello", "fontSize": 20, "sw": "hug", "sh": "hug"},
    ]);
    Bridge::new("usr_me", "web", 7, &nodes.to_string(), 0, None).expect("the file loads")
}

fn text_box(b: &Bridge) -> (f64, f64) {
    let scene: Vec<Value> = serde_json::from_str(&b.scene_json()).unwrap();
    let t = scene.iter().find(|b| b["id"] == "t").unwrap();
    (t["w"].as_f64().unwrap(), t["h"].as_f64().unwrap())
}

#[test]
fn a_measurer_changes_the_hug_size_of_a_text_once_the_flag_is_on() {
    let mut b = bridge();
    b.set_layout_on_canvas(true);
    assert_eq!(
        text_box(&b),
        (50.0, 24.0),
        "the stub: half an em per character"
    );
    let fx: Value = serde_json::from_str(
        &b.set_measurer(Box::new(GuardedMeasurer::new(|_| Some((77.0, 31.0))))),
    )
    .unwrap();
    assert_eq!(
        fx,
        json!([{"type": "render", "nodes": []}]),
        "a redraw of everything is asked for"
    );
    assert_eq!(text_box(&b), (77.0, 31.0));
}

#[test]
fn a_measurer_given_while_the_flag_is_off_asks_for_nothing_and_changes_nothing() {
    let mut b = bridge();
    let before = b.scene_json();
    assert_eq!(
        b.set_measurer(Box::new(GuardedMeasurer::new(|_| Some((77.0, 31.0))))),
        "[]"
    );
    assert_eq!(b.scene_json(), before);
    b.set_layout_on_canvas(true);
    assert_eq!(
        text_box(&b),
        (77.0, 31.0),
        "it is used as soon as the flag is on"
    );
}

#[test]
fn a_measurer_that_gives_nonsense_never_poisons_the_scene() {
    let mut b = bridge();
    b.set_layout_on_canvas(true);
    b.set_measurer(Box::new(GuardedMeasurer::new(|_| Some((f64::NAN, -3.0)))));
    assert_eq!(text_box(&b), (50.0, 24.0));
    let scene: Vec<Value> = serde_json::from_str(&b.scene_json()).unwrap();
    for bx in scene {
        // A NaN would serialise as null.
        assert!(
            ["x", "y", "w", "h"]
                .iter()
                .all(|k| bx[k].as_f64().is_some()),
            "{bx}"
        );
    }
}

#[test]
fn the_measurer_is_not_state_not_an_event_and_not_the_file() {
    let mut b = bridge();
    b.set_layout_on_canvas(true);
    let (state, nodes) = (b.state_json(), b.nodes_json());
    b.set_measurer(Box::new(GuardedMeasurer::new(|_| Some((77.0, 31.0)))));
    assert_eq!(b.state_json(), state);
    assert_eq!(b.nodes_json(), nodes);
    assert_eq!(b.take_events_json(), "[]");
}
