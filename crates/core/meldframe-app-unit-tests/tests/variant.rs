//! E5.4: a variant of a frame from the editor. `MakeVariant` copies the frame and its subtree beside the original,
//! selects the copy, and is one undo step.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Event};

fn frame_with_a_child(l: &mut Loopback) -> String {
    l.send(Action::CreatePreset {
        preset: meldframe_app::FramePreset::Mobile,
        x: 0.0,
        y: 0.0,
        parent: Some("root".into()),
    });
    let frame = l.app.state().selection[0].clone();
    l.send(Action::CreateNode {
        kind: meldframe_app::CreateKind::Rectangle,
        x: 10.0,
        y: 10.0,
        parent: Some(frame.clone()),
        w: Some(50.0),
        h: Some(20.0),
    });
    frame
}

#[test]
fn a_variant_copies_the_frame_beside_it_and_selects_the_copy() {
    let mut l = Loopback::live();
    let frame = frame_with_a_child(&mut l);
    let before = l.app.nodes().len();
    l.send(Action::MakeVariant { id: frame.clone() });
    assert_eq!(l.app.nodes().len(), before + 2, "the frame and its child");
    let copy = l.app.state().selection[0].clone();
    assert_ne!(copy, frame);
    let (o, c) = (get(&l.app, &frame), get(&l.app, &copy));
    assert_eq!((c.w, c.h, c.y), (o.w, o.h, o.y));
    assert!(c.x > o.x + o.w, "beside the original");
    assert_eq!(c.name, "Mobile variant");
    assert_eq!(
        l.app
            .nodes()
            .iter()
            .filter(|n| n.parent_id.as_deref() == Some(copy.as_str()))
            .count(),
        1
    );
}

#[test]
fn one_undo_takes_the_whole_variant_back() {
    let mut l = Loopback::live();
    let frame = frame_with_a_child(&mut l);
    let before = l.app.nodes().len();
    l.send(Action::MakeVariant { id: frame });
    l.send(Action::Undo {});
    assert_eq!(l.app.nodes().len(), before);
}

#[test]
fn a_variant_of_a_node_that_is_not_a_frame_is_refused_with_its_code() {
    let mut l = Loopback::live();
    let frame = frame_with_a_child(&mut l);
    let child = l.app.state().selection[0].clone();
    assert_ne!(child, frame);
    l.app.take_events();
    let before = l.app.nodes().len();
    l.app.dispatch(Action::MakeVariant { id: child });
    assert_eq!(l.app.nodes().len(), before);
    assert!(
        l.app
            .take_events()
            .iter()
            .any(|e| matches!(e, Event::Refused { code } if code == "not_a_frame"))
    );
}

#[test]
fn a_variant_of_a_missing_node_is_refused_as_not_found() {
    let mut l = Loopback::live();
    l.app.take_events();
    l.app.dispatch(Action::MakeVariant { id: "nope".into() });
    assert!(
        l.app
            .take_events()
            .iter()
            .any(|e| matches!(e, Event::Refused { code } if code == "not_found"))
    );
}
