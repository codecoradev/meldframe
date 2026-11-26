//! The corner radius: four equal corners as a number, `50%` as an ellipse or half the smaller side, and the forms that
//! the model has no field for.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "paint_common/mod.rs"]
mod common;
use common::*;
use html_import::paint::Ctx;
use meldframe_protocol::Bind;

fn radius(r: &Read) -> f64 {
    match r.paint.radius {
        Bind::Lit(v) => v,
        Bind::Var(_) => panic!("a bound radius"),
    }
}

#[test]
fn a_radius_in_pixels_on_four_equal_corners_is_the_radius() {
    for (decls, want) in [
        ("border-radius:8px", 8.0),
        ("border-radius:8px 8px 8px 8px", 8.0),
        ("border-radius:0.5rem", 8.0),
        ("font-size:10px;border-radius:2em", 20.0),
        ("border-radius:4pt", 4.0 * 4.0 / 3.0),
        (
            "border-top-left-radius:6px;border-top-right-radius:6px;border-bottom-right-radius:6px;border-bottom-left-radius:6px",
            6.0,
        ),
        ("border-radius:0", 0.0),
        ("border-radius:9999px", 9999.0),
        ("border-radius:5px 5px", 5.0),
    ] {
        let r = read(decls);
        assert_eq!(radius(&r), want, "{decls}");
        assert!(
            !r.paint.ellipse && r.report.entries.is_empty(),
            "{decls}: {:?}",
            r.notes()
        );
    }
}

#[test]
fn the_bound_radius_of_the_export_is_read_once_the_variable_is_replaced() {
    let r = read_in(
        "<div id=t style=\"border-radius:calc(var(--r) * 1px)\">x</div>",
        ":root{--r:6}",
        &Ctx::default(),
    );
    assert_eq!(radius(&r), 6.0);
    assert!(r.report.entries.is_empty());
}

#[test]
fn a_radius_past_what_the_model_holds_is_capped_and_listed() {
    let r = read("border-radius:1000000px");
    assert_eq!(radius(&r), 100_000.0);
    assert_eq!(r.notes(), ["listed:radius_capped"]);
}

#[test]
fn corners_that_differ_are_refused_and_the_radius_stays_zero() {
    for decls in [
        "border-radius:8px 0 0 8px",
        "border-radius:8px 4px",
        "border-top-left-radius:8px",
        "border-radius:8px;border-bottom-left-radius:0",
        "border-radius:50% 8px 8px 8px",
    ] {
        let r = read(decls);
        assert_eq!(radius(&r), 0.0, "{decls}");
        assert!(!r.paint.ellipse, "{decls}");
        assert_eq!(r.notes(), ["refused:radius_per_corner"], "{decls}");
    }
}

#[test]
fn an_elliptical_corner_is_refused() {
    let r = read(
        "border-top-left-radius:10px 20px;border-top-right-radius:10px 20px;border-bottom-right-radius:10px 20px;border-bottom-left-radius:10px 20px",
    );
    assert_eq!(radius(&r), 0.0);
    assert_eq!(r.notes(), ["refused:radius_elliptical"]);
    // The shorthand with a slash is refused whole by the style stage, which lists it: nothing here.
    let r = read("border-radius:10px / 20px");
    assert_eq!(radius(&r), 0.0);
    assert!(r.report.entries.is_empty());
    assert!(r.style.notes.iter().any(|n| n.code == "elliptical_radius"));
}

#[test]
fn fifty_percent_is_an_ellipse_when_nothing_is_inside() {
    let r = read("border-radius:50%");
    assert!(r.paint.ellipse);
    assert_eq!(radius(&r), 0.0);
    assert!(r.report.entries.is_empty());
    // The four longhands say the same.
    let r = read(
        "border-top-left-radius:50%;border-top-right-radius:50%;border-bottom-right-radius:50%;border-bottom-left-radius:50%",
    );
    assert!(r.paint.ellipse);
}

#[test]
fn fifty_percent_on_a_box_with_content_is_half_the_smaller_authored_side() {
    let with = |w, h| Ctx {
        width: w,
        height: h,
        has_children: true,
    };
    let r = read_ctx("border-radius:50%", &with(Some(100.0), Some(60.0)));
    assert!(!r.paint.ellipse);
    assert_eq!(radius(&r), 30.0);
    assert!(r.report.entries.is_empty());
    // A size that is not authored in pixels cannot be halved.
    for ctx in [
        with(Some(100.0), None),
        with(None, Some(60.0)),
        with(None, None),
    ] {
        let r = read_ctx("border-radius:50%", &ctx);
        assert_eq!(radius(&r), 0.0);
        assert!(!r.paint.ellipse);
        assert_eq!(r.notes(), ["refused:radius_percent_size"]);
    }
}

#[test]
fn another_percentage_and_a_word_that_is_not_a_length_are_refused() {
    for (decls, code) in [
        ("border-radius:25%", "radius_percent"),
        ("border-radius:100%", "radius_percent"),
        ("border-radius:banana", "radius_unreadable"),
        ("border-radius:10vw", "radius_unreadable"),
        ("border-radius:calc(1px + 2px)", "radius_unreadable"),
        ("border-radius:-4px", "radius_unreadable"),
        (
            "border-top-left-radius:banana;border-top-right-radius:banana;border-bottom-right-radius:banana;border-bottom-left-radius:banana",
            "radius_unreadable",
        ),
    ] {
        let r = read(decls);
        assert_eq!(radius(&r), 0.0, "{decls}");
        assert!(!r.paint.ellipse, "{decls}");
        assert_eq!(r.notes(), [format!("refused:{code}")], "{decls}");
    }
}

#[test]
fn an_ellipse_keeps_its_fill_and_border() {
    let r = read("border-radius:50%;background-color:#0f0;border:2px solid red");
    assert!(r.paint.ellipse);
    assert_eq!(r.fill(), "#0f0");
    assert_eq!(r.paint.border.width, 2.0);
}
