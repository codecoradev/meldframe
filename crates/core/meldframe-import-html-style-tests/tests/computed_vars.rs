//! The computed style and `var()`: custom properties resolved where they are declared, invalid ones, the CSS-wide
//! keywords on them, a declaration that cannot be resolved, and `revert`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

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
fn var_is_replaced_from_the_custom_properties_the_element_sees() {
    let css = ":root{--c:#123456; --gap:4px; --b:var(--c); --loop:var(--loop); --long:var(--a); --a:var(--long)} \
               p{color:var(--c); padding:var(--gap) 8px; margin:var(--none); border-color:var(--b); \
               width:var(--none, 9px); height:var(--loop); top:calc(var(--gap) * 1px)} \
               span{--c:#abcdef; color:var(--c); left:var(--gap)}";
    let (p, st, _) = styled("<p><span>x</span></p>", css);
    let pp = st.get(el(&p, "p"));
    assert_eq!(pp.color, "#123456");
    assert_eq!(get(pp, "padding-top"), "4px");
    assert_eq!(get(pp, "padding-right"), "8px");
    // The refused shorthand changes nothing: the default margin of a paragraph is still there.
    assert_eq!(get(pp, "margin-top"), "<none>");
    assert_eq!(get(pp, "border-top-color"), "#123456");
    assert_eq!(
        (get(pp, "width"), get(pp, "height")),
        ("9px".into(), "<none>".into())
    );
    assert_eq!(get(pp, "top"), "calc(4px * 1px)");
    let codes: Vec<_> = pp
        .notes
        .iter()
        .map(|n| (n.property.as_str(), n.code))
        .collect();
    assert_eq!(
        codes,
        [("margin", "var_undefined"), ("height", "var_undefined")]
    );
    // A custom property is inherited, and an element's own shadows the inherited one for itself and its children.
    let sp = st.get(el(&p, "span"));
    assert_eq!(
        (sp.color.as_str(), get(sp, "left")),
        ("#abcdef", "4px".into())
    );
    assert_eq!(sp.custom["--gap"].as_deref(), Some("4px"));
    // The cycle is noted where it is declared, on the `html` of the fragment, and is the guaranteed-invalid value.
    let html = st.html.as_ref().unwrap();
    let cyc: Vec<_> = html
        .notes
        .iter()
        .map(|n| (n.property.as_str(), n.code))
        .collect();
    // `--a` and `--long` are each other's: the first one met is the cycle, the other finds the first invalid.
    assert_eq!(
        cyc,
        [
            ("--a", "var_too_deep"),
            ("--long", "var_undefined"),
            ("--loop", "var_too_deep")
        ]
    );
    assert_eq!(html.custom["--loop"], None);
    // The custom properties are not among the properties.
    assert!(pp.props.keys().all(|k| !k.starts_with("--")));
}

#[test]
fn a_custom_property_is_resolved_where_it_is_declared_not_where_it_is_used() {
    // The pattern of a theme: `--a` is built from `--b` at the root, and a child that sets `--b` does not change it.
    let css =
        ":root{--b:1px;--a:var(--b)} .x{--b:2px} .y{margin-left:var(--a); padding-left:var(--b)}";
    let html = "<div class=x><p class=y>t</p></div>";
    assert_eq!(on(html, css, "p", "margin-left"), "1px");
    assert_eq!(on(html, css, "p", "padding-left"), "2px");
    // Declared on the same element, in any order.
    let css = ".x{--a:var(--b);--b:var(--c);--c:3px} p{margin-left:var(--a)}";
    assert_eq!(
        on("<div class=x><p>t</p></div>", css, "p", "margin-left"),
        "3px"
    );
    // A dark theme that sets the base, and one that sets what is built from it.
    let base =
        ":root{--bg:#fff;--surface:var(--bg)} [data-theme=dark]{--bg:#000} p{color:var(--surface)}";
    let dark = "<div data-theme=dark><p>t</p></div>";
    assert_eq!(style_of(dark, base, "p").color, "#fff");
    let both = ":root{--bg:#fff;--surface:var(--bg)} [data-theme=dark]{--bg:#000;--surface:var(--bg)} p{color:var(--surface)}";
    assert_eq!(style_of(dark, both, "p").color, "#000");
    assert_eq!(style_of("<p>t</p>", both, "p").color, "#fff");
}

#[test]
fn a_custom_property_that_cannot_be_resolved_is_invalid_and_hides_the_one_it_inherits() {
    let css = ":root{--x:1px;--c:var(--c, 1px);--n:var(--nope)} div{--x:var(--nope)} \
               p{margin-left:var(--x, 9px); margin-right:var(--c, 4px); margin-top:var(--n, 5px); padding-left:var(--x)}";
    let (p, st, _) = styled("<div><p>t</p></div><p>u</p>", css);
    let ps: Vec<_> = p
        .tree
        .ids()
        .filter(|i| p.tree.element(*i).tag == "p")
        .collect();
    // Under the div, `--x` is invalid: the fallback is used, and with none the property is unset.
    let inner = st.get(ps[0]);
    assert_eq!(
        (get(inner, "margin-left"), get(inner, "padding-left")),
        ("9px".into(), "<none>".into())
    );
    // Beside it, `--x` is the root's.
    assert_eq!(get(st.get(ps[1]), "padding-left"), "1px");
    // A cycle through its own fallback is invalid too, and an unresolved reference is.
    assert_eq!(get(inner, "margin-right"), "4px");
    assert_eq!(get(inner, "margin-top"), "5px");
    let notes: Vec<_> = st
        .html
        .as_ref()
        .unwrap()
        .notes
        .iter()
        .map(|n| (n.property.as_str(), n.code))
        .collect();
    assert_eq!(notes, [("--c", "var_too_deep"), ("--n", "var_undefined")]);
}

#[test]
fn the_css_wide_keywords_on_a_custom_property() {
    let css = ":root{--x:1px} .i{--x:initial} .h{--x:inherit} .u{--x:unset} .r{--x:revert} p{margin-left:var(--x, 9px)}";
    let at = |class: &str| {
        let html = format!("<div class={class}><p>t</p></div>");
        on(&html, css, "p", "margin-left")
    };
    assert_eq!(at("i"), "9px");
    assert_eq!(at("h"), "1px");
    assert_eq!(at("u"), "1px");
    assert_eq!(at("r"), "1px");
    // `inherit` of what the parent made invalid is invalid, and of what it never had is nothing.
    let html = "<div class=i><section class=h><p>t</p></section></div>";
    assert_eq!(on(html, css, "p", "margin-left"), "9px");
    let css = "div{--y:inherit} p{margin-left:var(--y, 7px)}";
    assert_eq!(on("<div><p>t</p></div>", css, "p", "margin-left"), "7px");
    // A value that names one of them finds what it came to.
    let css = ":root{--x:1px} div{--x:initial;--z:var(--x, 2px)} p{margin-left:var(--z)}";
    assert_eq!(on("<div><p>t</p></div>", css, "p", "margin-left"), "2px");
}

#[test]
fn a_declaration_that_cannot_be_resolved_unsets_the_property_whatever_it_had() {
    let css = "p{margin-top:5px;color:#00f} .a{margin-top:var(--missing);color:var(--missing)}";
    let html = "<div style=\"color:#f00\"><p class=a>t</p><p>u</p></div>";
    let (p, st, _) = styled(html, css);
    let ps: Vec<_> = p
        .tree
        .ids()
        .filter(|i| p.tree.element(*i).tag == "p")
        .collect();
    // Unset: the margin is absent (not 5px, and not the default paragraph margin either), and the colour inherits.
    assert_eq!(get(st.get(ps[0]), "margin-top"), "<none>");
    assert_eq!(st.get(ps[0]).color, "#f00");
    assert_eq!(st.get(ps[0]).props["color"].source, Source::Inherited);
    assert_eq!(get(st.get(ps[1]), "margin-top"), "5px");
    // A shorthand unsets all its longhands.
    let css = "p{margin:5px 6px} .a{margin:var(--missing)}";
    let s = style_of("<p class=a>t</p>", css, "p");
    assert!(s.props.keys().all(|k| !k.starts_with("margin")));
    assert_eq!(s.notes.len(), 1);
}

#[test]
fn revert_gives_the_value_of_the_defaults_table_or_inherits() {
    assert_eq!(
        on("<h1>t</h1>", "h1{margin:revert}", "h1", "margin-top"),
        "0.67em"
    );
    assert_eq!(
        on(
            "<h1>t</h1>",
            "h1{margin:0} h1{margin-top:revert}",
            "h1",
            "margin-top"
        ),
        "0.67em"
    );
    assert_eq!(
        on(
            "<h1>t</h1>",
            "h1{margin:0} h1{margin-top:revert}",
            "h1",
            "margin-bottom"
        ),
        "0"
    );
    assert_eq!(
        style_of(
            "<h1>t</h1>",
            "h1{font-size:10px} h1{font-size:revert}",
            "h1"
        )
        .font_size,
        32.0
    );
    // With nothing in the table: the property is unset (a margin is absent, an inherited colour is inherited).
    assert_eq!(
        on(
            "<div><span>t</span></div>",
            "span{margin:5px} span{margin:revert}",
            "span",
            "margin-top"
        ),
        "<none>"
    );
    let s = style_of(
        "<div style=\"color:#f00\"><span>t</span></div>",
        "span{color:#00f} span{color:revert}",
        "span",
    );
    assert_eq!(s.color, "#f00");
}
