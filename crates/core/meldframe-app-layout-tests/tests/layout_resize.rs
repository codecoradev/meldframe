//! A resize in a laid-out file makes a hug or fill side fixed, and the properties show the laid-out box. With the flag
//! off a resize writes the size and the origin and the properties are the stored box.

// Under WASM the same tests run through wasm-bindgen-test (`just wasm-test`).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod layout_fixture;
use layout_fixture::*;
use meldframe_app::Action;
use meldframe_protocol::SizeMode;
use serde_json::json;

// --- resize makes the side fixed ----------------------------------------------------------------------------------

#[test]
fn a_resize_of_a_fill_width_makes_the_width_fixed_and_leaves_the_other_side() {
    let mut a = app_of(with_sized_b("fill"), true);
    assert_eq!(box_of(&a, "b").2, 180.0);
    // Wider by 10; x and y are not written for a child of the flow (the layout ignores them).
    a.dispatch(Action::Resize {
        id: "b".into(),
        w: 190.0,
        h: 30.0,
        x: Some(-4.0),
        y: Some(-4.0),
    });
    let b = node(&a, "b");
    assert_eq!((b.sw, b.w), (SizeMode::Fixed, 190.0));
    assert_eq!(b.sh, SizeMode::Hug, "the height did not change");
    assert_eq!(
        (b.x, b.y),
        (0.0, 0.0),
        "the position of a flow child is left alone"
    );
    assert_eq!(steps(&a), 1, "size and mode in one step");
    a.dispatch(Action::Undo {});
    assert_eq!(node(&a, "b").sw, SizeMode::Fill);
}

#[test]
fn a_resize_fixes_only_the_side_that_changed() {
    let mut a = app_of(with_sized_b("fill"), true);
    a.dispatch(Action::resize("b", 180.0, 50.0));
    let b = node(&a, "b");
    assert_eq!((b.sw, b.sh), (SizeMode::Fill, SizeMode::Fixed));
    assert_eq!(b.h, 50.0);
}

#[test]
fn with_the_flag_off_a_resize_writes_the_size_and_nothing_else() {
    let mut a = app_of(with_sized_b("fill"), false);
    a.dispatch(Action::Resize {
        id: "b".into(),
        w: 190.0,
        h: 30.0,
        x: Some(-4.0),
        y: None,
    });
    let b = node(&a, "b");
    assert_eq!(
        (b.sw, b.sh, b.w, b.x),
        (SizeMode::Fill, SizeMode::Hug, 190.0, -4.0)
    );
}

#[test]
fn a_resize_of_a_free_node_still_moves_its_origin() {
    let mut a = app(true);
    a.dispatch(Action::Resize {
        id: "fg".into(),
        w: 60.0,
        h: 30.0,
        x: Some(10.0),
        y: None,
    });
    let fg = node(&a, "fg");
    assert_eq!((fg.x, fg.y, fg.w), (10.0, 20.0, 60.0));
}

#[test]
fn a_resize_of_an_absolute_child_of_the_flow_still_writes_its_origin() {
    let mut a = app(true);
    a.dispatch(Action::Resize {
        id: "abs".into(),
        w: 30.0,
        h: 20.0,
        x: Some(140.0),
        y: Some(190.0),
    });
    let abs = node(&a, "abs");
    assert_eq!((abs.x, abs.y, abs.w), (140.0, 190.0, 30.0));
}

#[test]
fn a_resize_of_hug_text_fixes_the_side_that_changed_and_only_that_one() {
    let mut list = nodes();
    list.push(
        serde_json::from_value(json!({
            "id": "t", "type": "text", "parentId": "f", "pos": "a4", "text": "hello", "fontSize": 16,
            "sw": "hug", "sh": "hug"
        }))
        .unwrap(),
    );
    let mut a = app_of(list, true);
    // The stub measures "hello" at 16 px as 40 x 20.
    let (_, _, w, h) = box_of(&a, "t");
    assert_eq!((w, h), (40.0, 20.0));
    a.dispatch(Action::resize("t", 60.0, 20.0));
    let t = node(&a, "t");
    assert_eq!(
        (t.sw, t.sh),
        (SizeMode::Fixed, SizeMode::Hug),
        "only the width changed"
    );
    a.dispatch(Action::resize("t", 60.0, 35.0));
    let t = node(&a, "t");
    assert_eq!((t.sw, t.sh, t.h), (SizeMode::Fixed, SizeMode::Fixed, 35.0));
}

// --- the properties show the laid-out box -------------------------------------------------------------------------

#[test]
fn the_properties_show_the_laid_out_box_and_lock_the_position_of_a_flow_child() {
    let mut a = app(true);
    let p = properties_of(&mut a, "b");
    assert_eq!((p.x, p.y, p.w, p.h), (10.0, 35.0, 50.0, 30.0));
    assert!(p.position_locked);
    // A free node, an absolute one and a frame on the page keep their own values and stay editable.
    let p = properties_of(&mut a, "fg");
    assert_eq!((p.x, p.y, p.position_locked), (20.0, 20.0, false));
    let p = properties_of(&mut a, "abs");
    assert_eq!((p.x, p.y, p.position_locked), (150.0, 200.0, false));
    let p = properties_of(&mut a, "r");
    assert_eq!((p.x, p.y, p.position_locked), (700.0, 0.0, false));
}

#[test]
fn the_properties_of_a_fill_node_show_the_size_it_is_drawn_at() {
    let mut a = app_of(with_sized_b("fill"), true);
    let p = properties_of(&mut a, "b");
    assert_eq!((p.w, p.h), (180.0, 30.0));
}

#[test]
fn with_the_flag_off_the_properties_are_the_stored_box() {
    let mut a = app(false);
    let p = properties_of(&mut a, "b");
    assert_eq!(
        (p.x, p.y, p.w, p.h, p.position_locked),
        (0.0, 0.0, 50.0, 30.0, false)
    );
}

#[test]
fn turning_the_flag_on_brings_the_properties_of_the_selection_along() {
    let mut a = app(false);
    a.dispatch(Action::Select {
        ids: vec!["b".into()],
    });
    a.set_layout_on_canvas(true);
    let p = a.state().properties.clone().unwrap();
    assert_eq!((p.y, p.position_locked), (35.0, true));
    a.set_layout_on_canvas(false);
    assert_eq!(a.state().properties.clone().unwrap().y, 0.0);
}
