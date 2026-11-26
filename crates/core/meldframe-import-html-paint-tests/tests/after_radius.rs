//! The radius of the export's border box. The export writes the radius of the node on the `::after` box too: the
//! radius clamped to half the box, increased by how far the box grows, and written as a `calc(min(...))` of the
//! variable, the halves of the box and that growth when the radius is bound. The model has one radius, so the
//! box's is read to check the element's radius, and to give the radius when the element does not say it.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

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

/// The box of the export with `inset` and a border of `w`, and `radius` on the box; the element's own rule is `own`.
fn css(own: &str, inset: &str, w: f64, radius: &str) -> String {
    format!(
        "#t{{{own}}} #t::after{{content:\"\";position:absolute;pointer-events:none;inset:{inset};border:{w}px solid #f00;border-radius:{radius}}}"
    )
}

fn read_box(css: &str, ctx: &Ctx) -> Read {
    read_in("<div id=t>x</div>", css, ctx)
}

fn none() -> Ctx {
    Ctx::default()
}

#[test]
fn a_box_that_says_what_the_element_says_adds_nothing_and_says_nothing() {
    // Inside, centre and outside: the box is the radius plus 0, w/2 and w.
    for (inset, grow) in [("0", 0.0), ("-1px", 1.0), ("-2px", 2.0)] {
        let r = read_box(
            &css(
                "border-radius:8px",
                inset,
                2.0,
                &format!("{}px", 8.0 + grow),
            ),
            &none(),
        );
        assert_eq!(radius(&r), 8.0, "{inset}");
        assert!(r.report.entries.is_empty(), "{inset}: {:?}", r.notes());
    }
}

#[test]
fn the_box_of_a_radius_the_box_clamps_is_explained_by_the_size() {
    // A radius of 50 on a 40 by 20 box is drawn as 10 by the canvas and by CSS: the box says 10.
    let ctx = Ctx {
        width: Some(40.0),
        height: Some(20.0),
        ..Ctx::default()
    };
    let r = read_box(&css("border-radius:50px", "0", 2.0, "10px"), &ctx);
    assert_eq!(radius(&r), 50.0);
    assert!(r.report.entries.is_empty(), "{:?}", r.notes());
    // With no size, a smaller box radius cannot be checked and is accepted.
    let r = read_box(&css("border-radius:50px", "0", 2.0, "10px"), &none());
    assert_eq!(radius(&r), 50.0);
    assert!(r.report.entries.is_empty());
}

#[test]
fn the_bound_form_is_read_and_agrees_with_the_bound_radius_of_the_element() {
    let sheet = ":root{--r:6} #t{border-radius:calc(var(--r) * 1px)} #t::after{content:\"\";position:absolute;pointer-events:none;inset:-1px;border:2px solid #f00;border-radius:calc(min(var(--r) * 1px, 50px, 20px) + 1px)}";
    let r = read_box(sheet, &none());
    assert_eq!(radius(&r), 6.0);
    assert!(r.report.entries.is_empty(), "{:?}", r.notes());
}

#[test]
fn a_box_with_no_element_radius_gives_the_radius_and_says_so() {
    let r = read_box(&css("", "-1px", 2.0, "9px"), &none());
    assert_eq!(radius(&r), 8.0);
    assert_eq!(r.notes(), ["listed:radius_from_border"]);
    let sheet = ":root{--r:6} #t::after{content:\"\";position:absolute;inset:0;border:2px solid #f00;border-radius:calc(min(var(--r) * 1px, 50px, 20px) + 0px)}";
    let r = read_box(sheet, &none());
    assert_eq!(radius(&r), 6.0);
    assert_eq!(r.notes(), ["listed:radius_from_border"]);
    // A box with a zero radius is no radius.
    let r = read_box(&css("", "0", 2.0, "0px"), &none());
    assert_eq!(radius(&r), 0.0);
    assert!(r.report.entries.is_empty());
}

#[test]
fn a_box_that_says_another_radius_is_refused_and_the_element_keeps_its_own() {
    let r = read_box(&css("border-radius:8px", "0", 2.0, "20px"), &none());
    assert_eq!(radius(&r), 8.0);
    assert_eq!(r.notes(), ["refused:after_radius_differs"]);
    // With the size known, a smaller box radius is checked too.
    let ctx = Ctx {
        width: Some(100.0),
        height: Some(100.0),
        ..Ctx::default()
    };
    let r = read_box(&css("border-radius:8px", "0", 2.0, "4px"), &ctx);
    assert_eq!(radius(&r), 8.0);
    assert_eq!(r.notes(), ["refused:after_radius_differs"]);
    // The element says 0 and the box says 5.
    let r = read_box(&css("border-radius:0", "0", 2.0, "5px"), &none());
    assert_eq!(radius(&r), 0.0);
    assert_eq!(r.notes(), ["refused:after_radius_differs"]);
}

#[test]
fn the_ellipse_of_the_export_says_fifty_percent_on_both() {
    let r = read_box(&css("border-radius:50%", "0", 2.0, "50%"), &none());
    assert!(r.paint.ellipse);
    assert!(r.report.entries.is_empty(), "{:?}", r.notes());
    // Fifty percent on the box alone cannot be told from a radius without knowing how far the box grows.
    let r = read_box(&css("", "0", 2.0, "50%"), &none());
    assert!(!r.paint.ellipse);
    assert_eq!(r.notes(), ["refused:after_radius_percent"]);
    let r = read_box(&css("border-radius:8px", "0", 2.0, "50%"), &none());
    assert_eq!(radius(&r), 8.0);
    assert_eq!(r.notes(), ["refused:after_radius_differs"]);
    let r = read_box(&css("border-radius:50%", "0", 2.0, "8px"), &none());
    assert!(r.paint.ellipse);
    assert_eq!(r.notes(), ["refused:after_radius_differs"]);
}

#[test]
fn a_box_radius_that_is_not_read_is_refused_by_its_reason() {
    for (value, code) in [
        ("banana", "radius_unreadable"),
        ("25%", "radius_percent"),
        ("calc(1px + 2px)", "radius_unreadable"),
        ("calc(min(6px, banana) + 1px)", "radius_unreadable"),
        ("calc(min(6px, 50px) + banana)", "radius_unreadable"),
        ("calc(min(6px, 50px) + 1px + 2px)", "radius_unreadable"),
        ("calc(min() + 1px)", "radius_unreadable"),
        ("calc(max(6px, 50px) + 1px)", "radius_unreadable"),
        ("calc(min(6px, 50px) * 2)", "radius_unreadable"),
        ("calc(min(6px, 50px) - 9px)", "radius_unreadable"),
        ("calc(min(6px, 50px)", "radius_unreadable"),
        ("calc(min(6px, 50px) 1px)", "radius_unreadable"),
        (
            "calc(min(1px, 2px, 3px, 4px, 5px, 6px, 7px, 8px, 9px) + 0px)",
            "radius_unreadable",
        ),
        ("1px 2px 3px 4px", "radius_per_corner"),
    ] {
        let r = read_box(&css("border-radius:8px", "0", 2.0, value), &none());
        assert_eq!(radius(&r), 8.0, "{value}");
        assert_eq!(r.notes(), [format!("refused:{code}")], "{value}");
        assert!(r.report.entries[0].name.contains("::after"), "{value}");
    }
}

#[test]
fn a_box_radius_next_to_an_element_radius_that_is_not_read_says_both() {
    let r = read_box(&css("border-radius:banana", "0", 2.0, "5px"), &none());
    assert_eq!(radius(&r), 0.0);
    assert_eq!(
        r.notes(),
        ["refused:radius_unreadable", "refused:after_radius_differs"]
    );
}

#[test]
fn the_min_form_reads_in_any_spacing_and_with_a_number_on_either_side() {
    for (value, want) in [
        ("calc(min(6 * 1px, 50px, 20px) + 0px)", 6.0),
        ("calc( min( 6px , 50px ) + 0px )", 6.0),
        ("calc(min(1px * 6, 50px, 20px) + 1.5px)", 7.5),
        ("calc(min(30px, 50px, 20px) + 0px)", 20.0),
        ("calc(min(6px, 50px))", 6.0),
        ("calc(min(6px, 50px) - 1px)", 5.0),
        ("calc(min(0.5rem, 50px) + 0px)", 8.0),
        ("CALC(MIN(6px, 50px) + 0PX)", 6.0),
    ] {
        let r = read_box(&css("", "0", 2.0, value), &none());
        assert_eq!(radius(&r), want, "{value}");
    }
}

#[test]
fn a_radius_on_a_box_that_is_not_the_border_is_not_read() {
    // The element has its own border: the `::after` here has no border of its own, so it is not the export's box
    // and its radius is nobody's here.
    let r = read_box(
        "#t{border:2px solid red;border-radius:8px} #t::after{content:\"x\";border-radius:20px}",
        &none(),
    );
    assert_eq!(radius(&r), 8.0);
    assert!(r.report.entries.is_empty(), "{:?}", r.notes());
}
