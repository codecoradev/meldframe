//! The preview and the drop of a selection cost what the selection and the file cost, not the square of the selection: a
//! person can select every child of a long column and drag it. The preview runs on every frame of a drag, so it is checked
//! with a budget: 5,000 selected siblings must take far less than the quadratic rules took (which was over 100 ms a call in
//! a release build and many times that in this debug build). The check measures time, which a WASM test has no clock for,
//! so it runs on native targets only.
#![cfg(not(target_arch = "wasm32"))]

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::App;
use meldframe_protocol::Node;
use std::time::Instant;

const N: usize = 5000;

/// One column with `N` flow children of 10 high, and the ids of all of them.
fn long_column() -> (App, Vec<String>) {
    let mut list = vec![serde_json::json!(
        {"id": "col", "type": "frame", "pos": "a0", "w": 200, "h": 60000, "layout": "column"}
    )];
    for i in 0..N {
        list.push(serde_json::json!(
            {"id": format!("k{i}"), "type": "rect", "parentId": "col", "pos": format!("a{i:05}"), "w": 50, "h": 10}
        ));
    }
    let nodes: Vec<Node> = serde_json::from_value(serde_json::Value::Array(list)).unwrap();
    let ids = (0..N).map(|i| format!("k{i}")).collect();
    (app_of(nodes, true), ids)
}

#[test]
fn the_preview_of_five_thousand_selected_siblings_is_linear_in_the_selection() {
    let (a, ids) = long_column();
    // Past the middle of the last child: the whole column is the block and stays in one piece.
    let at = (10.0, 49_995.0);
    a.drop_preview_many(&ids, at, (0.0, 0.0))
        .expect("a preview"); // lays the file out once
    let t = Instant::now();
    let calls = 5;
    for _ in 0..calls {
        a.drop_preview_many(&ids, at, (0.0, 0.0))
            .expect("a preview");
    }
    let per_call = t.elapsed() / calls;
    eprintln!("{N} selected siblings: {:?} a preview", per_call);
    assert!(
        per_call.as_millis() < 100,
        "{} ms a preview for {N} selected siblings (a linear pass takes a few ms even unoptimised)",
        per_call.as_millis()
    );
}
