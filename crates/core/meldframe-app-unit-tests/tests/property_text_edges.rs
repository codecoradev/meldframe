//! The edges of typed text to a property (audit finding T4, issue 322). The number is read as Rust reads an `f64`, not as
//! JavaScript `Number()` did: `0x10`, `0b11`, `0o7`, a comma and a text that is not finite are refused, a U+FEFF is not a
//! space, and U+0085 is. A zero is stored as `0.0` (never `-0.0`), an angle is kept within one turn, a four-digit hex
//! colour is a colour, and a part that is not there (a shadow, a stop) is said in words of its own.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::get;
use common::prop_text::{live, props, refused, steps, text};
use meldframe_app::{Action, TextField, TypedText};

#[test]
fn a_number_is_read_the_way_rust_reads_an_f64() {
    let mut l = live("r1");
    // Accepted: an exponent, a sign, a decimal point, the spaces around it and U+0085 (Rust trims it, JavaScript did not).
    for (raw, x) in [
        ("1e3", 1000.0),
        (" 5 ", 5.0),
        ("+7", 7.0),
        ("2.5", 2.5),
        ("\u{85}9\u{85}", 9.0),
    ] {
        assert!(
            refused(&text(&mut l, "r1", TextField::X {}, raw)).is_empty(),
            "{raw:?}"
        );
        assert_eq!(get(&l.app, "r1").x, x, "{raw:?}");
    }
    let done = steps(&l);
    // Refused: a prefix of another base, a decimal comma, text that is not finite, and U+FEFF (not a space to Rust).
    for raw in [
        "0x10",
        "0b11",
        "0o7",
        "1,5",
        "1e999",
        "-1e999",
        "NaN",
        "inf",
        "-inf",
        "\u{feff}5",
        "5 5",
        "--1",
        ".",
    ] {
        assert_eq!(
            refused(&text(&mut l, "r1", TextField::X {}, raw)),
            ["invalid_value"],
            "{raw:?}"
        );
    }
    assert_eq!(steps(&l), done);
    assert_eq!(get(&l.app, "r1").x, 9.0);
}

#[test]
fn a_zero_is_stored_as_a_zero_and_never_as_minus_zero() {
    let mut l = live("r1");
    text(&mut l, "r1", TextField::X {}, "-0");
    let x = get(&l.app, "r1").x;
    assert!(x == 0.0 && x.is_sign_positive(), "x is {x:?}");
    text(&mut l, "r1", TextField::Radius {}, "-0");
    let radius = props(&l).radius;
    assert!(
        radius == 0.0 && radius.is_sign_positive(),
        "radius is {radius:?}"
    );
    text(&mut l, "r1", TextField::GradientAngle {}, "-0");
    let angle = props(&l).gradient.unwrap().angle;
    assert!(
        angle == 0.0 && angle.is_sign_positive(),
        "angle is {angle:?}"
    );
}

#[test]
fn an_angle_is_kept_within_one_turn_so_360_over_0_changes_nothing() {
    let mut l = live("r1");
    text(&mut l, "r1", TextField::GradientAngle {}, "0");
    assert_eq!(props(&l).gradient.unwrap().angle, 0.0);
    let done = steps(&l);
    assert!(text(&mut l, "r1", TextField::GradientAngle {}, "360").is_empty());
    assert!(text(&mut l, "r1", TextField::GradientAngle {}, "720").is_empty());
    assert_eq!(steps(&l), done);
    text(&mut l, "r1", TextField::GradientAngle {}, "-90");
    assert_eq!(props(&l).gradient.unwrap().angle, 270.0);
    assert!(text(&mut l, "r1", TextField::GradientAngle {}, "270").is_empty());
    assert!(text(&mut l, "r1", TextField::GradientAngle {}, "-90").is_empty());
}

#[test]
fn a_four_digit_hex_colour_is_a_colour_for_every_colour_field() {
    let mut l = live("r1");
    assert!(refused(&text(&mut l, "r1", TextField::Fill {}, "#f00a")).is_empty());
    assert_eq!(steps(&l), 1);
    assert!(refused(&text(&mut l, "r1", TextField::BorderColor {}, "#0f08")).is_empty());
    for raw in ["#f00a0", "#f0", "#ggg", "f00", "#ff00ff0"] {
        assert_eq!(
            refused(&text(&mut l, "r1", TextField::Fill {}, raw)),
            ["E_INVALID_COLOR"],
            "{raw}"
        );
    }
    assert_eq!(steps(&l), 2);
}

#[test]
fn a_shadow_or_a_stop_that_is_not_there_is_said_in_words_of_its_own() {
    let mut l = live("r1");
    for field in [
        TextField::Shadow {
            index: 4,
            part: meldframe_app_model::ShadowPart::X {},
        },
        TextField::StopPos { index: 9 },
        TextField::StopColor { index: 9 },
    ] {
        assert_eq!(
            refused(&text(&mut l, "r1", field.clone(), "5")),
            ["part_not_found"],
            "{field:?}"
        );
    }
    // The node itself being gone is still `not_found`.
    l.send(Action::Typed {
        entry: TypedText::Property {
            id: "gone".into(),
            field: TextField::X {},
            text: "5".into(),
        },
    });
    assert_eq!(
        refused(&l.events.drain(..).collect::<Vec<_>>()),
        ["not_found"]
    );
    assert_eq!(steps(&l), 0);
}

#[test]
fn a_size_below_the_least_is_refused_before_the_same_value_is_looked_at() {
    let mut l = live("r1");
    // 0.5 is under the least (1 px) and refused; the node is 100 wide, 50 high.
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::W {}, "0.5")),
        ["invalid_value"]
    );
    assert_eq!(
        refused(&text(&mut l, "r1", TextField::H {}, "0")),
        ["invalid_value"]
    );
    assert!(text(&mut l, "r1", TextField::W {}, "100").is_empty());
    assert_eq!(steps(&l), 0);
}

#[test]
fn a_garbage_position_of_a_node_the_layout_places_is_refused_not_ignored() {
    let mut l = live("kid");
    l.app.set_layout_on_canvas(true);
    l.app.dispatch(Action::ClearSelection {});
    l.send(Action::Select {
        ids: vec!["kid".into()],
    });
    assert!(props(&l).position_locked);
    l.events.clear();
    // The child of an auto layout frame has no x or y of its own: a good number is ignored, a bad text is refused.
    assert!(text(&mut l, "kid", TextField::X {}, "12").is_empty());
    assert_eq!(
        refused(&text(&mut l, "kid", TextField::X {}, "abc")),
        ["invalid_value"]
    );
    assert_eq!(steps(&l), 0);
}
