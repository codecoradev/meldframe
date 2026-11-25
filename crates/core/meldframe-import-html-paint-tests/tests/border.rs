//! The border: the element's own CSS border (four equal solid sides, which takes room) and the export's border (the
//! `::after` box that takes none), with the position the inset says.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use meldframe_protocol::BorderPosition;

#[test]
fn four_equal_solid_sides_are_the_border_and_take_room() {
    let r = read("border:2px solid #ff0000");
    assert_eq!(r.paint.border.width, 2.0);
    assert_eq!(r.paint.border.color, "#ff0000");
    assert_eq!(r.paint.border.position, BorderPosition::Inside);
    assert_eq!(r.paint.border_room, 2.0);
    assert!(r.report.entries.is_empty());
    // The longhands say the same, and so does the colour as a function.
    let r = read("border-width:3px;border-style:solid;border-color:rgba(0,0,0,.5)");
    assert_eq!(
        (r.paint.border.width, r.paint.border.color.as_str()),
        (3.0, "#00000080")
    );
    assert_eq!(r.paint.border_room, 3.0);
}

#[test]
fn the_colour_left_out_is_the_colour_of_the_element() {
    assert_eq!(read("border:1px solid").paint.border.color, "#000000");
    // No colour declared at all (only a style and a width) is the same.
    let r = read("color:#336699;border-style:solid;border-width:1px");
    assert_eq!(r.paint.border.color, "#336699");
    assert_eq!(
        read("color:#336699;border:1px solid").paint.border.color,
        "#336699"
    );
    assert_eq!(
        read("color:#336699;border:1px solid currentcolor")
            .paint
            .border
            .color,
        "#336699"
    );
}

#[test]
fn units_and_keywords_of_the_width() {
    assert_eq!(read("border:0.5rem solid red").paint.border.width, 8.0);
    assert_eq!(
        read("font-size:20px;border:1.5em solid red")
            .paint
            .border
            .width,
        30.0
    );
    // The keywords are the widths every browser uses, and say so.
    for (w, px) in [("thin", 1.0), ("medium", 3.0), ("thick", 5.0)] {
        let r = read(&format!("border:{w} solid red"));
        assert_eq!(r.paint.border.width, px, "{w}");
        assert_eq!(r.notes(), ["listed:border_width_keyword"], "{w}");
    }
    // A style with no width is the medium width.
    let r = read("border-style:solid");
    assert_eq!(r.paint.border.width, 3.0);
    assert!(r.has("listed:border_width_keyword"));
    for bad in ["50%", "calc(1px + 2px)", "1vw", "-2px", "banana"] {
        let r = read(&format!("border-style:solid;border-width:{bad}"));
        assert_eq!(r.paint.border.width, 0.0, "{bad}");
        assert_eq!(r.paint.border_room, 0.0, "{bad}");
        assert_eq!(r.notes(), ["refused:border_width_unreadable"], "{bad}");
    }
}

#[test]
fn no_border_is_said_with_no_line() {
    for decls in [
        "",
        "border:none",
        "border:0 solid red",
        "border:2px hidden red",
        "border-width:2px",
        "border-color:red",
        "border-style:none;border-width:4px",
    ] {
        let r = read(decls);
        assert_eq!(r.paint.border.width, 0.0, "{decls}");
        assert_eq!(r.paint.border_room, 0.0, "{decls}");
        assert!(r.report.entries.is_empty(), "{decls}: {:?}", r.notes());
    }
}

#[test]
fn a_style_other_than_solid_is_refused_never_drawn_as_solid() {
    for style in [
        "dashed", "dotted", "double", "groove", "ridge", "inset", "outset", "banana",
    ] {
        let r = read(&format!("border:2px {style} red"));
        if style == "banana" {
            // The shorthand is refused whole by the style stage: nothing to say here.
            assert!(r.report.entries.is_empty());
        } else {
            assert_eq!(r.notes(), ["refused:border_style"], "{style}");
        }
        assert_eq!(r.paint.border.width, 0.0, "{style}");
    }
}

#[test]
fn sides_that_differ_are_refused_once() {
    for decls in [
        "border-top:2px solid red",
        "border:2px solid red;border-left-width:4px",
        "border:2px solid red;border-bottom-color:blue",
        "border:2px solid red;border-right-style:none",
        "border:2px solid red;border-right-style:dashed",
        "border-width:1px 2px;border-style:solid;border-color:red",
    ] {
        let r = read(decls);
        assert_eq!(r.paint.border.width, 0.0, "{decls}");
        assert_eq!(r.paint.border_room, 0.0, "{decls}");
        assert_eq!(r.notes(), ["refused:border_per_side"], "{decls}");
    }
}

#[test]
fn a_colour_that_is_not_read_is_refused() {
    let r = read("border-style:solid;border-width:1px;border-color:lab(50% 40 59)");
    assert_eq!(r.paint.border.width, 0.0);
    assert_eq!(r.notes(), ["refused:colour_unreadable"]);
}

fn after(rule: &str) -> Read {
    read_css(&format!("#t::after{{{rule}}}"))
}

const FORM: &str = "content:\"\";position:absolute;pointer-events:none;";

#[test]
fn the_export_border_is_read_with_its_position_from_the_inset() {
    for (inset, w, pos) in [
        ("0", 2.0, BorderPosition::Inside),
        ("0px", 3.0, BorderPosition::Inside),
        ("-1px", 2.0, BorderPosition::Center),
        ("-1.5px", 3.0, BorderPosition::Center),
        ("-2px", 2.0, BorderPosition::Outside),
        ("-3px", 3.0, BorderPosition::Outside),
    ] {
        let r = after(&format!("{FORM}inset:{inset};border:{w}px solid #f00"));
        assert_eq!(r.paint.border.width, w, "{inset}");
        assert_eq!(r.paint.border.position, pos, "{inset}");
        assert_eq!(r.paint.border.color, "#f00");
        // The box of the export takes no room.
        assert_eq!(r.paint.border_room, 0.0);
        assert!(r.report.entries.is_empty(), "{inset}: {:?}", r.notes());
    }
}

#[test]
fn an_inset_that_is_none_of_the_three_is_not_a_border_the_model_has() {
    for inset in ["-5px", "4px", "-0.5px", "10%"] {
        let r = after(&format!("{FORM}inset:{inset};border:2px solid red"));
        assert_eq!(r.paint.border.width, 0.0, "{inset}");
        assert!(r.has("refused:after_inset"), "{inset}: {:?}", r.notes());
    }
    // The four sides must say the same, and all four must be there.
    for rule in [
        "top:0;left:0;right:-1px;bottom:0",
        "top:0;left:0;right:0",
        "left:0",
    ] {
        let r = after(&format!("{FORM}{rule};border:2px solid red"));
        assert_eq!(r.paint.border.width, 0.0, "{rule}");
        assert!(r.has("refused:after_inset"), "{rule}");
    }
}

#[test]
fn a_box_that_is_not_the_export_form_is_not_read_as_a_border() {
    for rule in [
        "content:\"x\";position:absolute;inset:0;border:1px solid red",
        "position:absolute;inset:0;border:1px solid red",
        "content:\"\";position:relative;inset:0;border:1px solid red",
        "content:\"\";inset:0;border:1px solid red",
        "content:\"\";position:absolute;inset:0;border:1px dashed red",
    ] {
        let r = after(rule);
        assert_eq!(r.paint.border.width, 0.0, "{rule}");
        assert!(
            r.has("refused:after_not_border_form") || r.has("refused:border_style"),
            "{rule}: {:?}",
            r.notes()
        );
    }
    // A generated box with no border at all is not this stage's.
    let r = after("content:\"x\";color:red");
    assert!(r.report.entries.is_empty());
}

#[test]
fn the_other_properties_of_the_box_are_refused_by_name_and_the_border_is_still_read() {
    let r = after(&format!(
        "{FORM}inset:0;border:2px solid red;background:#00f;width:10px"
    ));
    assert_eq!(r.paint.border.width, 2.0);
    let names: Vec<&str> = r.report.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        r.notes(),
        ["refused:after_property", "refused:after_property"]
    );
    assert_eq!(names, ["background-color", "width"]);
}

#[test]
fn two_borders_are_one_and_the_export_box_is_the_one_kept() {
    let r = read_css(
        "#t{border:4px solid blue} #t::after{content:\"\";position:absolute;inset:-1px;border:2px solid red}",
    );
    assert_eq!(
        (r.paint.border.width, r.paint.border.color.as_str()),
        (2.0, "#ff0000")
    );
    assert_eq!(r.paint.border.position, BorderPosition::Center);
    assert_eq!(r.paint.border_room, 0.0);
    assert_eq!(r.notes(), ["refused:border_two_sources"]);
}

#[test]
fn the_tailwind_form_of_the_export_box_reads_the_same() {
    let html = "<div id=t class=\"after:content-[''] after:absolute after:pointer-events-none after:inset-[-1px] after:[border:2px_solid_#ff0000]\">x</div>";
    let r = read_in(html, "", &Default::default());
    assert_eq!(r.paint.border.width, 2.0);
    assert_eq!(r.paint.border.position, BorderPosition::Center);
    assert!(r.report.entries.is_empty(), "{:?}", r.notes());
}
