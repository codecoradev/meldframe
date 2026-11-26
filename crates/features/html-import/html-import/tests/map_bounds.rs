//! The bounds of an import and hostile text: the limits, the codes, nothing fetched and nothing run, and the
//! work staying in proportion to the text. A prefix of any HTML is read without a panic.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod map_common;
use html_import::map::{ImportError, MAX_COMMANDS, MAX_INPUT_BYTES, Options, import};
use map_common::*;
use meldframe_check::{Limits, check_node};
use meldframe_protocol::NodeKind;

fn list(n: usize) -> String {
    let items: String = (0..n)
        .map(|i| format!("<div style=\"height:1px;width:1px\" data-name=\"n{i}\"></div>"))
        .collect();
    format!("<div style=\"width:100px\">{items}</div>")
}

#[test]
fn exactly_one_hundred_nodes_are_written() {
    let i = run(&list(MAX_COMMANDS - 1));
    assert_eq!(i.commands.len(), MAX_COMMANDS);
}

#[test]
fn one_more_than_a_hundred_is_refused_and_says_how_many() {
    let e = refused(&list(MAX_COMMANDS));
    assert_eq!(
        e,
        ImportError::TooMany {
            nodes: MAX_COMMANDS + 1,
            max: MAX_COMMANDS
        }
    );
    assert_eq!(e.code(), "E_IMPORT_HTML_TOO_MANY");
    assert!(e.to_string().contains("101") && e.to_string().contains("one section at a time"));
}

#[test]
fn html_and_css_over_256_kib_together_are_refused_before_anything_is_read() {
    let html = format!("<div>{}</div>", "a".repeat(MAX_INPUT_BYTES - 100));
    let e = import(&html, &"b".repeat(200), &Options::default()).unwrap_err();
    assert!(matches!(e, ImportError::TooLarge { max, .. } if max == MAX_INPUT_BYTES));
    assert_eq!(e.code(), "E_IMPORT_HTML_TOO_LARGE");
    // The css counts: the same html alone is read.
    assert!(import(&html, "", &Options::default()).is_ok());
}

#[test]
fn more_than_five_thousand_elements_are_refused() {
    let html = "<i></i>".repeat(5_001);
    let e = refused(&html);
    assert_eq!(e.code(), "E_IMPORT_HTML_TOO_MANY_ELEMENTS");
}

#[test]
fn nothing_to_import_is_empty() {
    for h in [
        "",
        "just text",
        "<!-- c -->",
        "<script>x()</script>",
        "<div style=\"display:none\">x</div>",
    ] {
        assert_eq!(refused(h), ImportError::Empty, "{h:?}");
        assert_eq!(refused(h).code(), "E_IMPORT_HTML_EMPTY");
    }
}

#[test]
fn deep_nesting_is_bounded_and_wrappers_collapse() {
    // 3,000 bare wrappers: the tree builder cuts the depth (the content of an element it does not make goes into the
    // deepest it made), and the wrappers that are left make no node, so the text is the one node.
    let html = format!("{}x{}", "<div>".repeat(3_000), "</div>".repeat(3_000));
    let i = run(&html);
    assert!(
        i.report.counts.get("depth_cut").is_some_and(|n| *n > 0),
        "{:?}",
        i.report.counts
    );
    assert_eq!(
        i.commands.len(),
        2,
        "the page and the text: {:?}",
        names(&nodes(&i))
    );
}

#[test]
fn deep_nesting_that_says_something_is_cut_to_the_depth_of_the_tree() {
    let html = format!(
        "{}x{}",
        "<div style=\"padding:1px\">".repeat(100),
        "</div>".repeat(100)
    );
    let i = run(&html);
    // 64 frames, the deepest holding the text of the 36 that were cut.
    assert!(
        i.report.counts.get("depth_cut").is_some_and(|n| *n > 0),
        "{:?}",
        i.report.counts
    );
    assert_eq!(i.commands.len(), 65);
    assert!(i.commands.len() <= MAX_COMMANDS);
}

#[test]
fn a_huge_attribute_is_left_out_whole_and_the_node_is_named_by_its_role() {
    let big = "x".repeat(200_000);
    let html = format!(
        "<div style=\"width:30px\"><div data-name=\"{big}\" style=\"width:10px;height:5px\"></div></div>"
    );
    let i = run(&html);
    assert_eq!(i.report.counts.get("attribute_too_long"), Some(&1));
    assert_eq!(i.nodes[1].name, "Container", "{:?}", i.nodes);
}

#[test]
fn a_script_is_never_a_node_and_never_run_and_its_text_is_not_kept() {
    let i = run(
        "<div style=\"width:100px\"><script>document.write('<p>injected</p>')</script><p style=\"margin:0\">ok</p></div>",
    );
    let ns = nodes(&i);
    assert_eq!(ns.len(), 2);
    assert!(ns.iter().all(|n| !n.text.contains("injected")));
    assert_eq!(i.report.counts.get("script"), Some(&1));
}

#[test]
fn urls_are_listed_never_followed_and_a_data_payload_is_never_echoed() {
    let payload = "A".repeat(900);
    let html = format!(
        "<div style=\"width:300px\"><img id=a style=\"width:5px;height:5px\" src=\"javascript:alert(1)\"><img id=b style=\"width:5px;height:5px\" src=\"data:image/png;base64,{payload}\"><div id=c style=\"width:5px;height:5px;background:url(https://x.test/bg.png)\"></div></div>"
    );
    let i = run(&html);
    let srcs: Vec<&str> = i.assets.iter().map(|a| a.src.as_str()).collect();
    assert!(srcs.contains(&"javascript:alert(1)"), "{srcs:?}");
    assert!(srcs.contains(&"data:image/png;base64"), "{srcs:?}");
    assert!(srcs.iter().all(|s| s.chars().count() <= 200));
    assert!(
        srcs.iter().any(|s| s.contains("https://x.test/bg.png")),
        "{srcs:?}"
    );
    assert!(i.assets.iter().any(|a| a.usage == "background"));
    // Nothing in the result carries the payload.
    let dump = format!("{:?}{:?}", i.assets, i.report);
    assert!(!dump.contains(&payload[..100]));
    // A longer value is left out whole by the tree builder (1,000 bytes): the box is made, no address is listed.
    let long = "A".repeat(5_000);
    let j = run(&format!(
        "<div style=\"width:300px\"><img id=a style=\"width:5px;height:5px\" src=\"data:image/png;base64,{long}\"></div>"
    ));
    assert!(j.assets.is_empty());
    assert!(!format!("{:?}{:?}", j.assets, j.report).contains(&long[..100]));
    assert_eq!(j.report.counts.get("attribute_too_long"), Some(&1));
}

#[test]
fn an_entity_bomb_is_bounded() {
    let html = format!(
        "<div style=\"width:100px\"><p style=\"margin:0\">{}</p></div>",
        "&amp;".repeat(50_000)
    );
    let i = run(&html);
    assert_eq!(nodes(&i)[1].text.chars().count(), 50_000);
}

#[test]
fn hidden_and_bidi_characters_in_the_report_and_the_names_are_removed() {
    let i = run(
        "<div style=\"width:100px\"><div data-name=\"\u{202e}evil\" style=\"height:2px;transform:\u{202e}rotate(1deg)\"></div></div>",
    );
    let dump = format!("{:?}", i.report);
    assert!(!dump.contains('\u{202e}'));
    for n in &i.nodes {
        for l in &n.unsupported {
            assert!(
                meldframe_text::first_hidden(&l.value, false).is_none(),
                "{l:?}"
            );
        }
    }
    assert!(!format!("{:?}", i.nodes).contains('\u{202e}'));
}

#[test]
fn every_command_passes_the_checks_of_the_file() {
    let i = run(
        "<div style=\"display:flex;width:300px;gap:4px;padding:8px;background:#102030\"><p>a</p><div style=\"width:5px;height:5px;border-radius:50%;background:#fff\"></div></div>",
    );
    for n in nodes(&i) {
        check_node(&n, &Limits::default()).unwrap_or_else(|e| panic!("{n:?}: {e}"));
    }
}

#[test]
fn a_text_over_the_limit_of_the_file_is_still_one_node_the_gate_will_refuse_naming_the_source() {
    let html = format!(
        "<div style=\"width:100px\"><p>{}</p></div>",
        "x".repeat(20_001)
    );
    let i = run(&html);
    let n = nodes(&i)
        .into_iter()
        .find(|n| n.kind == NodeKind::Text)
        .unwrap();
    assert_eq!(
        n.text.chars().count(),
        20_001,
        "the text is file content: not cut"
    );
    assert!(check_node(&n, &Limits::default()).is_err());
    assert!(i.nodes[1].source.starts_with("p#"));
}

#[test]
fn a_very_long_text_is_measured_as_one_line_so_the_work_stays_small() {
    // The stub's wrap is quadratic in the length of a line: a text of 60,000 characters in a box of 100,000 px would
    // run for seconds. A text over 4,096 characters is measured as one line (the stored size is an estimate, the
    // layout ignores it), so the height is that of one line.
    let t = "a ".repeat(30_000);
    let ns = import_nodes(&format!(
        "<div style=\"width:300px\"><p style=\"margin:0;width:100000px\">{t}</p></div>"
    ));
    let n = ns.iter().find(|n| n.kind == NodeKind::Text).unwrap();
    assert_eq!(n.h, 20.0, "one line of the stub: ceil(16 * 1.2)");
}

#[test]
fn every_prefix_of_an_html_text_is_read_without_a_panic() {
    let doc = "<!doctype html><html><head><title>T</title><style>p{color:red}.a{width:10px}</style></head><body style=\"margin:0\"><div class=a style=\"display:flex;gap:4px\"><p style=\"margin:0\">Hello &amp; <b>world</b><br>x</p><img src=\"a.png\"><table><tr><td>1</td><td>2</td></tr></table></div></body></html>";
    for (i, _) in doc.char_indices() {
        let _ = import(&doc[..i], "", &Options::default());
    }
    assert!(import(doc, "", &Options::default()).is_ok());
}

#[test]
fn the_work_grows_with_the_text_and_no_faster() {
    let work =
        |n: usize| import(&list_text(n), "", &Options::default()).map_or(0, |i| i.report.work);
    let (a, b) = (work(8), work(16));
    assert!(a > 0 && b > 0);
    assert!(b <= a * 3, "{a} then {b}");
}

fn list_text(n: usize) -> String {
    let items: String = (0..n)
        .map(|i| format!("<p style=\"margin:0\">item {i}</p>"))
        .collect();
    format!("<div style=\"width:100px\">{items}</div>")
}
