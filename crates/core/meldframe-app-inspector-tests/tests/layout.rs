//! The layout fields of the inspector (D6): layout, gap, padding, margin, sizing, alignment and absolute. Every edit is
//! one undo step. The app layer also says which settings have no effect where the node is (`notes`): the rules are those
//! of the fidelity contract, so the panel never words a rule of its own.

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, Axis, Event, LayoutNote, Property, Side};
use meldframe_protocol::{Align, Justify, LayoutMode, SizeMode};

fn set(id: &str, property: Property) -> Action {
    Action::SetProperty {
        id: id.into(),
        property,
    }
}

fn props(l: &Loopback) -> meldframe_app::Properties {
    l.app.state().properties.clone().unwrap()
}

fn open(l: &mut Loopback, id: &str) {
    l.send(Action::Select { ids: ids(&[id]) });
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

#[test]
fn the_layout_of_a_frame_and_whether_its_children_are_in_it() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    assert_eq!(props(&l).layout, LayoutMode::None);
    l.send(set("root", Property::Layout(LayoutMode::Row)));
    l.send(set("root", Property::Gap(12.0)));
    l.send(set("root", Property::Align(Align::Center)));
    l.send(set("root", Property::Justify(Justify::Between)));
    let p = props(&l);
    assert_eq!(
        (p.layout, p.gap, p.align, p.justify),
        (LayoutMode::Row, 12.0, Align::Center, Justify::Between)
    );
    assert!(!p.in_layout);
    open(&mut l, "r1");
    assert!(props(&l).in_layout);
    open(&mut l, "root");
    l.send(set("root", Property::Layout(LayoutMode::None)));
    open(&mut l, "r1");
    assert!(!props(&l).in_layout);
}

#[test]
fn padding_is_set_by_side_or_for_all_sides_and_each_edit_is_one_step() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    let steps = l.app.state().history.undo_steps;
    l.send(set(
        "root",
        Property::Padding {
            side: Side::Left,
            value: 7.0,
        },
    ));
    let p = props(&l).padding;
    assert_eq!(p.left, 7.0);
    assert_eq!(l.app.state().history.undo_steps, steps + 1);
    l.send(set("root", Property::PaddingAll(5.0)));
    let p = props(&l).padding;
    assert_eq!((p.top, p.right, p.bottom, p.left), (5.0, 5.0, 5.0, 5.0));
    let n = get(&l.app, "root");
    assert!(n.pad_l.is_none() && n.padding == 5.0);
    l.send(Action::Undo {});
    assert_eq!(props(&l).padding.left, 7.0);
}

#[test]
fn the_panel_is_told_the_padding_the_four_sides_share_and_none_when_they_differ() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    assert_eq!(props(&l).padding_uniform, Some(get(&l.app, "root").padding));
    l.send(set("root", Property::PaddingAll(5.0)));
    assert_eq!(props(&l).padding_uniform, Some(5.0));
    l.send(set(
        "root",
        Property::Padding {
            side: Side::Left,
            value: 7.0,
        },
    ));
    assert_eq!(props(&l).padding_uniform, None);
}

#[test]
fn margin_sizing_and_absolute_are_set_on_the_node() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set(
        "r1",
        Property::Margin {
            side: Side::Top,
            value: 9.0,
        },
    ));
    l.send(set(
        "r1",
        Property::Sizing {
            axis: Axis::Height,
            mode: SizeMode::Fill,
        },
    ));
    l.send(set("r1", Property::Absolute(true)));
    let p = props(&l);
    assert_eq!(p.margin.top, 9.0);
    assert_eq!((p.sw, p.sh), (SizeMode::Fixed, SizeMode::Fill));
    assert!(p.absolute);
}

#[test]
fn a_value_the_core_does_not_take_is_refused_and_changes_nothing() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    l.send(set("root", Property::Gap(-1.0)));
    l.send(set("root", Property::PaddingAll(f64::NAN)));
    l.send(set(
        "root",
        Property::Padding {
            side: Side::Top,
            value: -4.0,
        },
    ));
    assert_eq!(refused(&mut l), vec!["invalid", "invalid", "invalid"]);
    assert_eq!(props(&l).padding, get_padding(&l));
}

fn get_padding(l: &Loopback) -> meldframe_app::Sides {
    let n = get(&l.app, "root").pad();
    meldframe_app::Sides {
        top: n.top,
        right: n.right,
        bottom: n.bottom,
        left: n.left,
    }
}

#[test]
fn fill_without_an_auto_layout_parent_or_under_a_hugging_one_is_a_note() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set(
        "r1",
        Property::Sizing {
            axis: Axis::Width,
            mode: SizeMode::Fill,
        },
    ));
    assert_eq!(
        props(&l).notes,
        vec![LayoutNote::FillUnderHug { axis: Axis::Width }]
    );
    open(&mut l, "root");
    l.send(set("root", Property::Layout(LayoutMode::Row)));
    open(&mut l, "r1");
    assert!(props(&l).notes.is_empty());
    open(&mut l, "root");
    l.send(set(
        "root",
        Property::Sizing {
            axis: Axis::Width,
            mode: SizeMode::Hug,
        },
    ));
    open(&mut l, "r1");
    assert_eq!(
        props(&l).notes,
        vec![LayoutNote::FillUnderHug { axis: Axis::Width }]
    );
}

#[test]
fn hug_on_a_rectangle_margin_and_absolute_outside_a_layout_are_notes_and_text_may_hug() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set(
        "r1",
        Property::Sizing {
            axis: Axis::Height,
            mode: SizeMode::Hug,
        },
    ));
    l.send(set(
        "r1",
        Property::Margin {
            side: Side::Left,
            value: 4.0,
        },
    ));
    l.send(set("r1", Property::Absolute(true)));
    assert_eq!(
        props(&l).notes,
        vec![
            LayoutNote::HugIgnored { axis: Axis::Height },
            LayoutNote::MarginIgnored {},
            LayoutNote::AbsoluteIgnored {},
        ]
    );
    open(&mut l, "t1");
    l.send(set(
        "t1",
        Property::Sizing {
            axis: Axis::Width,
            mode: SizeMode::Hug,
        },
    ));
    assert!(props(&l).notes.is_empty());
    open(&mut l, "root");
    assert!(props(&l).notes.is_empty());
}

fn sizing(axis: Axis, mode: SizeMode) -> Property {
    Property::Sizing { axis, mode }
}

fn side(side: Side, value: f64) -> Property {
    Property::Margin { side, value }
}

#[test]
fn the_height_axis_is_judged_on_its_own() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    l.send(set("root", Property::Layout(LayoutMode::Row)));
    // Hug on the width of the parent only: a fill height still has a size to share, a fill width has not.
    l.send(set("root", sizing(Axis::Width, SizeMode::Hug)));
    open(&mut l, "r1");
    l.send(set("r1", sizing(Axis::Height, SizeMode::Fill)));
    assert!(props(&l).notes.is_empty());
    l.send(set("r1", sizing(Axis::Height, SizeMode::Fixed)));
    l.send(set("r1", sizing(Axis::Width, SizeMode::Fill)));
    assert_eq!(
        props(&l).notes,
        vec![LayoutNote::FillUnderHug { axis: Axis::Width }]
    );
    open(&mut l, "root");
    l.send(set("root", sizing(Axis::Width, SizeMode::Fixed)));
    l.send(set("root", sizing(Axis::Height, SizeMode::Hug)));
    open(&mut l, "r1");
    l.send(set("r1", sizing(Axis::Width, SizeMode::Fixed)));
    l.send(set("r1", sizing(Axis::Height, SizeMode::Fill)));
    assert_eq!(
        props(&l).notes,
        vec![LayoutNote::FillUnderHug { axis: Axis::Height }]
    );
}

#[test]
fn hug_is_ignored_on_a_frame_without_layout_and_on_a_shader_but_not_on_a_layout_frame() {
    let mut nodes = doc();
    nodes.push(node("s1", "shader", Some("root")));
    let mut app = meldframe_app::App::new("user:me", "c0", 1, nodes, 0).unwrap();
    app.dispatch(Action::Connected { now_ms: 0 });
    let mut l = Loopback {
        app,
        seq: 0,
        events: vec![],
    };
    let note = |axis| vec![LayoutNote::HugIgnored { axis }];
    open(&mut l, "s1");
    l.send(set("s1", sizing(Axis::Width, SizeMode::Hug)));
    assert_eq!(props(&l).notes, note(Axis::Width));
    open(&mut l, "root");
    l.send(set("root", sizing(Axis::Height, SizeMode::Hug)));
    assert_eq!(props(&l).notes, note(Axis::Height));
    l.send(set("root", Property::Layout(LayoutMode::Column)));
    assert!(props(&l).notes.is_empty());
}

#[test]
fn an_absolute_node_is_not_a_fill_under_hug_but_keeps_its_own_size_and_says_so() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    l.send(set("root", Property::Layout(LayoutMode::Row)));
    l.send(set("root", sizing(Axis::Width, SizeMode::Hug)));
    open(&mut l, "r1");
    l.send(set("r1", sizing(Axis::Width, SizeMode::Fill)));
    l.send(set("r1", Property::Absolute(true)));
    // The parent lays out and hugs, as in the rule for fill under hug, but an absolute child is not in the flow.
    assert_eq!(
        props(&l).notes,
        vec![LayoutNote::AbsoluteFillIgnored { axis: Axis::Width }]
    );
    open(&mut l, "root");
    l.send(set("root", sizing(Axis::Width, SizeMode::Fixed)));
    open(&mut l, "r1");
    l.send(set("r1", sizing(Axis::Height, SizeMode::Fill)));
    assert_eq!(
        props(&l).notes,
        vec![
            LayoutNote::AbsoluteFillIgnored { axis: Axis::Width },
            LayoutNote::AbsoluteFillIgnored { axis: Axis::Height },
        ]
    );
    open(&mut l, "root");
    l.send(set("root", Property::Layout(LayoutMode::None)));
    open(&mut l, "r1");
    assert_eq!(
        props(&l).notes,
        vec![
            LayoutNote::AbsoluteFillIgnored { axis: Axis::Width },
            LayoutNote::AbsoluteFillIgnored { axis: Axis::Height },
            LayoutNote::AbsoluteIgnored {},
        ]
    );
}

#[test]
fn a_root_with_a_margin_or_absolute_is_outside_a_layout_too() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    assert!(props(&l).notes.is_empty());
    l.send(set("root", side(Side::Bottom, 6.0)));
    assert_eq!(props(&l).notes, vec![LayoutNote::MarginIgnored {}]);
    l.send(set("root", Property::Absolute(true)));
    assert_eq!(
        props(&l).notes,
        vec![LayoutNote::MarginIgnored {}, LayoutNote::AbsoluteIgnored {}]
    );
}

#[test]
fn the_layout_fields_are_refused_on_a_node_that_is_not_a_frame() {
    let mut l = Loopback::live();
    for id in ["r1", "t1"] {
        open(&mut l, id);
        for p in [
            Property::Layout(LayoutMode::Row),
            Property::Gap(4.0),
            Property::Padding {
                side: Side::Left,
                value: 2.0,
            },
            Property::PaddingAll(3.0),
            Property::Align(Align::Center),
            Property::Justify(Justify::End),
        ] {
            l.send(set(id, p));
            assert_eq!(refused(&mut l), vec!["invalid"], "{id}");
        }
    }
    assert_eq!(l.app.state().history.undo_steps, 0);
    // A margin, the sizing and absolute are the node's own: they stay open to every kind.
    l.send(set("t1", side(Side::Top, 1.0)));
    assert!(refused(&mut l).is_empty());
}

#[test]
fn an_absolute_node_in_an_auto_layout_parent_has_no_room_for_its_margin_either() {
    let mut l = Loopback::live();
    open(&mut l, "root");
    l.send(set("root", Property::Layout(LayoutMode::Row)));
    open(&mut l, "r1");
    l.send(set("r1", side(Side::Left, 5.0)));
    assert!(props(&l).notes.is_empty());
    // Out of the flow, the node sits where its own position says: the margin is not read.
    l.send(set("r1", Property::Absolute(true)));
    assert_eq!(props(&l).notes, vec![LayoutNote::MarginIgnored {}]);
    l.send(set("r1", side(Side::Left, 0.0)));
    assert!(props(&l).notes.is_empty());
}
