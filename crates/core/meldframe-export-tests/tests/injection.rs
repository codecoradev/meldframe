//! E4.1: a string a user wrote can land in the HTML (text, attribute), in the stylesheet (selector, identifier, value)
//! and in the names of variables. Each context escapes it, so that nothing a user wrote ends a string, a rule, a
//! `url(...)` or the `<style>` element, and the CSS in the `<style>` has no `<` at all.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod support;
use export::{ExportOptions, Format};
use meldframe_layout::StubMeasurer;
use meldframe_protocol::Variables;
use meldframe_render::ModeChoice;
use serde_json::json;
use support::*;

/// Strings that end a string, a rule, a `url(...)`, an element or a line in some context.
const HOSTILE: &[&str] = &[
    "</style><script>alert(1)</script>",
    "\"><img src=x onerror=alert(1)>",
    "}*{display:none}",
    "x;background:url(//evil/x)",
    "back\\slash\\",
    "line\u{2028}sep\u{2029}end",
    "expression(alert(1))",
    "@import 'evil.css';",
    "url(javascript:alert(1))",
    "a'b\"c\nd",
];

/// The style element of a plain CSS export: between `<style>` and the first `</style>`.
fn style_body(html: &str) -> &str {
    let at = html.find("<style>").unwrap() + "<style>".len();
    &html[at..at + html[at..].find("</style>").unwrap()]
}

fn only_one_style_element(e: &export::Export) {
    assert_eq!(e.html.matches("<style>").count(), 1, "{}", e.html);
    assert_eq!(e.html.matches("</style>").count(), 1, "{}", e.html);
    assert!(
        !style_body(&e.html).contains('<'),
        "{}",
        style_body(&e.html)
    );
    assert!(!e.html.contains("<script"), "{}", e.html);
    assert!(!e.html.contains("<img"), "{}", e.html);
}

#[test]
fn text_is_escaped_in_the_html_and_never_reaches_the_stylesheet() {
    for h in HOSTILE {
        let e = css(frame(
            json!({}),
            vec![json!({"id": "t", "type": "text", "text": h})],
        ));
        only_one_style_element(&e);
        assert!(
            !e.html.contains(h) || !h.contains('<') && !h.contains('"'),
            "{h}"
        );
        assert!(!e.css.contains(h), "text is not in the CSS: {h}");
        // The paragraph holds the text as escaped entities, and nothing else opens.
        let p = e.html.split("<p ").nth(1).unwrap();
        let text = &p[p.find('>').unwrap() + 1..p.find("</p>").unwrap()];
        assert!(
            !text.contains('<') && !text.contains('"') && !text.contains('>'),
            "{text}"
        );
    }
}

#[test]
fn name_and_id_are_escaped_in_attributes_and_in_the_selector() {
    for h in HOSTILE {
        let list = frame(
            json!({"name": h}),
            vec![json!({"id": "r", "type": "rect", "name": h})],
        );
        let mut list = list;
        // The id of a child, as a file loaded from elsewhere might carry it.
        list[1]["id"] = json!(h);
        let e = css(list);
        only_one_style_element(&e);
        assert!(
            !e.html.contains(&format!("data-name=\"{h}\"")) || !h.contains('"'),
            "{h}"
        );
        // An attribute value has no raw quote, `<` or `>` in it.
        for attr in e.html.split("data-name=\"").skip(1) {
            let value = &attr[..attr.find('"').unwrap()];
            assert!(!value.contains('<') && !value.contains('>'), "{value}");
        }
        // In the stylesheet the id is inside a quoted string with every special character escaped as hex, so the
        // rule of the next node still starts where it should and no brace comes from the id.
        let sheet = style_body(&e.html);
        for line in sheet.lines().filter(|l| l.starts_with("[data-node=")) {
            let close = line.find("\"]").expect("the selector string ends");
            let id = &line[12..close];
            assert!(
                !id.contains('"') && !id.contains('{') && !id.contains('}') && !id.contains(';'),
                "{line}"
            );
            assert!(!id.contains('\n') && !id.contains('\u{2028}'), "{line}");
            assert!(
                !id.contains('(') && !id.contains('@') && !id.contains('\''),
                "{line}"
            );
            // A backslash in the id is only the start of a hex escape.
            let mut chars = id.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    assert!(
                        chars.peek().is_some_and(|d| d.is_ascii_hexdigit()),
                        "{line}"
                    );
                }
            }
        }
        assert_eq!(
            sheet
                .lines()
                .filter(|l| l.starts_with("[data-node="))
                .count(),
            2,
            "one rule per node, none added by an id: {sheet}"
        );
    }
}

#[test]
fn the_title_is_escaped() {
    for h in HOSTILE {
        let e = css(frame(json!({"name": h}), vec![]));
        only_one_style_element(&e);
        let title = e.html.split("<title>").nth(1).unwrap();
        let title = &title[..title.find("</title>").unwrap()];
        assert!(!title.contains('<') && !title.contains('"'), "{title}");
    }
}

#[test]
fn the_tailwind_form_escapes_names_and_text_too() {
    for h in HOSTILE {
        let e = tailwind(frame(
            json!({"name": h}),
            vec![json!({"id": "t", "type": "text", "text": h, "name": h})],
        ));
        assert!(!e.html.contains("<script"), "{}", e.html);
        assert!(!e.html.contains("<img"), "{}", e.html);
        assert!(!e.css.contains('<'), "{}", e.css);
    }
}

#[test]
fn a_colour_that_is_not_hex_is_not_copied_into_the_stylesheet() {
    for h in HOSTILE {
        let e = css(frame(
            json!({"shadows": [{"blur": 2, "color": h}]}),
            vec![
                json!({"id": "s", "type": "shader", "fill2": h}),
                json!({"id": "b", "type": "rect", "border": {"width": 2, "color": h}}),
                json!({"id": "d", "type": "rect", "shadows": [{"blur": 1, "color": h}]}),
            ],
        ));
        only_one_style_element(&e);
        for part in ["evil", "expression", "@import", "javascript", "onerror"] {
            assert!(!e.css.contains(part), "{part} came from `{h}`: {}", e.css);
        }
        assert!(!e.css.contains("</style"), "{}", e.css);
        let codes: Vec<&str> = e.report.unsupported.iter().map(|f| f.code).collect();
        assert!(
            codes.iter().filter(|c| **c == "invalid_colour").count() >= 3,
            "{codes:?}"
        );
    }
}

#[test]
fn a_hex_colour_is_still_copied_as_it_is() {
    let e = css(frame(
        json!({}),
        vec![
            json!({"id": "b", "type": "rect", "border": {"width": 2, "color": "#aabbcc80"},
            "shadows": [{"blur": 1, "color": "#abc"}]}),
        ],
    ));
    assert!(after(&e, "b").unwrap().contains("2px solid #aabbcc80"));
    assert!(rule(&e, "b").unwrap().contains("0px 0px 1px 0px #abc"));
    assert!(e.report.unsupported.is_empty());
}

#[test]
fn an_image_that_is_not_a_64_hex_asset_id_is_left_out_with_a_report() {
    for h in HOSTILE {
        let e = css(frame(
            json!({}),
            vec![json!({"id": "i", "type": "rect", "image": {"assetId": h}})],
        ));
        only_one_style_element(&e);
        assert!(!e.css.contains("background-image"), "{}", e.css);
        assert!(e.report.assets.is_empty());
        assert!(
            e.report
                .unsupported
                .iter()
                .any(|f| f.code == "invalid_asset")
        );
    }
    let good = "ab".repeat(32);
    let e = css(frame(
        json!({}),
        vec![json!({"id": "i", "type": "rect", "image": {"assetId": good}})],
    ));
    assert!(e.css.contains(&format!("url(assets/{good})")), "{}", e.css);
    assert_eq!(e.report.assets, [good]);
}

#[test]
fn the_font_and_the_asset_prefix_of_the_options_are_escaped() {
    for h in HOSTILE {
        let list = nodes(frame(
            json!({}),
            vec![json!({"id": "i", "type": "rect", "image": {"assetId": "ab".repeat(32)}})],
        ));
        let mut o = ExportOptions::new(Format::Css, &StubMeasurer);
        o.font_href = h.to_string();
        o.asset_base = h.to_string();
        o.css_href = h.to_string();
        let e = export::export_frame(&list, &Variables::default(), "f", &o).unwrap();
        only_one_style_element(&e);
        let sheet = style_body(&e.html);
        assert!(!sheet.contains("}*{"), "{sheet}");
        assert!(!sheet.contains(";background:url("), "{sheet}");
        assert!(!sheet.contains("'b\"c"), "{sheet}");
        let t = ExportOptions::new(Format::Tailwind, &StubMeasurer);
        let mut t = t;
        t.css_href = h.to_string();
        let e = export::export_frame(&list, &Variables::default(), "f", &t).unwrap();
        let link = e.html.split("<link ").nth(1).unwrap();
        let link = &link[..link.find('>').unwrap()];
        assert!(
            !link.contains('<') && link.matches('"').count() == 4,
            "{link}"
        );
    }
}

/// Lowercase letters, digits and hyphens only.
fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn hostile_file(h: &str) -> (Vec<meldframe_protocol::Node>, Variables) {
    let list = nodes(frame(
        json!({"fill": {"var": "v1"}}),
        vec![json!({"id": "r", "type": "rect", "radius": {"var": "v2"}})],
    ));
    let v = vars(json!({
        "collections": [{"id": "c1", "name": h,
            "modes": [{"id": "m1", "name": "Light"}, {"id": "m2", "name": h}]}],
        "variables": [
            {"id": "v1", "collection": "c1", "name": h, "type": "color", "values": {"m1": "#112233", "m2": "#445566"}},
            {"id": "v2", "collection": "c1", "name": format!("{h}2"), "type": "number", "values": {"m1": 4, "m2": 8}}
        ]
    }));
    (list, v)
}

#[test]
fn collection_variable_and_mode_names_become_plain_identifiers() {
    for h in HOSTILE {
        let (list, v) = hostile_file(h);
        for format in [Format::Css, Format::Tailwind] {
            let e = run(&list, &v, format, ModeChoice::of([("c1", "m2")]));
            // The names are slugs (lowercase letters, digits and hyphens): no context can be left through them.
            for line in e.css.lines().filter(|l| l.starts_with(":root")) {
                let head = &line[..line.find('{').unwrap()];
                // `:root`, or `:root[data-mode-<slug>="<slug>"]`.
                let rest = head.strip_prefix(":root").unwrap();
                if !rest.is_empty() {
                    let inner = rest
                        .strip_prefix("[data-mode-")
                        .and_then(|r| r.strip_suffix("\"]"))
                        .expect(head);
                    let (col, mode) = inner.split_once("=\"").expect(head);
                    assert!(is_slug(col) && is_slug(mode), "{head}");
                }
                for decl in line[line.find('{').unwrap() + 1..line.rfind('}').unwrap()].split(';') {
                    let (name, _) = decl.split_once(':').unwrap();
                    assert!(name.starts_with("--") && is_slug(&name[2..]), "{name}");
                }
            }
            if format == Format::Css {
                only_one_style_element(&e);
            }
            // The attribute of `<html>` is a slug too.
            let root = e.html.lines().find(|l| l.starts_with("<html")).unwrap();
            let attrs = &root["<html lang=\"en\"".len()..root.len() - 1];
            for attr in attrs.split_whitespace() {
                let (k, v) = attr.split_once('=').unwrap();
                assert!(is_slug(k), "{attr}");
                assert!(is_slug(v.trim_matches('"')), "{attr}");
            }
        }
    }
}

#[test]
fn a_node_id_in_the_selector_is_hex_escaped_and_a_plain_id_is_as_it_was() {
    let e = css(frame(
        json!({}),
        vec![json!({"id": "a:b.c", "type": "rect"})],
    ));
    assert!(
        e.css.contains("[data-node=\"a\\3a b\\2e c\"]{"),
        "{}",
        e.css
    );
    assert!(e.html.contains("data-node=\"a:b.c\""), "{}", e.html);
    let e = css(frame(
        json!({}),
        vec![json!({"id": "card-1_x", "type": "rect"})],
    ));
    assert!(rule(&e, "card-1_x").is_some());
}

#[test]
fn a_value_is_json_not_code() {
    // A last look at the whole answer for the worst of the strings at once.
    let h = HOSTILE.join("");
    let mut list = frame(
        json!({"name": h, "shadows": [{"color": h}]}),
        vec![
            json!({"id": "t", "type": "text", "text": h, "name": h}),
            json!({"id": "s", "type": "shader", "fill2": h, "name": h}),
        ],
    );
    list[2]["id"] = json!(h);
    let e = css(list);
    only_one_style_element(&e);
    assert!(!e.css.contains("</style"));
}
