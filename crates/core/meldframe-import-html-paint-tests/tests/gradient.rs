//! Gradients: `linear-gradient()` with an angle or a side, stops with or without positions, the one radial form the
//! export writes, and everything else of the `background-image` list, which is refused and listed.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::paint::Ctx;
use meldframe_protocol::GradientKind;

fn bg(image: &str) -> Read {
    read(&format!("background-image:{image}"))
}

#[test]
fn a_linear_gradient_with_an_angle_and_percent_stops_is_carried() {
    let r = bg("linear-gradient(90deg, #f00 0%, #00f 100%)");
    assert_eq!(*r.gradient(), linear(90.0, &[(0.0, "#f00"), (1.0, "#00f")]));
    assert!(r.report.entries.is_empty());
    // The form the export writes, with no spaces.
    let r = bg("linear-gradient(45deg,#ff0000 0%,#0000ff80 33.5%,#00ff00 100%)");
    let g = r.gradient();
    assert!(near(g.angle, 45.0) && g.kind == GradientKind::Linear);
    assert!(near(g.stops[1].pos, 0.335) && g.stops[1].color == "#0000ff80");
}

#[test]
fn the_default_direction_and_the_side_keywords_are_angles() {
    assert!(near(
        bg("linear-gradient(red, blue)").gradient().angle,
        180.0
    ));
    for (to, deg) in [
        ("top", 0.0),
        ("right", 90.0),
        ("bottom", 180.0),
        ("left", 270.0),
    ] {
        let r = bg(&format!("linear-gradient(to {to}, red, blue)"));
        assert!(near(r.gradient().angle, deg), "to {to}");
    }
    assert!(near(
        bg("linear-gradient(-30deg, red, blue)").gradient().angle,
        -30.0
    ));
    assert!(near(
        bg("linear-gradient(0.5deg, red, blue)").gradient().angle,
        0.5
    ));
}

#[test]
fn a_corner_needs_the_size_of_the_box_to_be_an_angle() {
    let r = bg("linear-gradient(to top right, red, blue)");
    assert!(r.paint.gradient.is_none());
    assert_eq!(r.notes(), ["refused:gradient_corner_size"]);
    let square = Ctx {
        width: Some(100.0),
        height: Some(100.0),
        ..Ctx::default()
    };
    let wide = Ctx {
        width: Some(200.0),
        height: Some(100.0),
        ..Ctx::default()
    };
    let angle = |to: &str, c: &Ctx| {
        read_ctx(
            &format!("background-image:linear-gradient(to {to}, red, blue)"),
            c,
        )
        .gradient()
        .angle
    };
    assert!(near(angle("top right", &square), 45.0));
    assert!(near(angle("right top", &square), 45.0));
    assert!(near(angle("bottom right", &square), 135.0));
    assert!(near(angle("bottom left", &square), 225.0));
    assert!(near(angle("top left", &square), 315.0));
    let t = (100.0f64).atan2(200.0).to_degrees();
    assert!(near(angle("top right", &wide), t));
    assert!(near(angle("bottom right", &wide), 180.0 - t));
    // Two sides of one axis are no corner.
    let r = read_ctx(
        "background-image:linear-gradient(to top bottom, red, blue)",
        &square,
    );
    assert!(r.has("refused:gradient_config"));
}

#[test]
fn stops_without_a_position_are_spread_evenly_between_the_ones_that_have_one() {
    let pos =
        |image: &str| -> Vec<f64> { bg(image).gradient().stops.iter().map(|s| s.pos).collect() };
    assert_eq!(pos("linear-gradient(red, blue, lime)"), [0.0, 0.5, 1.0]);
    assert_eq!(pos("linear-gradient(red, blue, lime, white)").len(), 4);
    let p = pos("linear-gradient(red, blue 20%, lime, white, black)");
    assert!(near(p[0], 0.0) && near(p[1], 0.2) && near(p[4], 1.0));
    assert!(near(p[2], 0.2 + 0.8 / 3.0) && near(p[3], 0.2 + 1.6 / 3.0));
    // The first and the last are 0 and 100% when they have none.
    let p = pos("linear-gradient(red 10%, blue 30%)");
    assert_eq!(p, [0.1, 0.3]);
    let p = pos("linear-gradient(red, blue 30%)");
    assert_eq!(p, [0.0, 0.3]);
}

#[test]
fn two_positions_make_two_stops_and_equal_ones_a_hard_edge() {
    let r = bg("linear-gradient(red 0% 50%, blue 50%)");
    assert_eq!(
        r.gradient().stops,
        stops(&[(0.0, "#ff0000"), (0.5, "#ff0000"), (0.5, "#0000ff")])
    );
}

#[test]
fn a_position_that_goes_back_is_raised_as_css_does_and_listed() {
    let r = bg("linear-gradient(red 50%, blue 20%)");
    assert_eq!(
        r.gradient().stops,
        stops(&[(0.5, "#ff0000"), (0.5, "#0000ff")])
    );
    assert_eq!(r.notes(), ["listed:gradient_stop_clamped"]);
}

#[test]
fn stop_colours_may_be_any_colour_the_fill_reads() {
    let r =
        bg("linear-gradient(rgb(255, 0, 0), rgba(0, 0, 255, 0.5) 60%, transparent, currentcolor)");
    let c: Vec<&str> = r.gradient().stops.iter().map(|s| &*s.color).collect();
    assert_eq!(c, ["#ff0000", "#0000ff80", "#00000000", "#000000"]);
}

#[test]
fn more_than_eight_stops_keep_the_first_eight_and_say_so() {
    let list = (0..10)
        .map(|i| format!("#00000{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let r = bg(&format!("linear-gradient(90deg,{list})"));
    assert_eq!(r.gradient().stops.len(), 8);
    assert_eq!(r.gradient().stops[7].color, "#000007");
    assert_eq!(r.notes(), ["listed:gradient_stops_cut"]);
}

#[test]
fn what_a_gradient_cannot_be_is_refused_with_its_reason() {
    for (image, code) in [
        ("linear-gradient(red)", "gradient_stops_few"),
        ("linear-gradient(90deg)", "gradient_stops_few"),
        ("linear-gradient(red, blue 10px)", "gradient_stop_unit"),
        ("linear-gradient(red, blue banana)", "gradient_stop_unit"),
        ("linear-gradient(red, 30%, blue)", "gradient_hint"),
        ("linear-gradient(red, banana)", "colour_unreadable"),
        ("linear-gradient(red, blue 120%)", "gradient_stop_range"),
        ("linear-gradient(red -10%, blue)", "gradient_stop_range"),
        ("linear-gradient(1turn, red, blue)", "gradient_angle_unit"),
        ("linear-gradient(90, red, blue)", "gradient_angle_unit"),
        (
            "linear-gradient(4000deg, red, blue)",
            "gradient_angle_range",
        ),
        ("linear-gradient(banana, red, blue)", "gradient_config"),
        ("linear-gradient(in oklch, red, blue)", "gradient_config"),
        ("linear-gradient(to middle, red, blue)", "gradient_config"),
        ("linear-gradient(red, blue, )", "gradient_syntax"),
        ("linear-gradient()", "gradient_syntax"),
    ] {
        let r = bg(image);
        assert!(r.paint.gradient.is_none(), "{image}");
        assert_eq!(r.notes(), [format!("refused:{code}")], "{image}");
    }
}

#[test]
fn the_radial_gradient_of_the_export_is_carried_and_the_others_are_not() {
    let r = bg("radial-gradient(circle farthest-corner,#ff0000 0%,#0000ff 100%)");
    let g = r.gradient();
    assert_eq!(g.kind, GradientKind::Radial);
    assert_eq!(g.stops, stops(&[(0.0, "#ff0000"), (1.0, "#0000ff")]));
    assert!(near(g.angle, 180.0));
    assert!(r.report.entries.is_empty());
    for ok in ["circle, red, blue", "farthest-corner circle, red, blue"] {
        assert_eq!(
            bg(&format!("radial-gradient({ok})")).gradient().kind,
            GradientKind::Radial
        );
    }
    for (image, code) in [
        ("radial-gradient(red, blue)", "gradient_radial_shape"),
        (
            "radial-gradient(ellipse, red, blue)",
            "gradient_radial_shape",
        ),
        (
            "radial-gradient(ellipse farthest-corner, red, blue)",
            "gradient_radial_shape",
        ),
        (
            "radial-gradient(circle at top, red, blue)",
            "gradient_radial_position",
        ),
        (
            "radial-gradient(at center, red, blue)",
            "gradient_radial_position",
        ),
        (
            "radial-gradient(circle closest-side, red, blue)",
            "gradient_radial_size",
        ),
        (
            "radial-gradient(circle 20px, red, blue)",
            "gradient_radial_size",
        ),
        (
            "radial-gradient(circle banana, red, blue)",
            "gradient_radial_size",
        ),
        (
            "radial-gradient(circle circle, red, blue)",
            "gradient_radial_shape",
        ),
    ] {
        let r = bg(image);
        assert!(r.paint.gradient.is_none(), "{image}");
        assert_eq!(r.notes(), [format!("refused:{code}")], "{image}");
    }
}

#[test]
fn conic_repeating_and_unknown_layers_are_refused_by_kind() {
    for (image, code) in [
        ("conic-gradient(red, blue)", "gradient_kind"),
        ("repeating-linear-gradient(red, blue 10%)", "gradient_kind"),
        (
            "repeating-radial-gradient(circle, red, blue 10%)",
            "gradient_kind",
        ),
        (
            "-webkit-linear-gradient(red, blue)",
            "background_image_unreadable",
        ),
        ("image-set(\"a.png\" 1x)", "background_image_unreadable"),
        ("banana", "background_image_unreadable"),
        (
            "linear-gradient(red, blue) red",
            "background_image_unreadable",
        ),
    ] {
        let r = bg(image);
        assert!(
            r.paint.gradient.is_none() && r.paint.image.is_none(),
            "{image}"
        );
        assert_eq!(r.notes(), [format!("refused:{code}")], "{image}");
    }
    assert!(bg("none").report.entries.is_empty());
}

#[test]
fn a_second_gradient_is_refused_and_the_first_kept() {
    let r = bg("linear-gradient(90deg, red, blue), radial-gradient(circle, lime, white)");
    assert_eq!(r.gradient().kind, GradientKind::Linear);
    assert_eq!(r.notes(), ["refused:background_layer_extra"]);
}

#[test]
fn a_gradient_with_a_size_that_makes_a_pattern_is_not_carried() {
    for size in ["100% 100%", "100%", "auto", "auto auto", "cover", "contain"] {
        let r = read(&format!(
            "background-image:linear-gradient(red,blue);background-size:{size}"
        ));
        assert!(r.paint.gradient.is_some(), "{size}");
        assert!(r.report.entries.is_empty(), "{size}");
    }
    for size in ["20px 20px", "50%", "banana"] {
        let r = read(&format!(
            "background-image:linear-gradient(red,blue);background-size:{size}"
        ));
        assert!(r.paint.gradient.is_none(), "{size}");
        assert_eq!(r.notes(), ["refused:background_size"], "{size}");
    }
}

#[test]
fn a_gradient_comes_with_the_fill_and_through_var_and_the_shorthand() {
    let r = read("background-color:#fff;background-image:linear-gradient(red,blue)");
    assert_eq!(r.fill(), "#fff");
    assert!(r.paint.gradient.is_some());
    let r = read("background:linear-gradient(red,blue) #eee");
    assert_eq!(r.fill(), "#eee");
    assert!(r.paint.gradient.is_some());
    let r = read_in(
        "<div id=t style=\"background-image:var(--g)\">x</div>",
        ":root{--g:linear-gradient(to right, red, blue)}",
        &Ctx::default(),
    );
    assert!(near(r.gradient().angle, 90.0));
}
