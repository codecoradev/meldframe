//! Validation of the image fill: the asset id must be a lowercase SHA-256 hex string, and text
//! nodes take no image. Whether the asset exists is the server's business, not the core's.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{ApplyError, Limits, check_command, check_node, check_patch};
use meldframe_protocol::{Command, Node, NodePatch};
use serde_json::{Value, json};

const ID_A: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn node(kind: &str, image: Value) -> Node {
    serde_json::from_value(json!({"id": "n", "type": kind, "image": image})).unwrap()
}

fn patch(image: Value) -> NodePatch {
    serde_json::from_value(json!({"image": image})).unwrap()
}

fn bad_ids() -> Vec<String> {
    vec![
        String::new(),
        "abc".into(),
        ID_A[..63].into(),             // too short
        format!("{ID_A}0"),            // too long
        ID_A.to_uppercase(),           // upper case is not the canonical form
        format!("{}g", &ID_A[..63]),   // not hex
        format!("../{}", &ID_A[..61]), // path traversal attempt
    ]
}

#[test]
fn a_canonical_asset_id_is_accepted_in_a_node_and_a_patch() {
    let l = Limits::default();
    assert_eq!(
        check_node(&node("rect", json!({"assetId": ID_A})), &l),
        Ok(())
    );
    assert_eq!(
        check_patch(&patch(json!({"assetId": ID_A, "fit": "contain"})), &l),
        Ok(())
    );
}

#[test]
fn a_malformed_asset_id_is_refused_and_names_the_field() {
    let l = Limits::default();
    for bad in bad_ids() {
        let e = check_node(&node("rect", json!({"assetId": bad})), &l);
        assert_eq!(e, Err(ApplyError::InvalidAssetId), "{bad:?}");
        let e = check_patch(&patch(json!({"assetId": bad})), &l).unwrap_err();
        assert_eq!(e, ApplyError::InvalidAssetId, "{bad:?}");
        assert!(e.to_string().contains("image.assetId"), "{e}");
    }
}

#[test]
fn a_null_image_in_a_patch_is_always_fine() {
    assert_eq!(check_patch(&patch(Value::Null), &Limits::default()), Ok(()));
}

#[test]
fn image_is_allowed_on_frame_rect_ellipse_and_shader_but_not_text() {
    let l = Limits::default();
    let image = json!({"assetId": ID_A});
    for kind in ["frame", "rect", "ellipse", "shader"] {
        assert_eq!(check_node(&node(kind, image.clone()), &l), Ok(()), "{kind}");
    }
    let e = check_node(&node("text", image), &l).unwrap_err();
    assert_eq!(e, ApplyError::ImageOnText);
    assert!(e.to_string().contains("image"), "{e}");
}

#[test]
fn an_update_that_sets_an_image_on_a_text_node_is_refused() {
    let l = Limits::default();
    let text: Node = serde_json::from_value(json!({"id": "t", "type": "text"})).unwrap();
    let nodes = [text];
    let set = |image: Value| Command::UpdateNode {
        id: "t".into(),
        patch: patch(image),
    };
    let e = check_command(&nodes, &set(json!({"assetId": ID_A})), &l);
    assert_eq!(e, Err(ApplyError::ImageOnText));
    // Clearing an image on a text node is harmless.
    assert_eq!(check_command(&nodes, &set(Value::Null), &l), Ok(()));
}

#[test]
fn unknown_image_members_and_fits_do_not_parse() {
    let parse = |image: Value| serde_json::from_value::<NodePatch>(json!({"image": image}));
    assert!(parse(json!({"assetId": ID_A, "fit": "stretch"})).is_err());
    assert!(parse(json!({"assetId": ID_A, "url": "http://x"})).is_err());
}
