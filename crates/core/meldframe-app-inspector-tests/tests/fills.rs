//! The fill types of the inspector (D5): what the type switch does to `fill`, `gradient` and `image`, and the edits of a
//! gradient. The rules (defaults, the order of the stops, the limits) are the app layer's; each edit is one undo step.

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, Event, FillKind, ImageFit, Property};
use meldframe_protocol::{GradientKind, Node};

fn set(id: &str, property: Property) -> Action {
    Action::SetProperty {
        id: id.into(),
        property,
    }
}

fn open(l: &mut Loopback, id: &str) {
    l.send(Action::Select { ids: ids(&[id]) });
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

#[test]
fn a_plain_node_is_solid_and_a_none_fill_is_none() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    assert_eq!(props(&l).fill_kind, FillKind::Solid);
    assert!(props(&l).gradient.is_none() && props(&l).image.is_none());
    l.send(set("r1", Property::FillType(FillKind::None)));
    assert_eq!(props(&l).fill_kind, FillKind::None);
    assert_eq!(
        get(&l.app, "r1").fill,
        meldframe_protocol::Bind::Lit("none".to_string())
    );
}

#[test]
fn the_panel_is_told_the_limits_of_the_fill_and_whether_the_node_takes_a_gradient() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    let p = props(&l);
    assert!(p.gradient_allowed);
    assert_eq!((p.min_stops, p.max_stops), (2, 8));
    open(&mut l, "t1");
    assert!(!props(&l).gradient_allowed);
}

#[test]
fn the_linear_type_makes_a_two_stop_gradient_from_the_painted_colour_in_one_step() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set("r1", Property::Fill("#336699".into())));
    let steps = l.app.state().history.undo_steps;
    l.send(set("r1", Property::FillType(FillKind::Linear)));
    let p = props(&l);
    assert_eq!(p.fill_kind, FillKind::Linear);
    let g = p.gradient.unwrap();
    assert_eq!(g.kind, GradientKind::Linear);
    assert_eq!(g.angle, 180.0);
    assert_eq!(g.stops.len(), 2);
    assert_eq!(
        (g.stops[0].pos, g.stops[0].color.as_str()),
        (0.0, "#336699")
    );
    assert_eq!(g.stops[1].pos, 1.0);
    assert_eq!(l.app.state().history.undo_steps, steps + 1);
    l.send(Action::Undo {});
    assert_eq!(props(&l).fill_kind, FillKind::Solid);
}

#[test]
fn radial_keeps_the_stops_of_a_gradient_that_is_there_and_solid_removes_it() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set("r1", Property::FillType(FillKind::Linear)));
    l.send(set("r1", Property::FillType(FillKind::Radial)));
    let g = props(&l).gradient.unwrap();
    assert_eq!((g.kind, g.stops.len()), (GradientKind::Radial, 2));
    l.send(set("r1", Property::FillType(FillKind::Solid)));
    let p = props(&l);
    assert_eq!(p.fill_kind, FillKind::Solid);
    assert!(p.gradient.is_none());
    assert_eq!(l.app.state().history.undo_steps, 3);
}

#[test]
fn a_type_that_is_already_there_makes_no_step() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set("r1", Property::FillType(FillKind::Solid)));
    assert_eq!(l.app.state().history.undo_steps, 0);
}

#[test]
fn text_takes_no_gradient_and_the_image_type_needs_an_image() {
    let mut l = Loopback::live();
    open(&mut l, "t1");
    l.send(set("t1", Property::FillType(FillKind::Linear)));
    l.send(set("t1", Property::FillType(FillKind::Radial)));
    assert_eq!(
        refused(&mut l),
        vec!["gradient_on_text", "gradient_on_text"]
    );
    open(&mut l, "r1");
    l.send(set("r1", Property::FillType(FillKind::Image)));
    assert_eq!(refused(&mut l), vec!["no_image"]);
    assert_eq!(l.app.state().history.undo_steps, 0);
}

fn with_image() -> meldframe_app::App {
    let mut nodes = doc();
    nodes[1] = serde_json::from_value::<Node>(serde_json::json!({
        "id": "r1", "type": "rect", "parentId": "root",
        "image": {"assetId": "a".repeat(64), "fit": "contain"}
    }))
    .unwrap();
    let mut app = meldframe_app::App::new("user:me", "c0", 1, nodes, 0).unwrap();
    app.dispatch(Action::Select { ids: ids(&["r1"]) });
    app
}

#[test]
fn the_image_type_on_a_node_that_has_an_image_changes_nothing_and_is_not_refused() {
    let mut app = with_image();
    app.take_events();
    app.dispatch(set("r1", Property::FillType(FillKind::Image)));
    assert!(
        app.take_events()
            .iter()
            .all(|e| !matches!(e, Event::Refused { .. }))
    );
    assert_eq!(app.state().history.undo_steps, 0);
    assert_eq!(
        app.state().properties.as_ref().unwrap().fill_kind,
        FillKind::Image
    );
}

#[test]
fn none_on_a_fill_bound_to_a_variable_lets_it_go_in_one_step_and_undo_brings_it_back() {
    let mut l = Loopback::live();
    let cmd = |v: serde_json::Value| serde_json::from_value(v).unwrap();
    l.send(Action::EditVariables {
        commands: vec![
            cmd(serde_json::json!({"type": "addCollection", "collection": {"id": "t", "name": "appearance",
                "modes": [{"id": "light", "name": "light"}]}})),
            cmd(serde_json::json!({"type": "addVariable", "variable": {"id": "bg", "collection": "t",
                "name": "color/bg", "type": "color", "values": {"light": "#ffffff"}}})),
        ],
    });
    open(&mut l, "r1");
    l.send(Action::BindProperty {
        id: "r1".into(),
        field: meldframe_app::BindField::Fill,
        variable: Some("bg".into()),
    });
    let bound = get(&l.app, "r1").fill;
    assert_eq!(bound, meldframe_protocol::Bind::var("bg".to_string()));
    let steps = l.app.state().history.undo_steps;
    l.send(set("r1", Property::FillType(FillKind::None)));
    assert_eq!(
        get(&l.app, "r1").fill,
        meldframe_protocol::Bind::Lit("none".to_string())
    );
    assert!(props(&l).bound.is_empty());
    assert_eq!(l.app.state().history.undo_steps, steps + 1);
    l.send(Action::Undo {});
    assert_eq!(get(&l.app, "r1").fill, bound);
    assert_eq!(props(&l).bound, vec!["fill".to_string()]);
}

#[test]
fn an_image_fill_reads_as_the_image_type_and_the_solid_type_takes_it_away() {
    let mut nodes = doc();
    nodes[1] = serde_json::from_value::<Node>(serde_json::json!({
        "id": "r1", "type": "rect", "parentId": "root",
        "image": {"assetId": "a".repeat(64), "fit": "contain"}
    }))
    .unwrap();
    let mut app = meldframe_app::App::new("user:me", "c0", 1, nodes, 0).unwrap();
    app.dispatch(Action::Select { ids: ids(&["r1"]) });
    let p = app.state().properties.clone().unwrap();
    assert_eq!(p.fill_kind, FillKind::Image);
    assert_eq!(p.image.unwrap().asset_id, "a".repeat(64));
    app.dispatch(set("r1", Property::FillType(FillKind::Solid)));
    assert_eq!(
        app.state().properties.as_ref().unwrap().fill_kind,
        FillKind::Solid
    );
}

#[test]
fn the_angle_is_kept_in_a_turn_and_refused_without_a_gradient() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set("r1", Property::GradientAngle(45.0)));
    assert_eq!(refused(&mut l), vec!["no_gradient"]);
    l.send(set("r1", Property::FillType(FillKind::Linear)));
    l.send(set("r1", Property::GradientAngle(405.0)));
    assert_eq!(props(&l).gradient.unwrap().angle, 45.0);
    l.send(set("r1", Property::GradientAngle(f64::NAN)));
    assert_eq!(refused(&mut l), vec!["invalid"]);
}

#[test]
fn a_stop_is_edited_by_position_and_colour_and_held_between_its_neighbours() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set("r1", Property::FillType(FillKind::Linear)));
    l.send(set("r1", Property::AddGradientStop {}));
    let stops = props(&l).gradient.unwrap().stops;
    assert_eq!(stops.len(), 3);
    assert_eq!(
        stops[1].pos, 0.5,
        "a new stop goes in the middle of the widest gap"
    );
    l.send(set(
        "r1",
        Property::GradientStop {
            index: 1,
            pos: 2.0,
            color: "#ff0000".into(),
        },
    ));
    let stops = props(&l).gradient.unwrap().stops;
    assert_eq!((stops[1].pos, stops[1].color.as_str()), (1.0, "#ff0000"));
    l.send(set(
        "r1",
        Property::GradientStop {
            index: 1,
            pos: 0.5,
            color: "red".into(),
        },
    ));
    assert_eq!(refused(&mut l), vec!["invalid"]);
    l.send(set(
        "r1",
        Property::GradientStop {
            index: 9,
            pos: 0.5,
            color: "#fff".into(),
        },
    ));
    assert_eq!(refused(&mut l), vec!["not_found"]);
}

#[test]
fn stops_are_added_up_to_eight_and_removed_down_to_two() {
    let mut l = Loopback::live();
    open(&mut l, "r1");
    l.send(set("r1", Property::FillType(FillKind::Linear)));
    for _ in 0..6 {
        l.send(set("r1", Property::AddGradientStop {}));
    }
    assert_eq!(props(&l).gradient.unwrap().stops.len(), 8);
    refused(&mut l);
    l.send(set("r1", Property::AddGradientStop {}));
    assert_eq!(refused(&mut l), vec!["limit"]);
    for _ in 0..6 {
        l.send(set("r1", Property::RemoveGradientStop(1)));
    }
    assert_eq!(props(&l).gradient.unwrap().stops.len(), 2);
    refused(&mut l);
    l.send(set("r1", Property::RemoveGradientStop(0)));
    assert_eq!(refused(&mut l), vec!["min_stops"]);
}

#[test]
fn the_new_properties_read_from_json() {
    let a: Action = serde_json::from_value(serde_json::json!({
        "type": "setProperty", "id": "r1", "property": {"property": "fillType", "value": "radial"}
    }))
    .unwrap();
    assert!(matches!(
        a,
        Action::SetProperty {
            property: Property::FillType(FillKind::Radial),
            ..
        }
    ));
    let a: Action = serde_json::from_value(serde_json::json!({
        "type": "setProperty", "id": "r1", "property": {"property": "gradientStop", "value": {"index": 1, "pos": 0.5, "color": "#fff"}}
    }))
    .unwrap();
    assert!(matches!(a, Action::SetProperty { .. }));
}

// Choosing an image (E2.5): `Action::SetImageFill`; the picker is in `image_fill.rs`. The server says if an asset exists.

fn id_of(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

#[test]
fn set_image_fill_makes_the_node_an_image_in_one_undo_step_and_undo_takes_it_back() {
    let mut l = Loopback::live();
    select(&mut l, "r1");
    let id = id_of(7);
    l.send(Action::SetImageFill {
        id: "r1".into(),
        asset_id: id.clone(),
        fit: Some(ImageFit::Contain),
    });
    let n = get(&l.app, "r1");
    let im = n.image.clone().unwrap();
    assert_eq!(
        (im.asset_id.as_str(), im.fit),
        (id.as_str(), ImageFit::Contain)
    );
    let p = l.app.state().properties.clone().unwrap();
    assert_eq!(p.fill_kind, FillKind::Image);
    assert_eq!(l.app.state().history.undo_steps, 1);
    l.send(Action::Undo {});
    assert!(get(&l.app, "r1").image.is_none());
    assert_eq!(
        l.app.state().properties.as_ref().unwrap().fill_kind,
        FillKind::Solid
    );
    assert!(refused(&mut l).is_empty());
}

#[test]
fn set_image_fill_keeps_the_colour_and_the_gradient_and_the_same_image_again_makes_no_step() {
    let mut l = Loopback::live();
    select(&mut l, "r1");
    l.send(Action::SetProperty {
        id: "r1".into(),
        property: Property::Fill("#336699".into()),
    });
    l.send(Action::SetProperty {
        id: "r1".into(),
        property: Property::FillType(FillKind::Linear),
    });
    let before = get(&l.app, "r1");
    let steps = l.app.state().history.undo_steps;
    let set = |fit| Action::SetImageFill {
        id: "r1".into(),
        asset_id: id_of(3),
        fit,
    };
    l.send(set(Some(ImageFit::Cover)));
    let after = get(&l.app, "r1");
    assert_eq!(
        (&after.fill, &after.gradient),
        (&before.fill, &before.gradient)
    );
    assert_eq!(l.app.state().history.undo_steps, steps + 1);
    l.send(set(Some(ImageFit::Cover)));
    assert_eq!(
        l.app.state().history.undo_steps,
        steps + 1,
        "the same image and fit: no step"
    );
    l.send(set(Some(ImageFit::Fill)));
    assert_eq!(get(&l.app, "r1").image.unwrap().fit, ImageFit::Fill);
    assert_eq!(
        l.app.state().history.undo_steps,
        steps + 2,
        "another fit is a step"
    );
    // The solid type takes the image away again.
    l.send(Action::SetProperty {
        id: "r1".into(),
        property: Property::FillType(FillKind::Solid),
    });
    assert!(get(&l.app, "r1").image.is_none());
}

#[test]
fn set_image_fill_refuses_what_is_not_a_content_hash_text_and_unknown_nodes_and_changes_nothing() {
    let mut l = Loopback::live();
    select(&mut l, "r1");
    for bad in [
        "",
        "abc",
        &"A".repeat(64),
        &"g".repeat(64),
        &"a".repeat(63),
        &"a".repeat(65),
        "../../etc/passwd",
    ] {
        l.send(Action::SetImageFill {
            id: "r1".into(),
            asset_id: bad.into(),
            fit: Some(ImageFit::Cover),
        });
    }
    l.send(Action::SetImageFill {
        id: "t1".into(),
        asset_id: id_of(1),
        fit: Some(ImageFit::Cover),
    });
    l.send(Action::SetImageFill {
        id: "nope".into(),
        asset_id: id_of(1),
        fit: Some(ImageFit::Cover),
    });
    let codes = refused(&mut l);
    assert_eq!(codes.len(), 9, "{codes:?}");
    assert!(
        codes[..7].iter().all(|c| c == "invalid_asset_id"),
        "{codes:?}"
    );
    assert_eq!(&codes[7..], ["image_on_text", "not_found"]);
    assert!(get(&l.app, "r1").image.is_none() && get(&l.app, "t1").image.is_none());
    assert_eq!(l.app.state().history.undo_steps, 0);
}

#[test]
fn without_a_fit_the_node_keeps_its_fit_and_a_node_with_no_image_gets_cover() {
    let mut l = Loopback::live();
    select(&mut l, "r1");
    let pick = |n: u8, fit| Action::SetImageFill {
        id: "r1".into(),
        asset_id: id_of(n),
        fit,
    };
    l.send(pick(1, None));
    assert_eq!(get(&l.app, "r1").image.unwrap().fit, ImageFit::Cover);
    l.send(pick(2, Some(ImageFit::Contain)));
    l.send(pick(3, None));
    let im = get(&l.app, "r1").image.unwrap();
    assert_eq!(
        (im.asset_id, im.fit),
        (id_of(3), ImageFit::Contain),
        "the fit stays"
    );
    let steps = l.app.state().history.undo_steps;
    l.send(pick(3, None));
    assert_eq!(
        l.app.state().history.undo_steps,
        steps,
        "the same image: no step"
    );
    // The wire form may leave the fit out.
    let a: Action = serde_json::from_value(
        serde_json::json!({"type": "setImageFill", "id": "r1", "assetId": id_of(1)}),
    )
    .unwrap();
    assert!(matches!(a, Action::SetImageFill { fit: None, .. }));
    let a: Action = serde_json::from_value(
        serde_json::json!({"type": "setImageFill", "id": "r1", "assetId": id_of(1), "fit": "contain"}),
    )
    .unwrap();
    assert!(matches!(
        a,
        Action::SetImageFill {
            fit: Some(ImageFit::Contain),
            ..
        }
    ));
}

#[test]
fn choosing_an_image_closes_the_picker_and_a_refused_choice_leaves_it_open() {
    let mut l = Loopback::live();
    let choose = |asset_id: &str| Action::SetImageFill {
        id: "r1".into(),
        asset_id: asset_id.into(),
        fit: None,
    };
    select(&mut l, "r1");
    let app = &mut l.app;
    let told = |app: &mut meldframe_app::App| {
        let events = app.take_events();
        events
            .iter()
            .filter(|e| matches!(e, Event::AssetsChanged {}))
            .count()
    };
    app.dispatch(Action::OpenAssets { id: "r1".into() });
    told(app);
    app.dispatch(choose(&id_of(1)));
    assert!(app.state().assets.target.is_none() && app.state().assets.items.is_empty());
    assert_eq!(told(app), 1, "the front end is told");
    app.dispatch(Action::OpenAssets { id: "r1".into() });
    told(app);
    app.dispatch(choose("nope"));
    assert!(app.state().assets.target.is_some());
    assert_eq!(told(app), 0, "a refused choice changes nothing");
    app.dispatch(choose(&id_of(1)));
    assert!(
        app.state().assets.target.is_none(),
        "the same image makes no step and still closes it"
    );
}
