//! The paint properties that have no field in the model, and the ones of behaviour: refused by name, with the
//! initial value read as no declaration.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::paint::handles;

#[test]
fn a_paint_property_with_no_field_is_refused_by_name() {
    for decl in [
        "filter:blur(2px)",
        "backdrop-filter:blur(2px)",
        "mix-blend-mode:multiply",
        "mask:url(#m)",
        "mask-image:linear-gradient(red,blue)",
        "clip-path:circle(50%)",
        "isolation:isolate",
        "outline:2px solid red",
        "outline-style:solid",
        "outline-offset:2px",
        "border-image:url(a.png) 30",
        "border-image-source:linear-gradient(red,blue)",
        "background-clip:text",
        "background-clip:padding-box",
        "background-attachment:fixed",
        "background-blend-mode:multiply",
        "background-origin:border-box",
        "border-block:1px solid red",
        "border-inline-start-width:2px",
    ] {
        let r = read(decl);
        let name = decl.split(':').next().unwrap();
        assert_eq!(r.notes(), ["refused:refused_not_modelled"], "{decl}");
        assert_eq!(r.report.entries[0].name, name, "{decl}");
        assert_eq!(r.paint.border.width, 0.0, "{decl}");
    }
}

#[test]
fn the_initial_value_is_no_declaration() {
    for decl in [
        "filter:none",
        "backdrop-filter:none",
        "mix-blend-mode:normal",
        "mask:none",
        "clip-path:none",
        "isolation:auto",
        "outline:none",
        "outline:0",
        "outline-style:none",
        "background-clip:border-box",
        "background-attachment:scroll",
        "background-origin:padding-box",
        "border-image:none",
        "border-image-slice:30",
        "cursor:auto",
        "transition:none",
        "pointer-events:auto",
    ] {
        let r = read(decl);
        assert!(
            r.report.entries.is_empty() && r.report.counts.is_empty(),
            "{decl}"
        );
    }
}

#[test]
fn behaviour_is_counted_once_by_property_and_has_no_line() {
    for (decl, code) in [
        ("cursor:pointer", "behaviour_cursor"),
        ("transition:all .3s", "behaviour_transition"),
        ("transition-duration:.3s", "behaviour_transition"),
        ("animation:spin 1s infinite", "behaviour_animation"),
        ("pointer-events:none", "behaviour_pointer_events"),
        ("user-select:none", "behaviour_user_select"),
        ("will-change:transform", "behaviour_will_change"),
    ] {
        let r = read(decl);
        assert!(r.report.entries.is_empty(), "{decl}");
        assert_eq!(
            r.report.counts.get(code),
            Some(&1),
            "{decl}: {:?}",
            r.report.counts
        );
    }
    // `cursor` is inherited: the child that only inherits it counts nothing.
    let r = read_in(
        "<div style=\"cursor:pointer\"><b id=t>x</b></div>",
        "",
        &Default::default(),
    );
    assert!(r.report.counts.is_empty());
}

#[test]
fn a_rule_and_the_style_attribute_count_alike_and_the_browser_defaults_count_nothing() {
    let r = read_in(
        "<button id=t>x</button>",
        "button{cursor:pointer}",
        &Default::default(),
    );
    assert_eq!(r.report.counts.get("behaviour_cursor"), Some(&1));
    let r = read_in("<button id=t>x</button>", "", &Default::default());
    assert!(r.report.counts.is_empty() && r.report.entries.is_empty());
}

#[test]
fn handles_names_what_this_stage_has_an_answer_for() {
    for yes in [
        "background-color",
        "background-image",
        "background-size",
        "background-position",
        "background-repeat",
        "background-clip",
        "border-top-width",
        "border-left-style",
        "border-bottom-color",
        "border-top-left-radius",
        "border-block",
        "border-image-slice",
        "box-shadow",
        "opacity",
        "filter",
        "outline-offset",
        "mask-image",
        "cursor",
        "transition-delay",
        "animation-name",
        "pointer-events",
    ] {
        assert!(handles(yes), "{yes}");
    }
    for no in [
        "width",
        "margin-top",
        "color",
        "display",
        "font-size",
        "z-index",
        "border-collapse",
        "border-spacing",
        "visibility",
        "transform",
    ] {
        assert!(!handles(no), "{no}");
    }
}
