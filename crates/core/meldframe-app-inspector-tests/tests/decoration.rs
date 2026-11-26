//! The Border and Shadow sections of the inspector (D5): one `UpdateNode` per edit, so one undo step. The ranges and the
//! colours are the core's checks (a refusal is `invalid`); the app layer owns the list rules (the limit of four, the
//! indexes) and the default of a new shadow.

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, Event, Property};
use meldframe_protocol::{BorderPosition, Shadow};

fn set(id: &str, property: Property) -> Action {
    Action::SetProperty {
        id: id.into(),
        property,
    }
}

fn props(l: &Loopback) -> meldframe_app::Properties {
    l.app.state().properties.clone().unwrap()
}

fn refused(l: &mut Loopback) -> Vec<String> {
    l.events
        .drain(..)
        .filter_map(|e| match e {
            Event::Refused { code } => Some(code),
            _ => None,
        })
        .collect()
}

fn open(l: &mut Loopback) {
    l.send(Action::Select { ids: ids(&["r1"]) });
}

#[test]
fn a_node_starts_without_border_or_shadow() {
    let mut l = Loopback::live();
    open(&mut l);
    let p = props(&l);
    assert_eq!(p.border.width, 0.0);
    assert!(p.shadows.is_empty());
    assert_eq!(p.max_shadows, 4);
}

#[test]
fn the_border_is_set_a_field_at_a_time_and_each_is_one_undo_step() {
    let mut l = Loopback::live();
    open(&mut l);
    let steps = l.app.state().history.undo_steps;
    l.send(set("r1", Property::BorderWidth(2.0)));
    l.send(set("r1", Property::BorderColor("#ff0000".into())));
    l.send(set("r1", Property::BorderPosition(BorderPosition::Outside)));
    let b = props(&l).border;
    assert_eq!(
        (b.width, b.color.as_str(), b.position),
        (2.0, "#ff0000", BorderPosition::Outside)
    );
    assert_eq!(l.app.state().history.undo_steps, steps + 3);
    l.send(Action::Undo {});
    assert_eq!(props(&l).border.position, BorderPosition::Inside);
    assert_eq!(props(&l).border.width, 2.0);
}

#[test]
fn a_bad_border_is_refused_by_the_core_and_changes_nothing() {
    let mut l = Loopback::live();
    open(&mut l);
    l.send(set("r1", Property::BorderWidth(-1.0)));
    l.send(set("r1", Property::BorderColor("red".into())));
    assert_eq!(refused(&mut l), vec!["invalid", "E_INVALID_COLOR"]);
    assert_eq!(props(&l).border.width, 0.0);
}

#[test]
fn shadows_are_added_edited_and_removed_up_to_four() {
    let mut l = Loopback::live();
    open(&mut l);
    l.send(set("r1", Property::AddShadow {}));
    let s = props(&l).shadows;
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].x, s[0].y, s[0].blur), (0.0, 4.0, 8.0));
    let wide = Shadow {
        x: 2.0,
        y: 6.0,
        blur: 20.0,
        spread: -2.0,
        color: "#11223380".into(),
    };
    l.send(set(
        "r1",
        Property::SetShadow {
            index: 0,
            shadow: wide.clone(),
        },
    ));
    assert_eq!(props(&l).shadows, vec![wide]);
    for _ in 0..3 {
        l.send(set("r1", Property::AddShadow {}));
    }
    assert_eq!(props(&l).shadows.len(), 4);
    l.send(set("r1", Property::AddShadow {}));
    assert_eq!(refused(&mut l), vec!["limit"]);
    l.send(set("r1", Property::RemoveShadow(0)));
    assert_eq!(props(&l).shadows.len(), 3);
}

#[test]
fn a_shadow_that_is_not_there_or_not_valid_is_refused() {
    let mut l = Loopback::live();
    open(&mut l);
    l.send(set("r1", Property::RemoveShadow(0)));
    let nothing = Shadow {
        x: 0.0,
        y: 0.0,
        blur: 0.0,
        spread: 0.0,
        color: "#000000".into(),
    };
    l.send(set(
        "r1",
        Property::SetShadow {
            index: 3,
            shadow: nothing,
        },
    ));
    assert_eq!(refused(&mut l), vec!["not_found", "not_found"]);
    l.send(set("r1", Property::AddShadow {}));
    let negative_blur = Shadow {
        blur: -1.0,
        ..props(&l).shadows[0].clone()
    };
    l.send(set(
        "r1",
        Property::SetShadow {
            index: 0,
            shadow: negative_blur,
        },
    ));
    assert_eq!(refused(&mut l), vec!["invalid"]);
    let not_a_colour = Shadow {
        color: "blue".into(),
        ..props(&l).shadows[0].clone()
    };
    l.send(set(
        "r1",
        Property::SetShadow {
            index: 0,
            shadow: not_a_colour,
        },
    ));
    assert_eq!(refused(&mut l), vec!["E_INVALID_COLOR"]);
    assert_eq!(props(&l).shadows[0].blur, 8.0);
}
