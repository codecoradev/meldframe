//! The computed style: inheritance, the browser's defaults, `var()`, shorthands, the CSS-wide keywords, the three
//! numbers (`color`, `font-size`, `font-weight`), the generated boxes, fragments, and the budget of the stage.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::{Limits, Source};

fn get(s: &html_import::css::Style, p: &str) -> String {
    s.get(p).unwrap_or("<none>").to_owned()
}

#[test]
fn the_inherited_properties_pass_down_and_the_others_do_not() {
    let html = "<div id=d><p><span>x</span></p></div>";
    let css = "#d{color:#f00; font-size:20px; font-weight:700; text-align:center; white-space:pre; \
               font-family:Georgia; line-height:1.5; visibility:hidden; padding:4px; background-color:#0f0; \
               width:50px; display:flex}";
    let s = style_of(html, css, "span");
    assert_eq!(
        (s.color.as_str(), s.font_size, s.font_weight),
        ("#f00", 20.0, 700.0)
    );
    assert_eq!(
        props(&s),
        [
            "color:#f00^",
            "font-family:Georgia^",
            "font-size:20px^",
            "font-weight:700^",
            "line-height:1.5^",
            "text-align:center^",
            "visibility:hidden^",
            "white-space:pre^"
        ]
    );
    // The element that declares them has them as its own, and only those are `declared`.
    let d = style_of(html, css, "div");
    let own: Vec<&str> = d.declared().map(|(k, _)| k).collect();
    assert_eq!(own.len(), 15);
    assert!(own.contains(&"display") && own.contains(&"padding-top"));
    assert!(d.props["color"].source == Source::Author);
    // A tree with nothing says: black, 16 px, weight 400.
    let s = style_of("<p>x</p>", "", "p");
    assert_eq!(
        (s.color.as_str(), s.font_size, s.font_weight),
        ("#000000", 16.0, 400.0)
    );
}

#[test]
fn the_browsers_defaults_are_the_small_table_and_author_rules_beat_them() {
    let html = "<body><h1>t</h1><h3>u</h3><p>a</p><ul><li>b</li></ul><strong>s</strong>\
                <a href=x>l</a><a>m</a><div hidden>h</div><span>i</span><table><tr><th>c</th></tr></table></body>";
    let (p, st, _) = styled(html, "");
    let at = |tag: &str| st.get(el(&p, tag));
    let body = at("body");
    assert_eq!(body.len("margin-top").map(|l| l.px), Some(8.0));
    assert_eq!(get(body, "display"), "block");
    let h1 = at("h1");
    assert_eq!((h1.font_size, h1.font_weight), (32.0, 700.0));
    assert_eq!(get(h1, "margin-top"), "0.67em");
    assert!((h1.len("margin-top").unwrap().px - 21.44).abs() < 1e-9);
    assert_eq!(at("h3").font_size, 16.0 * 1.17);
    assert_eq!(at("p").len("margin-bottom").map(|l| l.px), Some(16.0));
    assert_eq!(get(at("ul"), "padding-left"), "40px");
    assert_eq!(get(at("li"), "display"), "list-item");
    assert_eq!(at("strong").font_weight, 700.0);
    assert_eq!(at("a").color, "#0000ee");
    assert_eq!(get(at("span"), "display"), "<none>");
    assert_eq!(get(at("div"), "display"), "none");
    assert_eq!(
        (get(at("tr"), "display"), at("th").font_weight),
        ("table-row".into(), 700.0)
    );
    assert_eq!(get(at("th"), "text-align"), "center");
    // The second `a` has no href, so it is not a link.
    let second = p
        .tree
        .ids()
        .filter(|i| p.tree.element(*i).tag == "a")
        .nth(1)
        .unwrap();
    assert_eq!(st.get(second).color, "#000000");
    // The head, scripts and styles are not shown.
    let (p, st, _) = styled(
        "<head><title>t</title><style>p{color:red}</style></head><p>x</p>",
        "",
    );
    assert_eq!(get(st.get(el(&p, "head")), "display"), "none");
    assert_eq!(get(st.get(el(&p, "style")), "display"), "none");
    // A rule of the author wins over the table, whatever its specificity.
    let s = style_of(
        "<h1>t</h1>",
        "h1{font-size:10px; margin:0; font-weight:400} * {display:inline}",
        "h1",
    );
    assert_eq!((s.font_size, s.font_weight), (10.0, 400.0));
    assert_eq!(
        (get(&s, "margin-top"), get(&s, "display")),
        ("0".into(), "inline".into())
    );
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
    assert_eq!(get(pp, "margin-top"), "1em");
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
        [("margin", "var_undefined"), ("height", "var_too_deep")]
    );
    // A custom property is inherited, and an element's own shadows the inherited one for itself and its children.
    let sp = st.get(el(&p, "span"));
    assert_eq!(
        (sp.color.as_str(), get(sp, "left")),
        ("#abcdef", "4px".into())
    );
    assert_eq!(sp.custom["--gap"].as_ref(), "4px");
    // The custom properties are not among the properties.
    assert!(pp.props.keys().all(|k| !k.starts_with("--")));
}

#[test]
fn a_later_longhand_beats_an_earlier_shorthand_and_the_cascade_decides_between_them() {
    let get4 = |css: &str| {
        let s = style_of("<p class=a>x</p>", css, "p");
        ["top", "right", "bottom", "left"]
            .map(|side| get(&s, &format!("margin-{side}")))
            .join(" ")
    };
    assert_eq!(get4("p{margin:0} p{margin-top:5px}"), "5px 0 0 0");
    assert_eq!(
        get4("p{margin-top:5px} p{margin:1px 2px}"),
        "1px 2px 1px 2px"
    );
    assert_eq!(
        get4("p{margin-top:5px !important} p{margin:0}"),
        "5px 0 0 0"
    );
    assert_eq!(get4(".a{margin:0} p{margin-top:5px}"), "0 0 0 0");
    assert_eq!(get4("p{margin:0 auto}"), "0 auto 0 auto");
    // The default margin of a paragraph is under all of them, and a shorthand that is refused changes nothing.
    assert_eq!(get4("p{margin:1px 2px 3px 4px 5px}"), "1em 0 1em 0");
    let s = style_of(
        "<p>x</p>",
        "p{border:1px solid red; border-left:none; border-top-color:blue}",
        "p",
    );
    assert_eq!(
        ["top", "left", "right"].map(|d| format!(
            "{} {} {}",
            get(&s, &format!("border-{d}-width")),
            get(&s, &format!("border-{d}-style")),
            get(&s, &format!("border-{d}-color"))
        )),
        [
            "1px solid blue",
            "medium none currentcolor",
            "1px solid red"
        ]
    );
    let s = style_of("<p>x</p>", "p{margin:1 2 3 4 5; flex:1 1 1 1}", "p");
    assert_eq!(
        s.notes
            .iter()
            .map(|n| (n.property.as_str(), n.code))
            .collect::<Vec<_>>(),
        [("margin", "too_many_values"), ("flex", "flex_shorthand")]
    );
}

#[test]
fn the_css_wide_keywords_are_read_for_what_they_are() {
    let html = "<div style=\"color:#f00; text-align:center; padding:4px; display:flex; white-space:pre\">\
                <p style=\"STYLE\">x</p></div>";
    let of = |style: &str| style_of(&html.replace("STYLE", style), "", "p");
    assert_eq!(of("color:inherit").color, "#f00");
    assert_eq!(of("color:initial").color, "#000000");
    assert_eq!(of("text-align:initial").get("text-align"), Some("start"));
    assert_eq!(
        of("text-align:unset; white-space:revert").get("text-align"),
        Some("center")
    );
    assert_eq!(
        of("text-align:unset").props["text-align"].source,
        Source::Inherited
    );
    assert_eq!(of("padding:inherit").get("padding-left"), Some("4px"));
    assert_eq!(
        of("padding:inherit").props["padding-left"].source,
        Source::Inline
    );
    assert_eq!(of("padding:unset").get("padding-left"), None);
    assert_eq!(of("padding:initial").get("padding-left"), None);
    assert_eq!(of("display:inherit").get("display"), Some("flex"));
    assert_eq!(of("font-size:initial").font_size, 16.0);
    // `inherit` on a property the parent has none of leaves it out.
    assert_eq!(of("width:inherit").get("width"), None);
}

#[test]
fn the_generated_boxes_have_declared_properties_only_with_var_and_shorthands_read() {
    let css = ":root{--w:2px} [data-node=n1]{color:#f00} \
               [data-node=n1]::after{content:\"\"; position:absolute; pointer-events:none; inset:-1px; \
               border:var(--w) solid #000; border-radius:5px; color:inherit; font-size:99px}";
    let s = style_of("<div data-node=n1 style=\"top:0\">t</div>", css, "div");
    let after = s.after.expect("::after");
    let names: Vec<&str> = after.keys().map(|k| &**k).collect();
    assert!(
        names.contains(&"content")
            && names.contains(&"left")
            && names.contains(&"border-top-width")
    );
    assert!(names.contains(&"border-bottom-right-radius"));
    assert_eq!(&*after["content"].value, "\"\"");
    assert_eq!(&*after["left"].value, "-1px");
    assert_eq!(&*after["border-left-width"].value, "2px");
    // Nothing is taken from the element: `color:inherit` finds no parent, and `top:0` of the style attribute is
    // not there (the `top` is the one of `inset`).
    assert!(!after.contains_key("color") && &*after["top"].value == "-1px");
    assert!(s.before.is_none());
    assert_eq!(s.font_size, 16.0);
}

#[test]
fn a_fragment_inherits_from_the_body_and_html_it_does_not_have() {
    let css = "html{--x:3px; color:#00f} body{font-size:10px; font-weight:500; margin:0} p{padding:var(--x); margin-top:2em}";
    let (p, st, _) = styled("<p>a<span>b</span></p><div>c</div>", css);
    let pp = st.get(el(&p, "p"));
    assert_eq!(
        (pp.color.as_str(), pp.font_size, pp.font_weight),
        ("#00f", 10.0, 500.0)
    );
    assert_eq!(
        (get(pp, "padding-top"), pp.len("margin-top").map(|l| l.px)),
        ("3px".into(), Some(20.0))
    );
    assert_eq!(st.get(el(&p, "div")).color, "#00f");
    assert_eq!(st.body.as_ref().unwrap().font_size, 10.0);
    assert_eq!(get(st.body.as_ref().unwrap(), "margin-top"), "0");
    assert!(st.html.is_some());
    // The page of a fragment is the table's: the body has its 8 px margin until a rule says otherwise.
    let (_, st, _) = styled("<p>a</p>", "");
    assert_eq!(get(st.body.as_ref().unwrap(), "margin-top"), "8px");
    // A document has its elements and nothing is made up.
    let (p, st, _) = styled("<html><body><p>a</p></body></html>", css);
    assert!(st.html.is_none() && st.body.is_none());
    assert_eq!(st.get(el(&p, "p")).font_size, 10.0);
}

#[test]
fn the_exports_own_stylesheet_and_a_node_rule_are_read_as_the_export_wrote_them() {
    let css = "*,::before,::after{box-sizing:border-box}body{margin:0;font-family:'Inter Variable'}p{margin:0}\n\
        :root{--c-brand:#3366ff}\n\
        [data-node=\"n1\"]{position:absolute;left:10px;top:20px;width:100px;height:auto;display:flex;flex-direction:row;\
        gap:8px;padding:4px 8px;align-items:center;justify-content:safe center;overflow:hidden;\
        background-color:var(--c-brand);border-radius:calc(var(--c-brand) * 1px)}\n\
        [data-node=\"n2\"]{font-size:16px;font-weight:600;line-height:19px;white-space:pre;word-break:keep-all;\
        text-align:center;color:#112233;width:auto;min-width:40px;margin:0 0 0 4px}";
    let (p, st, _) = styled("<div data-node=n1><p data-node=n2>x</p></div>", css);
    let d = st.get(el(&p, "div"));
    assert_eq!(get(d, "left"), "10px");
    assert_eq!(get(d, "justify-content"), "safe center");
    assert_eq!(get(d, "padding-right"), "8px");
    assert_eq!(get(d, "background-color"), "#3366ff");
    assert_eq!(get(d, "border-top-left-radius"), "calc(#3366ff * 1px)");
    let t = st.get(el(&p, "p"));
    assert_eq!(
        (t.font_size, t.font_weight, t.color.as_str()),
        (16.0, 600.0, "#112233")
    );
    assert_eq!(
        (get(t, "margin-left"), get(t, "margin-top")),
        ("4px".into(), "0".into())
    );
    assert_eq!(get(t, "white-space"), "pre");
    assert_eq!(get(t, "font-family"), "'Inter Variable'");
    assert!(d.notes.iter().all(|n| n.property == "border-radius"));
}

#[test]
fn the_extra_css_comes_after_the_style_blocks() {
    let (p, st, _) = styled(
        "<style>p{color:#111111; margin-top:1px}</style><p>x</p>",
        "p{color:#222222}",
    );
    let pp = st.get(el(&p, "p"));
    assert_eq!(
        (pp.color.as_str(), get(pp, "margin-top")),
        ("#222222", "1px".into())
    );
}

#[test]
fn a_style_stage_over_its_budget_leaves_the_elements_past_it_with_the_inherited_values_only() {
    let html = format!(
        "<div style=\"color:#123456\">{}</div>",
        "<p style=\"--a:1; width:2px\">x</p>".repeat(100)
    );
    let limits = Limits {
        max_work: 60,
        ..Limits::default()
    };
    let (p, st, r) = styled_with(&html, "", &limits);
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    let ps: Vec<_> = p
        .tree
        .ids()
        .filter(|i| p.tree.element(*i).tag == "p")
        .collect();
    let done = ps
        .iter()
        .filter(|i| st.get(**i).get("width").is_some())
        .count();
    assert!(0 < done && done < 100, "{done}");
    // The cut ones still inherit.
    assert_eq!(st.get(*ps.last().unwrap()).color, "#123456");
    assert!(st.get(*ps.last().unwrap()).get("width").is_none());
}

#[test]
fn custom_properties_are_shared_until_an_element_adds_one_and_copying_them_is_counted() {
    let root: String = (0..2_000).map(|i| format!("--r{i}:{i};")).collect();
    let css = format!(":root{{{root}}} p{{--own:1}}");
    let html = "<p>x</p>".repeat(3_000);
    let (_, st, r) = styled(&html, &css);
    assert!(!st.complete);
    assert_eq!(count(&r, "style_work_limit"), 1);
    // Not a copy of two thousand properties for each of three thousand elements.
    assert!(r.work < 3 * Limits::default().max_work, "{}", r.work);
    let (p, st, _) = styled("<div><p>x</p></div><p>y</p>", &css);
    let ps: Vec<_> = p
        .tree
        .ids()
        .filter(|i| p.tree.element(*i).tag == "p")
        .collect();
    assert_eq!(st.get(ps[0]).custom.len(), 2_001);
    let div = st.get(el(&p, "div"));
    assert!(std::sync::Arc::ptr_eq(
        &div.custom,
        &st.body.as_ref().unwrap().custom
    ));
}

#[test]
fn the_notes_of_an_element_are_bounded() {
    let decls: String = (0..40).map(|i| format!("width:var(--n{i});")).collect();
    let (p, st, r) = styled(&format!("<p style=\"{decls}\">x</p>"), "");
    assert_eq!(st.get(el(&p, "p")).notes.len(), 32);
    assert_eq!(count(&r, "note_omitted"), 8);
}
