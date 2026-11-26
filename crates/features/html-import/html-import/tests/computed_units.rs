//! The computed style and its numbers: `em` and `rem`, `color`, `font-size` and `font-weight` and what refuses them,
//! and what inherits.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "style_common/mod.rs"]
mod css_common;
use css_common::*;
use html_import::css::Source;

/// The first `<tag>` of `html` with `css`, and what `prop` is on it.
fn on(html: &str, css: &str, tag: &str, prop: &str) -> String {
    get(&style_of(html, css, tag), prop)
}

fn get(s: &html_import::css::Style, p: &str) -> String {
    s.get(p).unwrap_or("<none>").to_owned()
}

#[test]
fn em_is_read_against_the_parent_for_font_size_and_the_element_for_the_rest_and_rem_is_16() {
    let html = "<div style=\"font-size:20px\"><p style=\"font-size:1.5em; padding:2em 1rem; margin:12pt\">x</p></div>";
    let s = style_of(html, "", "p");
    assert_eq!(s.font_size, 30.0);
    assert_eq!(s.len("padding-top").map(|l| l.px), Some(60.0));
    assert_eq!(
        s.len("padding-left").map(|l| (l.px, l.unit)),
        Some((16.0, html_import::css::value::Unit::Rem))
    );
    assert_eq!(s.len("margin-top").map(|l| l.px), Some(16.0));
    assert_eq!(s.len("display"), None);
    // The default size of a heading is read against the parent's.
    let s = style_of("<div style=\"font-size:10px\"><h2>t</h2></div>", "", "h2");
    assert_eq!(s.font_size, 15.0);
}

#[test]
fn what_color_font_size_and_font_weight_cannot_read_is_noted_and_the_inherited_value_stays() {
    let html = "<div style=\"color:#123456; font-size:20px; font-weight:600\"><p>x</p></div>";
    let cases = [
        ("color:lab(50% 0 0)", "color", "color_unreadable"),
        ("color:Canvas", "color", "color_unreadable"),
        ("font-size:large", "font-size", "font_size_unreadable"),
        ("font-size:50%", "font-size", "font_size_unreadable"),
        ("font-size:-1px", "font-size", "font_size_unreadable"),
        (
            "font-weight:bolder",
            "font-weight",
            "font_weight_unreadable",
        ),
        ("font-weight:0", "font-weight", "font_weight_unreadable"),
    ];
    for (decl, prop, code) in cases {
        let s = style_of(
            &html.replace("<p>", &format!("<p style=\"{decl}\">")),
            "",
            "p",
        );
        assert_eq!(
            (s.color.as_str(), s.font_size, s.font_weight),
            ("#123456", 20.0, 600.0),
            "{decl}"
        );
        assert_eq!(s.notes.len(), 1, "{decl}");
        assert_eq!(
            (s.notes[0].property.as_str(), s.notes[0].code),
            (prop, code),
            "{decl}"
        );
    }
    let s = style_of(
        &html.replace(
            "<p>",
            "<p style=\"color:currentcolor; font-weight:bold; font-size:12pt\">",
        ),
        "",
        "p",
    );
    assert_eq!(
        (s.color.as_str(), s.font_size, s.font_weight),
        ("#123456", 16.0, 700.0)
    );
    let s = style_of("<p style=\"color:rgb(0 0 0 / 50%)\">x</p>", "", "p");
    assert_eq!(s.color, "#00000080");
    assert_eq!(
        style_of("<p style=\"color:Transparent\">x</p>", "", "p").color,
        "#00000000"
    );
    // A value of 80 characters at most is kept in a note.
    let long = format!("<p style=\"color:{}\">", "x".repeat(300));
    assert_eq!(style_of(&long, "", "p").notes[0].value.chars().count(), 80);
}

#[test]
fn a_length_that_an_element_inherits_in_em_is_not_one_of_its_own() {
    let css = "div{font-size:20px; letter-spacing:0.5em; text-indent:1rem} p{font-size:10px}";
    let (p, st, _) = styled("<div><p>t</p></div>", css);
    let (d, c) = (st.get(el(&p, "div")), st.get(el(&p, "p")));
    assert_eq!(d.len("letter-spacing").map(|l| l.px), Some(10.0));
    // The text is inherited, but the 10 px are the div's, not the paragraph's 5.
    assert_eq!(c.get("letter-spacing"), Some("0.5em"));
    assert_eq!(c.len("letter-spacing"), None);
    // A rem does not depend on the element.
    assert_eq!(c.len("text-indent").map(|l| l.px), Some(16.0));
}

#[test]
fn rem_is_the_font_size_of_the_root_and_sixteen_for_the_root_itself() {
    let css =
        "html{font-size:20px} body{font-size:10px} p{padding:1rem; font-size:2rem; margin:1em}";
    let (p, st, _) = styled("<html><body><p>t</p></body></html>", css);
    let pp = st.get(el(&p, "p"));
    assert_eq!((pp.rem, pp.font_size), (20.0, 40.0));
    assert_eq!(pp.len("padding-top").map(|l| l.px), Some(20.0));
    assert_eq!(pp.len("margin-top").map(|l| l.px), Some(40.0));
    assert_eq!(st.get(el(&p, "html")).rem, 16.0);
    // The root's own size in rem is read against 16, and the virtual root of a fragment is the root too.
    let (p, st, _) = styled(
        "<html><p>t</p></html>",
        "html{font-size:2rem} p{width:1rem}",
    );
    assert_eq!(st.get(el(&p, "html")).font_size, 32.0);
    assert_eq!(st.get(el(&p, "p")).len("width").map(|l| l.px), Some(32.0));
    let (p, st, _) = styled("<p>t</p>", "html{font-size:10px} p{width:2rem}");
    assert_eq!(st.get(el(&p, "p")).len("width").map(|l| l.px), Some(20.0));
    // A root size that is not read is 16, and says so on the root.
    let (p, st, _) = styled(
        "<html><p>t</p></html>",
        "html{font-size:large} p{width:2rem}",
    );
    assert_eq!(st.get(el(&p, "p")).len("width").map(|l| l.px), Some(32.0));
    assert_eq!(st.get(el(&p, "html")).notes[0].code, "font_size_unreadable");
}

#[test]
fn every_inherited_property_passes_to_a_child_that_declares_nothing() {
    use html_import::css::defaults::INHERITED;
    // The list itself is pinned, so that dropping a name from it does not drop its check.
    assert_eq!(
        INHERITED,
        [
            "color",
            "cursor",
            "font-family",
            "font-size",
            "font-style",
            "font-weight",
            "letter-spacing",
            "line-height",
            "text-align",
            "text-indent",
            "text-transform",
            "visibility",
            "white-space",
            "word-break"
        ]
    );
    let sample = |p: &str| match p {
        "color" => "#123456",
        "cursor" => "pointer",
        "font-family" => "Georgia",
        "font-size" => "20px",
        "font-style" => "italic",
        "font-weight" => "600",
        "letter-spacing" => "2px",
        "line-height" => "1.5",
        "text-align" => "center",
        "text-indent" => "4px",
        "text-transform" => "uppercase",
        "visibility" => "hidden",
        "white-space" => "pre",
        "word-break" => "keep-all",
        other => panic!("no sample for {other}"),
    };
    let decls: String = INHERITED
        .iter()
        .map(|p| format!("{p}:{};", sample(p)))
        .collect();
    let (p, st, _) = styled(&format!("<div style=\"{decls}\"><p>t</p></div>"), "");
    let c = st.get(el(&p, "p"));
    for prop in INHERITED {
        let got = c
            .props
            .get(prop)
            .unwrap_or_else(|| panic!("{prop} is not inherited"));
        assert_eq!(
            (&*got.value, got.source),
            (sample(prop), Source::Inherited),
            "{prop}"
        );
    }
    let inherited = c
        .props
        .values()
        .filter(|p| p.source == Source::Inherited)
        .count();
    assert_eq!(inherited, INHERITED.len());
    // And a property that does not inherit does not.
    let s = style_of(
        "<div style=\"padding:4px;display:flex\"><span>t</span></div>",
        "",
        "span",
    );
    assert!(s.props.is_empty());
}

#[test]
fn unset_removes_a_declared_property_and_the_typed_readers_take_what_is_documented() {
    assert_eq!(
        on(
            "<p>t</p>",
            "p{padding:4px} p{padding:unset}",
            "p",
            "padding-left"
        ),
        "<none>"
    );
    // `currentcolor` is the inherited colour and is no refusal.
    let s = style_of(
        "<div style=\"color:#123456\"><p style=\"color:currentcolor\">t</p></div>",
        "",
        "p",
    );
    assert_eq!((s.color.as_str(), s.notes.len()), ("#123456", 0));
    // The weight is 1 to 1000.
    for (v, ok) in [
        ("1", true),
        ("1000", true),
        ("0.5", false),
        ("1001", false),
        ("0", false),
        ("900", true),
    ] {
        let s = style_of(&format!("<p style=\"font-weight:{v}\">t</p>"), "", "p");
        assert_eq!(s.notes.is_empty(), ok, "{v}");
    }
    // `calc(2px * 8)` is a length, so a size.
    assert_eq!(
        style_of("<p style=\"font-size:calc(2px * 8)\">t</p>", "", "p").font_size,
        16.0
    );
    assert_eq!(
        style_of("<p style=\"font-size:calc(2px * 3)\">t</p>", "", "p").font_size,
        6.0
    );
}
