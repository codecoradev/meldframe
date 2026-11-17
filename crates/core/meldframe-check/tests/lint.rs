//! The layer-name lint: a layer keeps the name a tool gave it (`Frame 12`, `Rectangle`, `Text`) instead of a name
//! that says what it is. The lint only reports; nothing is refused.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{default_name_findings, is_default_layer_name};
use meldframe_protocol::Node;

fn node(id: &str, name: &str) -> Node {
    serde_json::from_value(serde_json::json!({"id": id, "type": "frame", "name": name})).unwrap()
}

#[test]
fn flags_the_names_a_tool_gives_a_layer() {
    for name in [
        "Frame",
        "Frame 1",
        "frame 12",
        "FRAME 007",
        "Rectangle",
        "Rectangle 3",
        "Rect",
        "Ellipse 2",
        "Text",
        "Text 4",
        "Group",
        "Group 2",
        "Layer",
        "Layer 9",
        "Shader",
        "Vector",
        "Image",
        "Untitled",
        "Untitled 2",
        "Auto layout",
        "Component 1",
        "  Frame  5 ",
    ] {
        assert!(is_default_layer_name(name), "{name:?} is a default name");
    }
}

#[test]
fn flags_an_empty_name() {
    assert!(is_default_layer_name(""));
    assert!(is_default_layer_name("   "));
}

#[test]
fn leaves_a_name_that_says_what_the_layer_is() {
    for name in [
        "Hero",
        "Hero heading",
        "Pricing card",
        "CTA button",
        "Frame 3 header",
        "Text block",
        "Image gallery",
        "Nav bar 2",
        "Frames",
    ] {
        assert!(!is_default_layer_name(name), "{name:?} is a real name");
    }
}

#[test]
fn a_name_that_is_only_the_id_is_flagged() {
    let findings = default_name_findings(&[node("n_8f3a", "n_8f3a")]);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, "n_8f3a");
}

#[test]
fn reports_each_flagged_node_once_with_its_id_and_name_in_the_order_given() {
    let nodes = [
        node("a", "Hero"),
        node("b", "Frame 2"),
        node("c", "Pricing"),
        node("d", "Text"),
    ];
    let findings = default_name_findings(&nodes);
    let got: Vec<(&str, &str)> = findings
        .iter()
        .map(|f| (f.id.as_str(), f.name.as_str()))
        .collect();
    assert_eq!(got, vec![("b", "Frame 2"), ("d", "Text")]);
}

#[test]
fn a_file_of_semantic_names_has_no_findings() {
    assert!(default_name_findings(&[node("a", "Hero"), node("b", "Hero heading")]).is_empty());
}

#[test]
fn the_warnings_of_a_big_file_are_bounded() {
    let nodes: Vec<Node> = (0..500).map(|i| node(&format!("n{i}"), "Frame")).collect();
    let w = meldframe_check::name_warnings_of_nodes(&nodes);
    assert_eq!(w.len(), meldframe_check::MAX_NAME_WARNINGS);
    assert_eq!(w[0]["nodeId"], "n0", "the first ones, in file order");
}
