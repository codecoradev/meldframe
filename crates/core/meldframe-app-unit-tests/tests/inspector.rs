//! The inspector's fields (D4): opacity and clip are read from the node and written with one undo step each; a
//! value out of range is refused and changes nothing.

mod common;
use common::*;
use meldframe_app::{Action, Event, Property};

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
