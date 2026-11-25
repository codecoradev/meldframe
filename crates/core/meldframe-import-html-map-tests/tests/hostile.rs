//! Hostile text: the plan never panics, stays linear in the size of the text, and what it writes passes the checks of
//! every write (`docs/import-html.md` section 5, item 6).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::ParseError;
use html_import::map::{Options, plan};

const DOC: &str = "<!doctype html><html><head><title>T &amp; t</title><style>.a{padding:4px;display:flex;gap:3px}\
    #b>p{color:#f00}@media(x){.a{width:1px}}</style></head><body style='margin:0'>\
    <div class=a id=b data-name='Card'><p style='font-size:20px;font-weight:700'>Hello <b>big</b> world<br>again</p>\
    <img src='https://x.test/a.png' style='width:10px;height:10px'><button>Go</button>\
    <input value='v' placeholder=\"p\"><table><tr><td colspan=2>c</td><td>d</td></tr></table>\
    <span style='background:#eee;padding:2px'>chip</span></div></body></html>";

#[test]
fn every_prefix_of_a_document_is_planned_or_refused_and_what_is_written_is_valid() {
    let mut planned_any = 0;
    for end in (0..=DOC.len()).filter(|i| DOC.is_char_boundary(*i)) {
        match plan(&DOC[..end], "", &Options::default()) {
            Ok(p) => {
                planned_any += 1;
                check_all(&p).unwrap_or_else(|e| panic!("prefix {end}: {e}"));
            }
            Err(e) => assert_eq!(e, ParseError::Empty, "prefix {end}"),
        }
    }
    assert!(planned_any > DOC.len() / 2);
}

#[test]
fn deep_nesting_is_cut_by_the_builder_and_collapses() {
    let html = format!("{}x{}", "<div>".repeat(4_000), "</div>".repeat(4_000));
    let p = plan(&html, RESET, &Options::default()).unwrap();
    assert!(p.nodes.len() <= 70, "{}", p.nodes.len());
    check_all(&p).unwrap();
}

#[test]
fn too_many_elements_are_refused_by_the_builder_not_planned() {
    let html = "<p>x</p>".repeat(6_000);
    let e = plan(&html, "", &Options::default()).unwrap_err();
    assert_eq!(e.code(), "E_IMPORT_HTML_TOO_MANY_ELEMENTS");
}

#[test]
fn the_work_grows_with_the_text_and_no_faster() {
    let work = |n: usize| {
        let html = format!(
            "<div data-name=Box>{}</div>",
            "<p style='padding:2px;margin:1px'>a b <b>c</b></p>".repeat(n)
        );
        plan(&html, RESET, &Options::default()).unwrap().report.work
    };
    let (small, big) = (work(250), work(1_000));
    assert!(
        big < small * 5,
        "{small} then {big} for four times the text"
    );
}

#[test]
fn a_huge_attribute_a_long_name_and_hidden_characters_are_cut_and_cleaned() {
    let (long, huge) = ("n".repeat(900), "n".repeat(100_000));
    let html = format!(
        "<div data-name='A\u{202e}\u{200b}B{long}' class='{huge}'><p data-name='\u{7}\u{1b}x'>t\u{202e}t</p><p>b</p></div>"
    );
    let p = plan(&html, RESET, &Options::default()).unwrap();
    let list = nodes(&p);
    assert_eq!(list[0].name.chars().count(), 256);
    assert!(list[0].name.starts_with("AB"));
    assert_eq!(list[1].name, "x");
    check_all(&p).unwrap();
}

#[test]
fn nothing_echoed_in_the_report_carries_a_hidden_character() {
    let html =
        "<div style='zoom:\u{202e}\u{7}x'><img src='javascript:\u{202e}alert(1)'><p>a</p></div>";
    let p = plan(html, RESET, &Options::default()).unwrap();
    for e in &p.report.entries {
        assert!(
            !e.message
                .chars()
                .chain(e.name.chars())
                .any(|c| c.is_control() || c == '\u{202e}'),
            "{e:?}"
        );
    }
    assert!(
        p.assets
            .iter()
            .all(|a| !a.src.chars().any(|c| c.is_control() || c == '\u{202e}'))
    );
}

#[test]
fn entities_comments_and_script_in_a_style_do_not_reach_the_nodes() {
    let amp = "&amp;".repeat(20_000);
    let html = format!(
        "<div data-name=Box><p>{amp}</p><!-- <p>no</p> --><script><p>no</p></script><p>y</p></div>"
    );
    let p = plan(&html, RESET, &Options::default()).unwrap();
    let list = nodes(&p);
    assert_eq!(list.len(), 3);
    assert_eq!(list[1].text.chars().count(), 20_000);
}

#[test]
fn a_style_that_tries_to_end_itself_inside_a_string_is_text_of_the_style() {
    let html = "<style>a::before{content:'</style><div>x</div>'}</style><div data-name=Box><p>a</p><p>b</p></div>";
    let p = plan(html, RESET, &Options::default()).unwrap();
    check_all(&p).unwrap();
}

#[test]
fn values_that_the_model_would_refuse_are_never_written() {
    let html = "<div data-name=Box style='width:999999999px;height:-4px;padding:500000px;margin:5000000px;gap:99999999999px;\
        font-size:0;left:99999999999px;top:-99999999999px;position:absolute'>\
        <p style='font-size:99999999999px;font-weight:99999'>t</p><p style='margin-left:-99999999px;width:7px'>b</p></div>";
    let p = plan(html, RESET, &Options::default()).unwrap();
    check_all(&p).unwrap();
}
