//! The bounds of the tree builder, each at its limit and one past it, and the bounds of its report: what is refused,
//! what is cut or left out and counted, and what the error says (which bound, how much, where).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod common;
use common::*;
use html_import::html::{
    EntryKind, Limits, MAX_ENTRIES, MAX_NAME_BYTES, MAX_REPORT_BYTES, ParseError, parse,
};

#[test]
fn the_default_limits_are_the_ones_of_adr_0024() {
    let l = Limits::default();
    assert_eq!(l.max_input_bytes, 256 * 1024);
    assert_eq!(l.max_elements, 5_000);
    assert_eq!(l.max_depth, 64);
    assert_eq!(l.max_value_bytes, 1_000);
    assert_eq!(l.max_attributes, 64);
    assert_eq!(l.max_text_bytes, 64 * 1024);
}

#[test]
fn text_of_exactly_the_input_limit_is_read_and_one_byte_more_is_refused_before_anything_is_read() {
    let l = Limits {
        max_input_bytes: 100,
        ..Limits::default()
    };
    let ok = format!("<p>{}", "x".repeat(97));
    assert_eq!(ok.len(), 100);
    assert!(parse(&ok, &l).is_ok());
    let big = format!("<p>{}", "x".repeat(98));
    let e = refused(&big, &l);
    assert_eq!(
        e,
        ParseError::TooLarge {
            bytes: 101,
            max: 100
        }
    );
    assert_eq!(e.code(), "E_IMPORT_HTML_TOO_LARGE");
    assert!(
        e.to_string().contains("101") && e.to_string().contains("100"),
        "{e}"
    );
    // Refused first: text that would be refused for another reason says too large.
    let e = refused(&"</".repeat(51), &l);
    assert_eq!(e.code(), "E_IMPORT_HTML_TOO_LARGE");
}

#[test]
fn the_input_limit_counts_bytes_not_characters() {
    let l = Limits {
        max_input_bytes: 13,
        ..Limits::default()
    };
    assert!(parse("<p>ééé</p>", &l).is_ok(), "13 bytes");
    assert_eq!(
        refused("<p>éééé</p>", &l),
        ParseError::TooLarge { bytes: 15, max: 13 }
    );
}

#[test]
fn text_with_no_element_in_it_is_empty() {
    for html in [
        "",
        "   \n",
        "just text",
        "<!-- c -->",
        "<!DOCTYPE html>",
        "<script>var a = '<div>';</script>",
        "</p></div>",
    ] {
        let e = refused(html, &Limits::default());
        assert_eq!(e, ParseError::Empty, "{html:?}");
        assert_eq!(e.code(), "E_IMPORT_HTML_EMPTY");
    }
    assert!(parse("<style></style>", &Limits::default()).is_ok());
}

#[test]
fn the_element_limit_is_met_at_its_number_and_refused_one_past_it_at_the_tag_that_passes_it() {
    let l = Limits {
        max_elements: 5,
        ..Limits::default()
    };
    let ok = "<br>".repeat(5);
    assert_eq!(parse(&ok, &l).unwrap().tree.len(), 5);
    let e = refused(&"<br>".repeat(6), &l);
    assert_eq!(e, ParseError::TooMany { max: 5, at: 20 });
    assert_eq!(e.code(), "E_IMPORT_HTML_TOO_MANY");
    assert!(
        e.to_string().contains('5') && e.to_string().contains("20"),
        "{e}"
    );
    // A script is dropped and an element past the depth limit is flattened: neither is counted.
    let mixed = "<p><script>x</script></p>".repeat(2) + "<i><s><u>";
    assert!(parse(&mixed, &l).is_ok(), "5 elements and 2 scripts");
    assert!(parse(&format!("{ok}<i>"), &l).is_err());
}

#[test]
fn an_element_past_the_depth_limit_is_flattened_into_the_deepest_one_and_counted() {
    let l = Limits {
        max_depth: 3,
        ..Limits::default()
    };
    let p = parse("<a><b><c>x</c></b></a>", &l).unwrap();
    assert_eq!(dump(&p.tree), r#"a{b{c{"x"}}}"#);
    assert_eq!(count(&p, "depth_cut"), 0);
    let p = parse("<a><b><c><d>x<e>y</e></d>z</c></b></a>w", &l).unwrap();
    assert_eq!(dump(&p.tree), r#"a{b{c{"xyz"}}} "w""#);
    assert_eq!(count(&p, "depth_cut"), 2);
    assert_eq!(p.tree.len(), 3);
    let cut = p
        .report
        .entries
        .iter()
        .find(|e| e.code == "depth_cut")
        .unwrap();
    assert_eq!((cut.kind, cut.at), (EntryKind::Recovered, 9));
    assert!(cut.message.contains('3'), "{}", cut.message);
}

#[test]
fn an_element_with_more_attributes_than_the_limit_keeps_the_first_ones() {
    let l = Limits {
        max_attributes: 3,
        ..Limits::default()
    };
    assert_eq!(
        dump(&parse("<a a=1 b=2 c=3>", &l).unwrap().tree),
        "a(a=1 b=2 c=3)"
    );
    let p = parse("<a a=1 b=2 c=3 d=4 e=5>", &l).unwrap();
    assert_eq!(dump(&p.tree), "a(a=1 b=2 c=3)");
    assert_eq!(count(&p, "attribute_dropped"), 2);
    // A repeated name and an event attribute are not counted against the limit.
    let p = parse("<a a=1 a=2 onclick=x b=2 c=3>", &l).unwrap();
    assert_eq!(dump(&p.tree), "a(a=1 b=2 c=3)");
}

#[test]
fn an_attribute_value_of_the_limit_is_kept_and_one_byte_more_is_left_out_with_its_name() {
    let ok = "x".repeat(1_000);
    let p = run(&format!("<a title=\"{ok}\" id=i>"));
    assert_eq!(
        p.tree.element(first_element(&p.tree)).attr("title"),
        Some(ok.as_str())
    );
    let p = run(&format!("<a title=\"{}\" id=i>", "x".repeat(1_001)));
    assert_eq!(dump(&p.tree), "a(id=i)");
    assert_eq!(count(&p, "attribute_too_long"), 1);
    let e = p
        .report
        .entries
        .iter()
        .find(|e| e.code == "attribute_too_long")
        .unwrap();
    assert_eq!(e.name, "title");
    assert!(
        e.message.contains("1001") && e.message.contains("1000"),
        "{}",
        e.message
    );
}

#[test]
fn a_text_of_the_limit_is_kept_and_a_longer_one_is_cut_on_a_character_boundary() {
    let l = Limits {
        max_text_bytes: 10,
        ..Limits::default()
    };
    assert_eq!(
        dump(&parse("<p>0123456789</p>", &l).unwrap().tree),
        r#"p{"0123456789"}"#
    );
    let p = parse("<p>0123456789X</p>", &l).unwrap();
    assert_eq!(dump(&p.tree), r#"p{"0123456789"}"#);
    assert_eq!(count(&p, "text_cut"), 1);
    let l9 = Limits {
        max_text_bytes: 9,
        ..Limits::default()
    };
    assert_eq!(
        dump(&parse("<p>éééééé</p>", &l9).unwrap().tree),
        r#"p{"éééé"}"#
    );
    // The pieces around a comment are one text and one bound, counted once.
    let p = parse("<p>01234<!---->56789ab<!---->cd</p>", &l).unwrap();
    assert_eq!(dump(&p.tree), r#"p{"0123456789"}"#);
    assert_eq!(count(&p, "text_cut"), 1);
    // A style sheet is bounded by the input and not by the bound of a text.
    let p = parse("<style>0123456789ABCDEF</style>", &l).unwrap();
    assert_eq!(dump(&p.tree), r#"style{"0123456789ABCDEF"}"#);
}

#[test]
fn a_name_is_cut_to_64_bytes() {
    let long = "a".repeat(500);
    let p = run(&format!("<{long} {long}=1>"));
    let el = p.tree.element(first_element(&p.tree));
    assert_eq!(el.tag.len(), MAX_NAME_BYTES);
    assert_eq!(el.attrs[0].name.len(), MAX_NAME_BYTES);
}

#[test]
fn the_report_lists_at_most_200_entries_and_32_kib_and_counts_the_rest() {
    let p = run(&format!("<p></p>{}", "</x>".repeat(10_000)));
    assert_eq!(count(&p, "stray_end_tag"), 10_000);
    assert_eq!(p.report.entries.len(), MAX_ENTRIES);
    assert_eq!(p.report.omitted, 10_000 - MAX_ENTRIES);
    // Names that are as long as the text allows do not make the entries long.
    let html: String = (0..100)
        .map(|k| format!("</{}{k}>", "n".repeat(2_000)))
        .collect();
    let p = run(&format!("<p>{html}"));
    let bytes: usize = p
        .report
        .entries
        .iter()
        .map(|e| e.name.len() + e.message.len() + e.code.len())
        .sum();
    assert!(bytes <= MAX_REPORT_BYTES, "{bytes} bytes");
    assert!(
        p.report
            .entries
            .iter()
            .all(|e| e.name.chars().count() <= 80)
    );
    assert!(
        p.report
            .entries
            .iter()
            .all(|e| e.message.chars().count() <= 240)
    );
}

#[test]
fn a_recovery_entry_says_where_in_the_text_it_was() {
    let p = run("<p>a<div class=\"x");
    let e = p
        .report
        .entries
        .iter()
        .find(|e| e.code == "unterminated_tag")
        .expect("an entry for the open tag");
    assert_eq!((e.kind, e.at), (EntryKind::Recovered, 4));
}
