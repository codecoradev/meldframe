//! Validation of font weight, border and shadows (moved from meldframe-check, whose size ceiling it counted against). Every refusal names the field it refused.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{ApplyError, Limits, check_node, check_patch};
use meldframe_protocol::{Node, NodePatch};
use serde_json::{Value, json};

fn node(extra: Value) -> Node {
    let mut v = json!({"id": "n", "type": "rect"});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn patch(v: Value) -> NodePatch {
    serde_json::from_value(v).unwrap()
}

fn message(r: Result<(), ApplyError>) -> String {
    r.expect_err("must be refused").to_string()
}

#[test]
fn font_weight_must_be_100_to_900_in_steps_of_100() {
    let l = Limits::default();
    for bad in [0, 50, 99, 450, 1000, 901] {
        let e = check_patch(&patch(json!({"fontWeight": bad})), &l);
        assert_eq!(e, Err(ApplyError::InvalidFontWeight(bad)), "{bad}");
        assert!(message(e).contains("fontWeight"));
        let e = check_node(&node(json!({"fontWeight": bad})), &l);
        assert_eq!(e, Err(ApplyError::InvalidFontWeight(bad)), "{bad}");
    }
    for ok in [100, 400, 500, 900] {
        assert_eq!(check_patch(&patch(json!({"fontWeight": ok})), &l), Ok(()));
        assert_eq!(check_node(&node(json!({"fontWeight": ok})), &l), Ok(()));
    }
}

#[test]
fn a_default_node_has_valid_typography_and_decoration() {
    assert_eq!(check_node(&node(json!({})), &Limits::default()), Ok(()));
}

#[test]
fn border_width_and_colour_are_validated() {
    let l = Limits::default();
    let cases = [
        (json!({"width": -1}), "border.width"),
        (json!({"width": 100001}), "border.width"),
        (json!({"width": 2, "color": "red"}), "border.color"),
        (json!({"color": "#12"}), "border.color"),
    ];
    for (border, field) in cases {
        let e = message(check_patch(&patch(json!({"border": border.clone()})), &l));
        assert!(e.contains(field), "{border}: {e}");
        let e = message(check_node(&node(json!({"border": border.clone()})), &l));
        assert!(e.contains(field), "{border}: {e}");
    }
    let ok = json!({"width": 3, "color": "#ff0000", "position": "outside"});
    assert_eq!(check_patch(&patch(json!({"border": ok})), &l), Ok(()));
    assert_eq!(
        check_patch(&patch(json!({"border": {"width": 0}})), &l),
        Ok(())
    );
    assert_eq!(
        check_patch(&patch(json!({"border": {"width": 100000}})), &l),
        Ok(())
    );
}

#[test]
fn shadow_values_are_validated_and_name_the_field() {
    let l = Limits::default();
    let cases = [
        (json!({"blur": -1}), "shadows.blur"),
        (json!({"blur": 100001}), "shadows.blur"),
        (json!({"x": 100001}), "shadows.x"),
        (json!({"y": -100001}), "shadows.y"),
        (json!({"spread": 100001}), "shadows.spread"),
        (json!({"color": "black"}), "shadows.color"),
    ];
    for (shadow, field) in cases {
        let e = message(check_patch(
            &patch(json!({"shadows": [shadow.clone()]})),
            &l,
        ));
        assert!(e.contains(field), "{shadow}: {e}");
        let e = message(check_node(&node(json!({"shadows": [shadow.clone()]})), &l));
        assert!(e.contains(field), "{shadow}: {e}");
    }
    let ok = json!({"x": -5, "y": 4, "blur": 8, "spread": -1, "color": "#00000080"});
    assert_eq!(check_patch(&patch(json!({"shadows": [ok]})), &l), Ok(()));
}

#[test]
fn at_most_four_shadows_by_default_and_the_limit_is_configurable() {
    let l = Limits::default();
    let list = |n: usize| (0..n).map(|_| json!({"blur": 1})).collect::<Vec<_>>();
    assert_eq!(check_patch(&patch(json!({"shadows": list(4)})), &l), Ok(()));
    assert_eq!(check_patch(&patch(json!({"shadows": []})), &l), Ok(()));
    let five = check_patch(&patch(json!({"shadows": list(5)})), &l);
    assert_eq!(
        five,
        Err(ApplyError::TooMany {
            field: "shadows",
            max: 4
        })
    );
    assert!(message(five).contains("shadows"));
    assert!(check_node(&node(json!({"shadows": list(5)})), &l).is_err());
    let tight = Limits {
        max_shadows: 1,
        ..Limits::default()
    };
    assert!(check_patch(&patch(json!({"shadows": list(2)})), &tight).is_err());
}

#[test]
fn unknown_decoration_members_do_not_parse() {
    assert!(
        serde_json::from_value::<NodePatch>(json!({"border": {"width": 1, "dash": 2}})).is_err()
    );
    assert!(
        serde_json::from_value::<NodePatch>(json!({"border": {"position": "middle"}})).is_err()
    );
    assert!(serde_json::from_value::<NodePatch>(json!({"shadows": [{"inset": true}]})).is_err());
    assert!(serde_json::from_value::<NodePatch>(json!({"textAlign": "justify"})).is_err());
}
