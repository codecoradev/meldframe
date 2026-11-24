//! Hostile text: nesting and siblings past the limits, a huge attribute, every kind of unfinished construct, every
//! prefix of a document, control and invalid characters, markup hidden in a script, reference bombs. The builder must
//! not panic, must stay inside its bounds, and must say what it did.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::{Child, Limits, ParseError, parse};

#[test]
fn a_hundred_thousand_nested_elements_make_a_tree_of_the_depth_limit() {
    let n = 100_000;
    let html = format!("{}core{}", "<div>".repeat(n), "</div>".repeat(n));
    let p = parse(&html, &roomy()).unwrap();
    assert_eq!(p.tree.len(), 64);
    assert_eq!(count(&p, "depth_cut"), n - 64);
    // What was inside the cut elements is kept, in the deepest element that was made.
    let mut at = p.tree.root_id();
    let mut depth = 0;
    while let Some(Child::Element(e)) = p.tree.element(at).children.first() {
        at = *e;
        depth += 1;
    }
    assert_eq!(depth, 64);
    assert_eq!(
        p.tree.element(at).children,
        vec![Child::Text("core".into())]
    );
    // The default limits refuse it first, for its size.
    assert_eq!(
        refused(&html, &Limits::default()).code(),
        "E_IMPORT_HTML_TOO_LARGE"
    );
}

#[test]
fn ten_thousand_siblings_are_refused_by_the_element_limit_and_read_when_it_is_raised() {
    let html = "<i></i>".repeat(10_000);
    assert_eq!(
        refused(&html, &Limits::default()),
        ParseError::TooMany {
            max: 5_000,
            at: 5_000 * 7
        }
    );
    let p = parse(&html, &roomy()).unwrap();
    assert_eq!(p.tree.root().children.len(), 10_000);
}

#[test]
fn a_megabyte_attribute_is_refused_by_the_input_limit_and_left_out_when_that_is_raised() {
    let html = format!("<a href=\"{}\" id=x>", "h".repeat(1 << 20));
    assert_eq!(
        refused(&html, &Limits::default()).code(),
        "E_IMPORT_HTML_TOO_LARGE"
    );
    let p = parse(&html, &roomy()).unwrap();
    assert_eq!(dump(&p.tree), "a(id=x)");
    assert_eq!(count(&p, "attribute_too_long"), 1);
}

#[test]
fn a_tag_with_a_hundred_thousand_attributes_keeps_64_of_them() {
    let attrs: String = (0..100_000).map(|k| format!(" a{k}")).collect();
    let p = parse(&format!("<p{attrs}>"), &roomy()).unwrap();
    assert_eq!(p.tree.element(first_element(&p.tree)).attrs.len(), 64);
    assert_eq!(count(&p, "attribute_dropped"), 100_000 - 64);
}

#[test]
fn text_that_stops_in_the_middle_of_a_construct_ends_there_and_is_reported() {
    // (text, the code the report must carry). Each starts with a paragraph so that the tree is not empty.
    for (tail, code) in [
        ("<div", "unterminated_tag"),
        ("<div class", "unterminated_tag"),
        ("<div class=", "unterminated_tag"),
        ("<div class=\"x", "unterminated_tag"),
        ("<div class='x'", "unterminated_tag"),
        ("<div class=x /", "unterminated_tag"),
        ("</div", "unterminated_tag"),
        ("</div class=\"", "unterminated_tag"),
        ("<!-- x", "unterminated_comment"),
        ("<!--", "unterminated_comment"),
        ("<!-", "unterminated_comment"),
        ("<![CDATA[ x", "unterminated_comment"),
        ("<?xml", "unterminated_comment"),
        ("<!DOCTYPE html", "unterminated_comment"),
        ("<script>var a", "unterminated_raw_text"),
        ("<style>a {", "unterminated_raw_text"),
        ("<title>a", "unterminated_raw_text"),
        ("<textarea>a</textare", "unterminated_raw_text"),
    ] {
        let p = parse(&format!("<p>a{tail}"), &Limits::default())
            .unwrap_or_else(|e| panic!("{tail:?}: {e}"));
        assert!(count(&p, code) >= 1, "{tail:?}: {:?}", p.report.counts);
    }
    // A lone `<`, `</` or `<!` at the end is text or a comment, never a panic.
    for tail in ["<", "</", "<!", "<a", "&", "&#", "&#x"] {
        parse(&format!("<p>a{tail}"), &Limits::default()).unwrap();
    }
}

#[test]
fn the_unfinished_tail_does_not_take_what_came_before_it_away() {
    let p = run("<p>a<b>c</b><div class=\"x>y</div>");
    assert_eq!(dump(&p.tree), r#"p{"a" b{"c"}}"#);
}

#[test]
fn every_prefix_of_a_document_is_read_without_a_panic_and_inside_the_bounds() {
    let doc = "<!DOCTYPE html><html lang=\"en\"><head><title>A &amp; B</title><meta charset=utf-8>\
               <style>p > a { color: red } /* </p> */</style><script>if (a < b) { x = \"<div>\" }</script></head>\
               <body class='x y' onload=\"go()\"><!-- c --><ul><li>one<li>two &copy;<li><a href=\"/u?a=1&b=2\">é</a></ul>\
               <table><tr><td>a<td>b<tr><td>c</table><p>end<br/>x<![CDATA[ y ]]></body></html>";
    let l = Limits::default();
    for end in 0..=doc.len() {
        if !doc.is_char_boundary(end) {
            continue;
        }
        match parse(&doc[..end], &l) {
            Ok(p) => assert!(p.tree.len() <= l.max_elements),
            Err(ParseError::Empty) => {}
            Err(e) => panic!("prefix {end}: {e}"),
        }
    }
}

#[test]
fn nul_bytes_and_other_controls_are_made_safe_where_they_are_kept() {
    let p = run("<p\0x a\0b=\"c\0d\" e=f\0>t\0u</p>");
    let el = p.tree.element(first_element(&p.tree));
    assert_eq!(el.tag, "p\u{fffd}x");
    assert_eq!(el.attrs[0].name, "a\u{fffd}b");
    assert_eq!(el.attrs[0].value, "c\u{fffd}d");
    assert_eq!(el.attrs[1].value, "f\u{fffd}");
    assert_eq!(el.children, vec![Child::Text("t\u{fffd}u".into())]);
}

#[test]
fn bytes_that_are_not_utf8_reach_the_builder_as_replacement_characters_and_nothing_else() {
    let bytes = b"<p title=\"\xff\xfe\">\xc3(\xe2\x82\xf0\x28\x8c\xbc</p>";
    let html = String::from_utf8_lossy(bytes);
    let p = run(&html);
    let el = p.tree.element(first_element(&p.tree));
    assert_eq!(el.attrs[0].value, "\u{fffd}\u{fffd}");
    assert!(matches!(&el.children[0], Child::Text(t) if t.contains('\u{fffd}')));
}

#[test]
fn control_and_bidi_characters_do_not_reach_the_reader_of_a_report() {
    let evil = "\u{202e}\u{200b}\u{1b}\u{7}x";
    let p = run(&format!(
        "<p><b{evil} t{evil}={evil}>a</i{evil}><a {} x=1>",
        "q".repeat(70)
    ));
    assert!(!p.report.entries.is_empty());
    for e in &p.report.entries {
        for s in [e.name.as_str(), e.message.as_str(), e.code] {
            assert!(
                !s.chars().any(|c| c.is_control()
                    || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')),
                "{s:?}"
            );
        }
    }
}

#[test]
fn a_script_that_looks_like_markup_is_one_dropped_element_and_runs_nothing() {
    let p = run(
        "<script>document.write('<div id=a>'); /* </p> */ var s = '<!--'; <style>x</style></script \n>\
         <p>after</p><SCRIPT src=\"http://evil/x.js\"></SCRIPT><script/><p>z</p>",
    );
    // `<script/>` is an open script (a browser reads it so): it swallows the rest.
    assert_eq!(dump(&p.tree), r#"p{"after"}"#);
    assert_eq!(count(&p, "script"), 3);
    assert_eq!(count(&p, "unterminated_raw_text"), 1);
}

#[test]
fn a_style_ends_at_the_first_end_tag_even_inside_a_css_string() {
    let p = run("<style>a::before { content: \"</style><b>x</b>\" }</style><p>y</p>");
    assert_eq!(
        dump(&p.tree),
        r#"style{"a::before { content: \""} b{"x"} "\" }" p{"y"}"#
    );
}

#[test]
fn no_url_is_followed_and_nothing_is_echoed_as_markup() {
    let p = run(
        "<link rel=stylesheet href=//evil/x.css><img src=\"javascript:alert(1)\"><a href=\"data:text/html,<b>\">x</a>",
    );
    assert_eq!(p.tree.len(), 3);
    // The values are text, kept as written for the mapper to list; nothing in the report carries them.
    assert!(
        p.report
            .entries
            .iter()
            .all(|e| !e.message.contains("javascript"))
    );
}

#[test]
fn a_reference_bomb_is_no_bigger_than_the_text_that_made_it() {
    // `&amp;amp;amp;...` is read once, not again: the result is shorter than the input, not longer.
    let html = format!("<p>{}</p>", "&amp;amp;amp;amp;".repeat(10_000));
    let p = parse(&html, &roomy()).unwrap();
    let Child::Text(t) = &p.tree.element(first_element(&p.tree)).children[0] else {
        panic!()
    };
    assert_eq!(t.len(), "&amp;amp;amp;".len() * 10_000);
    for r in [
        "&lt;",
        "&#x1F600;",
        "&#0;",
        "&#65;",
        "&nbsp;",
        "&hellip;",
        "&mdash;",
    ] {
        let html = format!("<p>{}</p>", r.repeat(50_000));
        let p = parse(&html, &roomy()).unwrap();
        let Child::Text(t) = &p.tree.element(first_element(&p.tree)).children[0] else {
            panic!()
        };
        assert!(t.len() <= html.len(), "{r}: {} > {}", t.len(), html.len());
    }
}

#[test]
fn a_numeric_reference_with_a_run_of_digits_is_read_in_a_bounded_window() {
    let html = format!(
        "<p>&#{}</p><p>&#x{};</p><p>&{}</p>",
        "9".repeat(100_000),
        "f".repeat(100_000),
        "a".repeat(100_000)
    );
    let p = parse(&html, &roomy()).unwrap();
    assert_eq!(p.tree.len(), 3);
    // Linear: the window of a reference is a few bytes, so the work does not grow with the run.
    assert!(p.report.work <= 8 * html.len(), "{}", p.report.work);
}

#[test]
fn tag_soup_that_never_closes_anything_is_read_to_the_end() {
    for html in [
        "<<<<<<<<<<",
        "<a<b<c>text",
        "<a/b/c/d>x</a/b/c/d>",
        "<p <p <p>",
        "<p =x =y=z>",
        "<p ===>",
        "<p \"\"\">",
        "<!-- --!> <p>",
        "<!--> <p>a",
        "<!---> <p>b",
        "<p>a</ p>b</>c<//>d",
        "<p>&&&&&&&&;;;;###",
        "<table><td><table><td><table><td>x",
        "<select><option><p><div><option>",
    ] {
        let p = parse(html, &Limits::default());
        assert!(matches!(&p, Ok(_) | Err(ParseError::Empty)), "{html:?}");
    }
}

#[test]
fn nul_bytes_are_stored_as_three_bytes_each_so_a_text_is_at_most_three_times_the_input() {
    let n = 1_000;
    let nuls = "\0".repeat(n);
    // The limits count the bytes as written: a value of 1,000 NULs is within `max_value_bytes` and stored as 3,000.
    let p = parse(&format!("<a b=\"{nuls}\">{nuls}</a>"), &roomy()).unwrap();
    let Child::Element(a) = p.tree.root().children[0] else {
        panic!()
    };
    let el = p.tree.element(a);
    assert_eq!(el.attrs[0].value.len(), 3 * n);
    assert_eq!(el.children, vec![Child::Text("\u{fffd}".repeat(n))]);
    // A style keeps its NULs the same way, and no stored text is over three times the input.
    let p = parse(&format!("<style>{nuls}</style>"), &roomy()).unwrap();
    let styles: Vec<&str> = p.tree.style_texts().collect();
    assert_eq!(styles, vec!["\u{fffd}".repeat(n).as_str()]);
}
