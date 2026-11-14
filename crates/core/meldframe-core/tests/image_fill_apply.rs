//! The image fill through the core's submit-a-command interface: `Node.image`, and the three-state
//! patch `NodePatch.image` (key absent = unchanged, `null` = remove, object = set).

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_core::{Limits, apply_command};
use meldframe_protocol::{Command, Node};
use serde_json::{Value, json};

const ID_A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn cmd(json: &str) -> Command {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid command JSON: {e}: {json}"))
}

fn nodes_json(nodes: &[Node]) -> Value {
    serde_json::to_value(nodes).unwrap()
}

fn update(patch: &str) -> Command {
    cmd(&format!(
        r#"{{"type":"updateNode","id":"r","patch":{patch}}}"#
    ))
}

fn plain_rect() -> Vec<Node> {
    let mut nodes = vec![];
    let add = cmd(r#"{"type":"addNode","node":{"id":"r","type":"rect"}}"#);
    apply_command(&mut nodes, &add, &Limits::default()).unwrap();
    nodes
}

#[test]
fn adding_a_node_with_an_image_stores_it_and_round_trips_json() {
    let mut nodes = vec![];
    let image = format!(r#"{{"assetId":"{ID_A}","fit":"contain"}}"#);
    let add = cmd(&format!(
        r#"{{"type":"addNode","node":{{"id":"r","type":"rect","image":{image}}}}}"#
    ));
    apply_command(&mut nodes, &add, &Limits::default()).unwrap();
    let j = nodes_json(&nodes);
    assert_eq!(j[0]["image"], json!({"assetId": ID_A, "fit": "contain"}));
    let back: Vec<Node> = serde_json::from_value(j.clone()).unwrap();
    assert_eq!(nodes_json(&back), j);
}

#[test]
fn a_patch_object_sets_null_clears_and_an_absent_key_leaves_it() {
    let l = Limits::default();
    let mut nodes = plain_rect();
    // An object sets it; a fit left out is cover.
    apply_command(
        &mut nodes,
        &update(&format!(r#"{{"image":{{"assetId":"{ID_A}"}}}}"#)),
        &l,
    )
    .unwrap();
    assert_eq!(
        nodes_json(&nodes)[0]["image"],
        json!({"assetId": ID_A, "fit": "cover"})
    );
    // An absent key changes nothing.
    apply_command(&mut nodes, &update(r#"{"x":5}"#), &l).unwrap();
    assert_eq!(nodes_json(&nodes)[0]["image"]["assetId"], ID_A);
    // null removes it: the key disappears from the node JSON.
    apply_command(&mut nodes, &update(r#"{"image":null}"#), &l).unwrap();
    assert!(nodes_json(&nodes)[0].get("image").is_none());
    // A null patch stays null when serialised; an empty patch stays empty.
    let null_patch = serde_json::to_value(update(r#"{"image":null}"#)).unwrap();
    assert_eq!(null_patch["patch"], json!({"image": null}));
    let empty_patch = serde_json::to_value(update("{}")).unwrap();
    assert_eq!(empty_patch["patch"], json!({}));
}

#[test]
fn a_refused_image_leaves_the_nodes_unchanged() {
    let l = Limits::default();
    let mut nodes = plain_rect();
    let before = nodes_json(&nodes);
    let bad = update(&format!(r#"{{"image":{{"assetId":"{}"}}}}"#, &ID_A[..63]));
    let e = apply_command(&mut nodes, &bad, &l).unwrap_err();
    assert!(e.to_string().contains("image.assetId"), "{e}");
    let add =
        cmd(r#"{"type":"addNode","node":{"id":"n2","type":"rect","image":{"assetId":"abc"}}}"#);
    assert!(apply_command(&mut nodes, &add, &l).is_err());
    assert_eq!(nodes_json(&nodes), before);
}

#[test]
fn text_nodes_take_no_image_but_may_clear_one() {
    let l = Limits::default();
    let mut nodes = vec![];
    let image = format!(r#"{{"assetId":"{ID_A}"}}"#);
    let add_text_image = cmd(&format!(
        r#"{{"type":"addNode","node":{{"id":"t","type":"text","image":{image}}}}}"#
    ));
    assert!(apply_command(&mut nodes, &add_text_image, &l).is_err());
    assert!(nodes.is_empty());
    apply_command(
        &mut nodes,
        &cmd(r#"{"type":"addNode","node":{"id":"t","type":"text"}}"#),
        &l,
    )
    .unwrap();
    let set = cmd(&format!(
        r#"{{"type":"updateNode","id":"t","patch":{{"image":{image}}}}}"#
    ));
    assert!(apply_command(&mut nodes, &set, &l).is_err());
    assert!(nodes_json(&nodes)[0].get("image").is_none());
    let clear = cmd(r#"{"type":"updateNode","id":"t","patch":{"image":null}}"#);
    apply_command(&mut nodes, &clear, &l).unwrap();
}

#[test]
fn an_old_file_without_image_is_valid_and_serialises_without_the_key() {
    let old = r##"[{"id":"a","name":"old","type":"rect","parentId":null,"x":1,"y":2,"w":3,"h":4,"fill":"#112233"}]"##;
    let nodes: Vec<Node> = serde_json::from_str(old).unwrap();
    assert!(nodes_json(&nodes)[0].get("image").is_none());
    assert!(meldframe_core::check_node(&nodes[0], &Limits::default()).is_ok());
}
