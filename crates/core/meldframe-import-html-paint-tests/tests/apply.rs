//! What the structure stage uses: a paint put on a Node, the placeholder, and what makes a box decorated.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::paint::{Asset, Paint};
use meldframe_protocol::{Bind, Node};
use serde_json::json;

fn node() -> Node {
    serde_json::from_value(json!({"id": "n", "type": "frame"})).unwrap()
}

#[test]
fn applying_the_default_paint_replaces_the_defaults_of_the_node() {
    // A Node that leaves its fill out is grey, which is not the CSS initial value.
    let mut n = node();
    assert_eq!(n.fill, Bind::Lit("#3f3f46".into()));
    Paint::default().apply(&mut n);
    assert_eq!(n.fill, Bind::Lit("none".into()));
    assert_eq!(n.border.width, 0.0);
    assert!(n.shadows.is_empty() && n.gradient.is_none() && n.image.is_none());
}

#[test]
fn applying_writes_every_paint_field() {
    let r = read(
        "background-color:#0f0;background-image:linear-gradient(red,blue);border-radius:8px;box-shadow:1px 2px red;opacity:.5;border:2px solid blue",
    );
    let mut n = node();
    r.paint.apply(&mut n);
    assert_eq!(n.fill, Bind::Lit("#0f0".into()));
    assert!(n.gradient.is_some());
    assert_eq!(n.radius, Bind::Lit(8.0));
    assert_eq!(n.shadows.len(), 1);
    assert_eq!(n.opacity, Bind::Lit(0.5));
    assert_eq!((n.border.width, n.border.color.as_str()), (2.0, "#0000ff"));
    // The result is a Node the protocol accepts as it is.
    let again: Node = serde_json::from_value(serde_json::to_value(&n).unwrap()).unwrap();
    assert_eq!(again.fill, n.fill);
}

#[test]
fn a_placeholder_paints_nothing_and_keeps_its_src() {
    let p = Paint::placeholder(Some("photo.png"));
    assert_eq!(
        p.assets,
        [Asset {
            src: "photo.png".into()
        }]
    );
    assert!(!p.decorates());
    assert_eq!(Paint::placeholder(None), Paint::default());
}

#[test]
fn a_box_is_decorated_by_a_colour_a_gradient_an_image_a_border_or_a_shadow() {
    for (decls, want) in [
        ("", false),
        ("border-radius:8px", false),
        ("opacity:.5", false),
        ("background-color:transparent", false),
        ("background-color:#f00", true),
        ("background-image:linear-gradient(red,blue)", true),
        ("border:1px solid red", true),
        ("box-shadow:0 1px red", true),
    ] {
        assert_eq!(read(decls).paint.decorates(), want, "{decls}");
    }
}
