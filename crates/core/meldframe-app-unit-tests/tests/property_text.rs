//! Typed text to a property (audit finding T4, issue 322): the parse of what a person types into a field of the
//! properties panel, its range, and the decision that it changes nothing are the app layer's. A front end sends the
//! text and the field; a text that is empty or says what the field already says makes no action, a text outside the
//! range is refused, and a colour that is not one is refused by the core with the same code for every colour field.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::prop_text::{live, props, refused, steps, text};
use common::*;
use meldframe_app::{Action, Side, TextField, TypedText};
use meldframe_app_model::ShadowPart;

#[test]
fn opacity_text_33_3_sets_0_333() {
    let mut l = live("r1");
    let e = text(&mut l, "r1", TextField::Opacity {}, " 33.3 ");
    assert!(refused(&e).is_empty());
    assert_eq!(get(&l.app, "r1").opacity, 0.333);
    assert_eq!(props(&l).opacity, 0.333);
    assert_eq!(steps(&l), 1);
}

#[test]
fn opacity_text_101_is_refused_invalid_value() {
    let mut l = live("r1");
    for t in ["101", "-1", "abc", "NaN", "inf"] {
        assert_eq!(
            refused(&text(&mut l, "r1", TextField::Opacity {}, t)),
            ["invalid_value"],
            "{t}"
        );
    }
    assert_eq!(steps(&l), 0);
    assert_eq!(props(&l).opacity, 1.0);
}

#[test]
fn radius_text_minus_1_is_refused() {
    let mut l = live("r1");
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::Radius {}, "-1")),
        ["invalid_value"]
    );
    assert_eq!(props(&l).radius, 4.0);
    assert!(refused(&text(&mut l, "r1", TextField::Radius {}, "12")).is_empty());
    assert_eq!(props(&l).radius, 12.0);
    assert_eq!(steps(&l), 1);
}

#[test]
fn an_empty_text_sends_nothing_and_says_nothing() {
    let mut l = live("r1");
    for field in [
        TextField::Name {},
        TextField::Fill {},
        TextField::Radius {},
        TextField::Opacity {},
        TextField::W {},
        TextField::X {},
        TextField::BorderWidth {},
        TextField::BorderColor {},
        TextField::GradientAngle {},
        TextField::Gap {},
    ] {
        let e = text(&mut l, "r1", field.clone(), "  ");
        assert!(e.is_empty(), "{field:?}: {e:?}");
    }
    assert_eq!(steps(&l), 0);
}

#[test]
fn fill_text_zzz_is_refused_with_e_invalid_color_like_the_border() {
    let mut l = live("r1");
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::Fill {}, "zzz")),
        ["E_INVALID_COLOR"]
    );
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::BorderColor {}, "zzz")),
        ["E_INVALID_COLOR"]
    );
    assert_eq!(
        refused(&text(
            &mut l,
            "r1",
            TextField::StopColor { index: 0 },
            "zzz"
        )),
        ["E_INVALID_COLOR"]
    );
    assert_eq!(
        refused(&text(
            &mut l,
            "r1",
            TextField::Shadow {
                index: 0,
                part: ShadowPart::Color {}
            },
            "zzz"
        )),
        ["E_INVALID_COLOR"]
    );
    assert_eq!(steps(&l), 0);
}

#[test]
fn a_colour_is_trimmed_and_sets_the_fill_and_none_takes_it_away() {
    let mut l = live("r1");
    assert!(refused(&text(&mut l, "r1", TextField::Fill {}, " #00FF00 ")).is_empty());
    assert_eq!(props(&l).fill.to_lowercase(), "#00ff00");
    assert!(refused(&text(&mut l, "r1", TextField::Fill {}, "none")).is_empty());
    assert_eq!(props(&l).fill, "none");
    assert_eq!(steps(&l), 2);
}

#[test]
fn same_value_makes_no_undo_step() {
    let mut l = live("r1");
    for (field, t) in [
        (TextField::Radius {}, "4"),
        (TextField::Opacity {}, "100"),
        (TextField::Fill {}, "#FF0000"),
        (TextField::X {}, "10"),
        (TextField::W {}, "100"),
        (TextField::BorderWidth {}, "2"),
        (TextField::BorderColor {}, "#000000"),
        (TextField::Name {}, ""),
        (TextField::GradientAngle {}, "90"),
        (TextField::StopPos { index: 1 }, "100"),
    ] {
        let e = text(&mut l, "r1", field.clone(), t);
        assert!(e.is_empty(), "{field:?} {t}: {e:?}");
    }
    assert_eq!(steps(&l), 0);
}

#[test]
fn a_name_is_trimmed_and_the_same_name_changes_nothing() {
    let mut l = live("r1");
    text(&mut l, "r1", TextField::Name {}, "  Button  ");
    assert_eq!(get(&l.app, "r1").name, "Button");
    text(&mut l, "r1", TextField::Name {}, "Button");
    assert_eq!(steps(&l), 1);
}

#[test]
fn size_text_0_is_refused_and_a_size_keeps_the_origin_and_the_other_side() {
    let mut l = live("r1");
    for t in ["0", "0.5", "-3", "wide"] {
        assert_eq!(
            refused(&text(&mut l, "r1", TextField::W {}, t)),
            ["invalid_value"],
            "{t}"
        );
    }
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::H {}, "0")),
        ["invalid_value"]
    );
    assert_eq!(steps(&l), 0);
    text(&mut l, "r1", TextField::W {}, "120");
    let n = get(&l.app, "r1");
    assert_eq!((n.x, n.y, n.w, n.h), (10.0, 20.0, 120.0, 50.0));
    text(&mut l, "r1", TextField::H {}, "60");
    let n = get(&l.app, "r1");
    assert_eq!((n.w, n.h), (120.0, 60.0));
    assert_eq!(steps(&l), 2);
}

#[test]
fn x_and_y_move_the_node_and_keep_the_other_coordinate() {
    let mut l = live("r1");
    text(&mut l, "r1", TextField::X {}, "33");
    let n = get(&l.app, "r1");
    assert_eq!((n.x, n.y), (33.0, 20.0));
    text(&mut l, "r1", TextField::Y {}, "-5");
    let n = get(&l.app, "r1");
    assert_eq!((n.x, n.y), (33.0, -5.0));
}

#[test]
fn x_text_is_ignored_when_position_locked() {
    let mut l = live("kid");
    l.app.set_layout_on_canvas(true);
    l.app.dispatch(Action::ClearSelection {});
    l.send(Action::Select { ids: ids(&["kid"]) });
    assert!(props(&l).position_locked);
    l.events.clear();
    for field in [TextField::X {}, TextField::Y {}] {
        assert!(text(&mut l, "kid", field, "99").is_empty());
    }
    assert_eq!(steps(&l), 0);
    // The size is still the person's to set.
    text(&mut l, "kid", TextField::W {}, "50");
    assert_eq!(get(&l.app, "kid").w, 50.0);
}

#[test]
fn font_size_must_be_above_zero() {
    let mut l = live("t1");
    assert_eq!(
        refused(&text(&mut l, "t1", TextField::FontSize {}, "0")),
        ["invalid_value"]
    );
    assert_eq!(
        refused(&text(&mut l, "t1", TextField::FontSize {}, "-2")),
        ["invalid_value"]
    );
    text(&mut l, "t1", TextField::FontSize {}, "20");
    assert_eq!(props(&l).font_size, 20.0);
    assert_eq!(steps(&l), 1);
}

#[test]
fn the_border_takes_a_width_of_zero_or_more_and_a_zero_removes_it() {
    let mut l = live("r1");
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::BorderWidth {}, "-1")),
        ["invalid_value"]
    );
    text(&mut l, "r1", TextField::BorderWidth {}, "0");
    assert_eq!(props(&l).border.width, 0.0);
    text(&mut l, "r1", TextField::BorderWidth {}, "3.5");
    assert_eq!(props(&l).border.width, 3.5);
}

#[test]
fn a_shadow_part_is_a_number_or_a_colour_and_a_blur_is_not_negative() {
    let mut l = live("r1");
    let part = |part| TextField::Shadow { index: 0, part };
    assert_eq!(
        refused(&text(&mut l, "r1", part(ShadowPart::Blur {}), "-1")),
        ["invalid_value"]
    );
    assert_eq!(
        refused(&text(&mut l, "r1", part(ShadowPart::X {}), "wide")),
        ["invalid_value"]
    );
    assert!(text(&mut l, "r1", part(ShadowPart::Y {}), "4").is_empty());
    text(&mut l, "r1", part(ShadowPart::Spread {}), "-2");
    assert_eq!(props(&l).shadows[0].spread, -2.0);
    text(&mut l, "r1", part(ShadowPart::Color {}), "#112233");
    assert_eq!(props(&l).shadows[0].color, "#112233");
    assert_eq!(
        refused(&text(
            &mut l,
            "r1",
            TextField::Shadow {
                index: 3,
                part: ShadowPart::X {}
            },
            "1"
        )),
        ["part_not_found"]
    );
    assert_eq!(steps(&l), 2);
}

#[test]
fn the_gradient_angle_and_stop_position_are_typed_as_degrees_and_percent() {
    let mut l = live("r1");
    text(&mut l, "r1", TextField::GradientAngle {}, "45");
    assert_eq!(props(&l).gradient.unwrap().angle, 45.0);
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::StopPos { index: 1 }, "101")),
        ["invalid_value"]
    );
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::StopPos { index: 1 }, "-1")),
        ["invalid_value"]
    );
    text(&mut l, "r1", TextField::StopPos { index: 1 }, "66.66");
    // Rounded to a tenth of a percent: the file holds 0 to 1.
    assert_eq!(props(&l).gradient.unwrap().stops[1].pos, 0.667);
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::StopPos { index: 9 }, "5")),
        ["part_not_found"]
    );
}

#[test]
fn a_node_without_a_gradient_refuses_an_angle() {
    let mut l = live("t1");
    assert_eq!(
        refused(&text(&mut l, "t1", TextField::GradientAngle {}, "10")),
        ["no_gradient"]
    );
}

#[test]
fn gap_padding_and_margin_take_a_length_of_zero_or_more() {
    let mut l = live("col");
    assert_eq!(
        refused(&text(&mut l, "col", TextField::Gap {}, "-1")),
        ["invalid_value"]
    );
    text(&mut l, "col", TextField::Gap {}, "12");
    assert_eq!(props(&l).gap, 12.0);
    text(&mut l, "col", TextField::PaddingAll {}, "6");
    assert_eq!(props(&l).padding_uniform, Some(6.0));
    assert!(text(&mut l, "col", TextField::PaddingAll {}, "6").is_empty());
    text(&mut l, "col", TextField::Padding { side: Side::Left }, "9");
    assert_eq!(props(&l).padding.left, 9.0);
    assert_eq!(props(&l).padding_uniform, None);
    assert_eq!(
        refused(&text(
            &mut l,
            "col",
            TextField::Margin { side: Side::Top },
            "-4"
        )),
        ["invalid_value"]
    );
    text(&mut l, "col", TextField::Margin { side: Side::Top }, "5");
    assert_eq!(props(&l).margin.top, 5.0);
}

#[test]
fn a_field_of_a_node_that_is_not_the_selected_one_is_refused() {
    let mut l = live("r1");
    assert_eq!(
        refused(&text(&mut l, "t1", TextField::Radius {}, "3")),
        ["not_selected"]
    );
    assert_eq!(
        refused(&text(&mut l, "ghost", TextField::Radius {}, "3")),
        ["not_found"]
    );
    assert_eq!(steps(&l), 0);
}

#[test]
fn the_properties_say_which_sections_a_kind_has() {
    let kinds = [
        ("root", (true, true, false, true)),
        ("r1", (true, false, false, false)),
        ("t1", (false, false, true, false)),
    ];
    for (id, (radius, clip, font_size, layout)) in kinds {
        let l = live(id);
        let s = props(&l).sections;
        assert_eq!(
            (s.radius, s.clip, s.font_size, s.layout),
            (radius, clip, font_size, layout),
            "{id}"
        );
    }
}

#[test]
fn the_placement_section_shows_in_a_flow_when_absolute_or_with_a_margin() {
    let mut l = live("r1");
    assert!(!props(&l).sections.placement);
    text(&mut l, "r1", TextField::Margin { side: Side::Left }, "3");
    assert!(props(&l).sections.placement);
    l.send(Action::Select { ids: ids(&["kid"]) });
    assert!(
        props(&l).sections.placement,
        "a child of an auto layout frame"
    );
}

#[test]
fn the_action_has_a_stable_wire_shape() {
    let a: Action = serde_json::from_str(
        r#"{"type":"typed","entry":{"type":"property","id":"r1","field":{"type":"shadow","index":1,"part":{"type":"blur"}},"text":"4"}}"#,
    )
    .unwrap();
    assert!(matches!(
        a,
        Action::Typed {
            entry: TypedText::Property {
                field: TextField::Shadow {
                    index: 1,
                    part: ShadowPart::Blur {}
                },
                ..
            }
        }
    ));
    let v: Action = serde_json::from_str(
        r##"{"type":"typed","entry":{"type":"variableValue","variable":"bg","mode":"light","text":"#fff"}}"##,
    )
    .unwrap();
    assert!(matches!(
        v,
        Action::Typed {
            entry: TypedText::VariableValue { .. }
        }
    ));
    for bad in [
        r#"{"type":"typed","entry":{"type":"property","id":"r1","field":{"type":"opacity"},"text":"4","extra":1}}"#,
        r#"{"type":"typed","entry":{"type":"property","id":"r1","field":{"type":"opacity"},"text":"4"},"extra":1}"#,
        r#"{"type":"setPropertyText","id":"r1","field":{"type":"opacity"},"text":"4"}"#,
    ] {
        assert!(serde_json::from_str::<Action>(bad).is_err(), "{bad}");
    }
}
