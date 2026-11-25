//! Hostile text: the tool never panics, what it echoes is clean and bounded, and the work stays bounded by the size of the
//! text (`docs/import-html.md` section 5, item 6). The plan is a function of the text, so most of this runs without a
//! server; the last test sends nasty pages through the real one.

mod common;
mod harness;
use common::*;
use harness::*;
use html_import::html::clean;
use meldframe_http_html_import::{Args, describe, make};
use serde_json::{Value, json};

/// The codes a call may answer with for any text.
const CODES: [&str; 5] = [
    "E_IMPORT_HTML_TOO_LARGE",
    "E_IMPORT_HTML_TOO_MANY",
    "E_IMPORT_HTML_TOO_MANY_ELEMENTS",
    "E_IMPORT_HTML_EMPTY",
    "E_IMPORT_HTML_UNSUPPORTED",
];

/// Every string of a JSON value, keys included.
fn strings<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        Value::Object(m) => m.iter().for_each(|(k, x)| {
            out.push(k);
            strings(x, out);
        }),
        _ => {}
    }
}

/// Plan `html` and `css`: an error is one of the documented codes; an answer is bounded and holds no control or hidden
/// character. Returns the size of the answer, or `None` for an error.
fn check(html: &str, css: &str) -> Option<usize> {
    let a = match Args::parse(args_of(html, json!({"css": css, "name": "N"}))) {
        Ok(a) => a,
        Err(e) => {
            assert_eq!(code(&e), "E_IMPORT_HTML_TOO_LARGE", "{e}");
            return None;
        }
    };
    match make(&a, ACTOR) {
        Err(e) => {
            assert!(CODES.contains(&code(&e).as_str()), "{e}");
            None
        }
        Ok(p) => {
            let answer = describe(&p);
            let mut all = Vec::new();
            strings(&answer, &mut all);
            for s in all {
                assert_eq!(clean(s, usize::MAX), s, "a hidden character in {s:?}");
            }
            let size = answer.to_string().len();
            assert!(size < 700_000, "an answer of {size} bytes");
            Some(size)
        }
    }
}

const DOC: &str = "<!doctype html><html><head><title>T &amp; t</title><style>.a{padding:4px;display:flex;gap:3px}\
    #b>p{color:#f00}@media(x){.a{width:1px}}</style></head><body style='margin:0'>\
    <div class=a id=b data-name='Card'><p style='font-size:20px;font-weight:700'>Hello <b>big</b> world<br>again</p>\
    <img src='https://x.test/a.png' style='width:10px;height:10px'><button>Go</button>\
    <input value='v' placeholder=\"p\"><table><tr><td colspan=2>c</td><td>d</td></tr></table>\
    <span style='background:#eee;padding:2px'>chip</span></div></body></html>";

#[test]
fn every_prefix_of_a_document_is_planned_or_refused_with_a_code() {
    let mut planned = 0;
    for end in (0..=DOC.len()).filter(|i| DOC.is_char_boundary(*i)) {
        planned += usize::from(check(&DOC[..end], "").is_some());
    }
    assert!(planned > DOC.len() / 2, "{planned}");
}

#[test]
fn deep_nesting_a_wide_text_and_a_bomb_of_references_are_bounded() {
    let deep = format!(
        "{}x{}",
        "<div style='padding:1px'>".repeat(8_000),
        "</div>".repeat(8_000)
    );
    // The builder flattens what is deeper than 64; the plan collapses plain wrappers; either way an answer or a code.
    check(&deep, "");
    let refs = format!(
        "<p>{}</p>",
        "&amp;&#x41;&lt;&notanentity;&#99999999;".repeat(5_000)
    );
    assert!(check(&refs, "").is_some());
    let wide = format!("<p>{}</p>", "a".repeat(250_000));
    assert!(check(&wide, "").is_some());
    let many = format!(
        "<div class='{}'>x</div>",
        (0..20_000)
            .map(|i| format!("c{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let size = check(&many, "").unwrap_or(0);
    assert!(size < 60_000, "the report keeps 200 entries: {size}");
    let styles = "<div style='color:red'>x</div>".repeat(60);
    // 15,000 rules over the limit of 5,000: the reader cuts them and the report says so.
    assert!(check(&styles, &"a{color:#fff}".repeat(15_000)).is_some());
}

#[test]
fn control_bidi_and_hidden_characters_never_reach_the_answer() {
    let nasty = "a\u{202e}b\u{200b}c\u{7}d\u{feff}e\u{2066}f";
    let html = format!(
        "<div id='{nasty}' class='{nasty} x' data-name='{nasty}' style='color:#{nasty};line-height:{nasty}'>\
         <p title='{nasty}'>{nasty}</p><img src='javascript:alert(1){nasty}' alt='{nasty}' width=5 height=5>\
         <span class='{nasty}'>x</span></div>"
    );
    let css = format!(
        ".x{{background:url(javascript:{nasty});unknown-{nasty}:1}} @media {nasty} {{ p {{ color: red }} }}"
    );
    assert!(check(&html, &css).is_some());
}

#[test]
fn text_that_is_a_script_or_an_attack_on_the_style_reader_is_only_text() {
    for (html, css) in [
        ("<script>alert(1)</script><p>x</p>", ""),
        ("<p>x</p>", "</style><script>alert(1)</script>"),
        (
            "<style>p::before{content:'</style><p>y</p>'}</style><p>x</p>",
            "",
        ),
        (
            "<p style=\"background:url('http://127.0.0.1:1/x')\">x</p>",
            "@import url(http://127.0.0.1:1/y);",
        ),
        (
            "<link rel=stylesheet href='http://127.0.0.1:1/z.css'><p>x</p>",
            "",
        ),
        (
            "<p style='--a:var(--b);--b:var(--a);color:var(--a)'>x</p>",
            ":root{--c:var(--c)}",
        ),
        ("<p style='width:calc(calc(calc(1px)))'>x</p>", ""),
        (
            "<div data-node='a\\\"b' style='color:red'>x</div>",
            "[data-node=\"a\\\"b\"]{color:blue}",
        ),
    ] {
        assert!(check(html, css).is_some(), "{html} {css}");
    }
}

#[test]
fn the_limits_hold_at_their_edge() {
    assert_eq!(
        code(
            &make(
                &Args::parse(args_of(&"<i></i>".repeat(5_001), json!({}))).unwrap(),
                ACTOR
            )
            .unwrap_err()
        ),
        "E_IMPORT_HTML_TOO_MANY_ELEMENTS"
    );
    let at_limit = "a".repeat(262_144);
    assert_eq!(
        code(&make(&Args::parse(args_of(&at_limit, json!({}))).unwrap(), ACTOR).unwrap_err()),
        "E_IMPORT_HTML_EMPTY"
    );
    let over = "a".repeat(262_145);
    assert_eq!(
        code(&Args::parse(args_of(&over, json!({}))).unwrap_err()),
        "E_IMPORT_HTML_TOO_LARGE"
    );
}

#[tokio::test]
async fn nasty_pages_sent_to_the_server_get_an_answer_and_leave_the_file_valid() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let pages = [
        format!(
            "{}x{}",
            "<div style='padding:1px'>".repeat(8_000),
            "</div>".repeat(8_000)
        ),
        format!("<p>{}</p>", "&amp;&#x41;".repeat(20_000)),
        format!(
            "<div class='{}'>x</div>",
            (0..5_000)
                .map(|i| format!("c{i}"))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        "\u{0}<\u{0}div \u{202e}style=\"\u{0}\"><p>\u{200b}</p>".repeat(500),
    ];
    for (i, html) in pages.iter().enumerate() {
        let r = import(&c, &format!("k{i}"), html, json!({})).await;
        let code = if r.is_error {
            r.body["error"]["code"].as_str().unwrap().to_owned()
        } else {
            "ok".into()
        };
        // Or a refusal of the file's own checks (`E_TOO_LONG` for a text past 20,000 characters): it names its command.
        let refused = code.starts_with("E_") && r.body["error"]["index"].is_u64();
        assert!(
            code == "ok" || CODES.contains(&code.as_str()) || refused,
            "page {i}: {}",
            r.body
        );
        assert!(r.body.to_string().len() < 700_000);
    }
    let tree = call(&c, "get_tree", json!({"file": "f"})).await;
    assert!(!tree.is_error, "{}", tree.body);
}
