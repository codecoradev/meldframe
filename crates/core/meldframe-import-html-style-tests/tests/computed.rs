//! The computed style: inheritance, the browser's defaults, shorthands in order, the CSS-wide keywords, the generated
//! boxes, fragments, and the export's own stylesheet.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::Source;

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
