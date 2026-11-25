//! `opacity`: a number or a percentage from 0 to 1, clamped and listed when it is outside.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::Bind;

fn opacity(r: &Read) -> f64 {
    match r.paint.opacity {
        Bind::Lit(v) => v,
        Bind::Var(_) => panic!("a bound opacity"),
    }
}

#[test]
fn a_number_or_a_percentage_is_the_opacity() {
    for (css, want) in [
        ("0.5", 0.5),
        (".25", 0.25),
        ("1", 1.0),
        ("0", 0.0),
        ("50%", 0.5),
        ("12.5%", 0.125),
        ("+0.5", 0.5),
    ] {
        let r = read(&format!("opacity:{css}"));
        assert_eq!(opacity(&r), want, "{css}");
        assert!(r.report.entries.is_empty(), "{css}");
    }
    assert_eq!(opacity(&read("")), 1.0);
}

#[test]
fn a_value_outside_the_range_is_clamped_as_css_does_and_listed() {
    for (css, want) in [("1.5", 1.0), ("-0.2", 0.0), ("150%", 1.0), ("-5%", 0.0)] {
        let r = read(&format!("opacity:{css}"));
        assert_eq!(opacity(&r), want, "{css}");
        assert_eq!(r.notes(), ["listed:opacity_clamped"], "{css}");
    }
}

#[test]
fn what_is_not_a_number_is_refused_and_the_box_stays_opaque() {
    for bad in [
        "banana",
        "calc(0.5)",
        "1e-1",
        "50px",
        "0.5.5",
        "-",
        "%",
        "half",
    ] {
        let r = read(&format!("opacity:{bad}"));
        assert_eq!(opacity(&r), 1.0, "{bad}");
        assert_eq!(r.notes(), ["refused:opacity_unreadable"], "{bad}");
    }
}

#[test]
fn a_bound_opacity_of_the_export_is_read_once_the_variable_is_replaced() {
    let r = read_in(
        "<div id=t style=\"opacity:var(--o)\">x</div>",
        ":root{--o:0.5}",
        &Default::default(),
    );
    assert_eq!(opacity(&r), 0.5);
}
