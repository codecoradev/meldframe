//! Font weight, text alignment, border and shadows through the core's submit-a-command interface
//! (wire JSON + `apply_command`). Expected values are literals, not recomputed.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{Limits, apply_command};
use meldframe_protocol::{Command, Node};
use serde_json::{Value, json};

fn cmd(v: Value) -> Command {
    serde_json::from_value(v).unwrap()
}

fn wire(nodes: &[Node], id: &str) -> Value {
    serde_json::to_value(nodes.iter().find(|n| n.id == id).unwrap()).unwrap()
}

fn apply(nodes: &mut Vec<Node>, v: Value) -> Result<(), meldframe_core::ApplyError> {
    apply_command(nodes, &cmd(v), &Limits::default())
}

fn update(nodes: &mut Vec<Node>, patch: Value) -> Result<(), meldframe_core::ApplyError> {
    apply(
        nodes,
        json!({"type": "updateNode", "id": "t", "patch": patch}),
    )
}

fn base_doc() -> Vec<Node> {
    let mut ns = vec![];
    let add = json!({"type": "addNode", "node": {"id": "t", "type": "text", "text": "hi"}});
    apply(&mut ns, add).unwrap();
    ns
}

#[test]
fn font_weight_is_set_on_add_and_defaults_to_400() {
    let mut ns = vec![];
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "a", "type": "text", "fontWeight": 700}}),
    )
    .unwrap();
    apply(
        &mut ns,
        json!({"type": "addNode", "node": {"id": "b", "type": "text"}}),
    )
    .unwrap();
    assert_eq!(wire(&ns, "a")["fontWeight"], json!(700));
    assert_eq!(wire(&ns, "b")["fontWeight"], json!(400));
}

#[test]
fn a_bad_font_weight_is_refused_and_leaves_the_nodes_alone() {
    let mut ns = base_doc();
    let before = serde_json::to_string(&ns).unwrap();
    for bad in [0, 50, 450, 1000] {
        let e = update(&mut ns, json!({"fontWeight": bad})).unwrap_err();
        assert!(e.to_string().contains("fontWeight"), "{e}");
        let add =
            json!({"type": "addNode", "node": {"id": "z", "type": "text", "fontWeight": bad}});
        assert!(apply(&mut ns, add).is_err());
        assert_eq!(serde_json::to_string(&ns).unwrap(), before);
    }
    update(&mut ns, json!({"fontWeight": 500})).unwrap();
    assert_eq!(wire(&ns, "t")["fontWeight"], json!(500));
}

#[test]
fn text_align_defaults_left_and_takes_center_and_right() {
    let mut ns = base_doc();
    assert_eq!(wire(&ns, "t")["textAlign"], json!("left"));
    for v in ["center", "right", "left"] {
        update(&mut ns, json!({"textAlign": v})).unwrap();
        assert_eq!(wire(&ns, "t")["textAlign"], json!(v));
    }
}

#[test]
fn a_border_patch_replaces_the_whole_border() {
    let mut ns = base_doc();
    assert_eq!(
        wire(&ns, "t")["border"],
        json!({"width": 0.0, "color": "#000000", "position": "inside"})
    );
    let full = json!({"width": 3, "color": "#ff0000", "position": "outside"});
    update(&mut ns, json!({"border": full})).unwrap();
    assert_eq!(
        wire(&ns, "t")["border"],
        json!({"width": 3.0, "color": "#ff0000", "position": "outside"})
    );
    // Members left out take their defaults.
    update(&mut ns, json!({"border": {"width": 1}})).unwrap();
    assert_eq!(
        wire(&ns, "t")["border"],
        json!({"width": 1.0, "color": "#000000", "position": "inside"})
    );
}

#[test]
fn a_bad_border_is_refused_on_add_and_update() {
    let mut ns = base_doc();
    let before = serde_json::to_string(&ns).unwrap();
    let e = update(&mut ns, json!({"border": {"width": 2, "color": "red"}})).unwrap_err();
    assert!(e.to_string().contains("border.color"), "{e}");
    let add =
        json!({"type": "addNode", "node": {"id": "z", "type": "rect", "border": {"width": -1}}});
    assert!(
        apply(&mut ns, add)
            .unwrap_err()
            .to_string()
            .contains("border.width")
    );
    assert_eq!(serde_json::to_string(&ns).unwrap(), before);
}

#[test]
fn a_shadows_patch_replaces_the_list_and_empty_clears_it() {
    let mut ns = base_doc();
    assert_eq!(wire(&ns, "t")["shadows"], json!([]));
    let first = json!({"x": 2, "y": 4, "blur": 8, "spread": 1, "color": "#00000080"});
    update(
        &mut ns,
        json!({"shadows": [first.clone(), {"color": "#f00"}]}),
    )
    .unwrap();
    let got = wire(&ns, "t")["shadows"].clone();
    assert_eq!(got.as_array().unwrap().len(), 2);
    assert_eq!(
        got[0],
        json!({"x": 2.0, "y": 4.0, "blur": 8.0, "spread": 1.0, "color": "#00000080"})
    );
    update(&mut ns, json!({"shadows": [first]})).unwrap();
    assert_eq!(wire(&ns, "t")["shadows"].as_array().unwrap().len(), 1);
    // Members left out take their defaults (offsets 0, 25% black).
    update(&mut ns, json!({"shadows": [{"blur": 5}]})).unwrap();
    let defaults = json!({"x": 0.0, "y": 0.0, "blur": 5.0, "spread": 0.0, "color": "#00000040"});
    assert_eq!(wire(&ns, "t")["shadows"][0], defaults);
    update(&mut ns, json!({"shadows": []})).unwrap();
    assert_eq!(wire(&ns, "t")["shadows"], json!([]));
}

#[test]
fn five_shadows_are_refused_and_the_list_stays() {
    let mut ns = base_doc();
    update(&mut ns, json!({"shadows": [{"blur": 1}]})).unwrap();
    let five: Vec<Value> = (0..5).map(|_| json!({"blur": 1})).collect();
    let e = update(&mut ns, json!({"shadows": five})).unwrap_err();
    assert!(e.to_string().contains("shadows"), "{e}");
    assert_eq!(wire(&ns, "t")["shadows"].as_array().unwrap().len(), 1);
}

#[test]
fn an_old_node_without_the_new_fields_is_valid_with_defaults() {
    // A node exactly as stored before typography and decoration existed.
    let old = r##"{"id":"a","name":"Card","type":"rect","parentId":null,"x":1,"y":2,"w":10,"h":20,"sw":"fixed","sh":"fixed","fill":"#111111","fill2":"#22d3ee","radius":4,"layout":"none","gap":8,"padding":16,"align":"start","justify":"start","clip":true,"text":"","fontSize":16,"shader":"aurora"}"##;
    let n: Node = serde_json::from_str(old).unwrap();
    meldframe_core::check_node(&n, &Limits::default()).unwrap();
    let v = serde_json::to_value(&n).unwrap();
    assert_eq!(v["fontWeight"], json!(400));
    assert_eq!(v["textAlign"], json!("left"));
    assert_eq!(v["border"]["width"], json!(0.0));
    assert_eq!(v["shadows"], json!([]));
    // Unknown fields are still refused.
    let unknown = old.replace(
        "\"fontSize\":16",
        "\"fontSize\":16,\"fontStyle\":\"italic\"",
    );
    assert!(serde_json::from_str::<Node>(&unknown).is_err());
}
