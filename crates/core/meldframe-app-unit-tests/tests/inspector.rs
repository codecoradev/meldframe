//! The inspector's fields (D4): opacity and clip are read from the node and written with one undo step each; a
//! value out of range is refused and changes nothing.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_app::{Action, Event, Property};
use meldframe_protocol::TextAlign;

fn set(id: &str, property: Property) -> Action {
    Action::SetProperty {
        id: id.into(),
        property,
    }
}

#[test]
fn the_panel_reads_opacity_and_clip_of_the_selected_node() {
    let mut l = Loopback::live();
    l.send(Action::Select {
        ids: ids(&["root"]),
    });
    let p = l.app.state().properties.clone().unwrap();
    assert_eq!((p.opacity, p.clip), (1.0, true));
    assert!(p.bindings.is_empty());
}

#[test]
fn opacity_and_clip_are_set_one_undo_step_each() {
    let mut l = Loopback::live();
    l.send(Action::Select {
        ids: ids(&["root"]),
    });
    l.send(set("root", Property::Opacity(0.4)));
    l.send(set("root", Property::Clip(false)));
    let p = l.app.state().properties.clone().unwrap();
    assert_eq!((p.opacity, p.clip), (0.4, false));
    assert_eq!(l.app.state().history.undo_steps, 2);
    l.send(Action::Undo {});
    assert!(l.app.state().properties.as_ref().unwrap().clip);
    l.send(Action::Undo {});
    assert_eq!(l.app.state().properties.as_ref().unwrap().opacity, 1.0);
}

#[test]
fn an_opacity_out_of_range_is_refused_and_changes_nothing() {
    let mut l = Loopback::live();
    l.send(Action::Select { ids: ids(&["r1"]) });
    for v in [-0.1, 1.5] {
        l.send(set("r1", Property::Opacity(v)));
        assert_eq!(
            l.events
                .drain(..)
                .filter(|e| matches!(e, Event::Refused { .. }))
                .collect::<Vec<_>>(),
            vec![Event::Refused {
                code: "invalid".into()
            }]
        );
    }
    assert_eq!(l.app.state().history.undo_steps, 0);
    assert_eq!(l.app.state().properties.as_ref().unwrap().opacity, 1.0);
}

#[test]
fn clip_is_a_frame_property_and_is_refused_for_any_other_kind() {
    let mut l = Loopback::live();
    l.send(Action::Select { ids: ids(&["r1"]) });
    l.send(set("r1", Property::Clip(false)));
    assert_eq!(
        l.events
            .drain(..)
            .filter(|e| matches!(e, Event::Refused { .. }))
            .collect::<Vec<_>>(),
        vec![Event::Refused {
            code: "invalid".into()
        }]
    );
    assert_eq!(l.app.state().history.undo_steps, 0);
    assert!(l.app.state().properties.as_ref().unwrap().clip);
    l.send(set("root", Property::Clip(false)));
    assert!(!get(&l.app, "root").clip);
}

#[test]
fn the_panel_reads_weight_and_alignment_of_a_text_and_of_no_other_kind() {
    let mut l = Loopback::live();
    l.send(Action::Select { ids: ids(&["t1"]) });
    let p = l.app.state().properties.clone().unwrap();
    assert_eq!(
        (p.font_weight, p.text_align),
        (Some(400), Some(TextAlign::Left))
    );
    for id in ["r1", "root"] {
        l.send(Action::Select { ids: ids(&[id]) });
        let p = l.app.state().properties.clone().unwrap();
        assert_eq!((p.font_weight, p.text_align), (None, None), "{id}");
    }
}

#[test]
fn weight_and_alignment_are_set_one_undo_step_each_and_undo_brings_back_the_old_value() {
    let mut l = Loopback::live();
    l.send(Action::Select { ids: ids(&["t1"]) });
    l.send(set("t1", Property::FontWeight(700)));
    l.send(set("t1", Property::TextAlign(TextAlign::Center)));
    let p = l.app.state().properties.clone().unwrap();
    assert_eq!(
        (p.font_weight, p.text_align),
        (Some(700), Some(TextAlign::Center))
    );
    let n = get(&l.app, "t1");
    assert_eq!((n.font_weight, n.text_align), (700, TextAlign::Center));
    assert_eq!(l.app.state().history.undo_steps, 2);
    l.send(Action::Undo {});
    l.send(Action::Undo {});
    let n = get(&l.app, "t1");
    assert_eq!((n.font_weight, n.text_align), (400, TextAlign::Left));
}

#[test]
fn a_weight_the_model_does_not_take_is_refused_and_changes_nothing() {
    let mut l = Loopback::live();
    l.send(Action::Select { ids: ids(&["t1"]) });
    for bad in [0, 50, 450, 1000] {
        l.send(set("t1", Property::FontWeight(bad)));
    }
    let refused: Vec<_> = l
        .events
        .drain(..)
        .filter(|e| matches!(e, Event::Refused { .. }))
        .collect();
    assert_eq!(
        refused,
        vec![
            Event::Refused {
                code: "invalid".into()
            };
            4
        ]
    );
    assert_eq!(get(&l.app, "t1").font_weight, 400);
    assert_eq!(l.app.state().history.undo_steps, 0);
}

#[test]
fn weight_and_alignment_are_text_properties_and_are_refused_for_any_other_kind() {
    let mut l = Loopback::live();
    for id in ["r1", "root"] {
        l.send(Action::Select { ids: ids(&[id]) });
        l.send(set(id, Property::FontWeight(700)));
        l.send(set(id, Property::TextAlign(TextAlign::Right)));
    }
    let refused: Vec<_> = l
        .events
        .drain(..)
        .filter(|e| matches!(e, Event::Refused { .. }))
        .collect();
    assert_eq!(
        refused,
        vec![
            Event::Refused {
                code: "invalid".into()
            };
            4
        ]
    );
    assert_eq!(get(&l.app, "r1").font_weight, 400);
    assert_eq!(l.app.state().history.undo_steps, 0);
}
