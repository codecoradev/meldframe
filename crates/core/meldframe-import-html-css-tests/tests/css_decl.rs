//! The declaration reader (the value of a `style` attribute and the body of a rule): names, `!important`,
//! comments, strings, what is refused and what is cut, with the bytes the caps count.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod css_common;
use css_common::*;
use html_import::css::{EntryKind, Limits};

#[test]
fn names_fold_to_lower_case_and_important_is_read_in_any_spacing() {
    let (d, r) = decls(
        "COLOR : Red ! IMPORTANT; margin:0!important;--Brand: #FFF ;--empty:;padding:1px /* c */ 2px;\
         background:url(a;b) , none;content:\"a;b}c\"; font-family: 'A\\';B'",
    );
    assert_eq!(
        d,
        [
            "color:Red!",
            "margin:0!",
            "--Brand:#FFF",
            "--empty:",
            "padding:1px 2px",
            "background:url(a;b) , none",
            "content:\"a;b}c\"",
            "font-family:'A\\';B'"
        ]
    );
    assert!(r.entries.is_empty());
    // Only the end is read for the flag, in a value that has the word elsewhere or in a string.
    assert_eq!(
        decls("a:important b;c:'!important'").0,
        ["a:important b", "c:'!important'"]
    );
}

#[test]
fn a_piece_that_is_not_a_declaration_is_listed_and_the_rest_stay() {
    let (d, r) = decls("color red; :x; 1a:b; a b:c; width:; height:1px; }; x:y");
    assert_eq!(d, ["height:1px", "x:y"]);
    assert_eq!(
        names(&r, "invalid_declaration"),
        ["color red", ":x", "1a:b", "a b:c"]
    );
    assert_eq!(names(&r, "empty_value"), ["width:"]);
    assert!(r.entries.iter().all(|e| e.kind == EntryKind::Refused));
}

#[test]
fn a_rule_inside_a_block_is_skipped_and_the_declarations_around_it_stay() {
    let (d, r) = decls("x:1; .b{y:2} z:3; &:hover{w:4; v:{u:1}} t:5");
    assert_eq!(d, ["x:1", "z:3", "t:5"]);
    assert_eq!(names(&r, "nested_rule_skipped"), [".b", "&:hover"]);
}

#[test]
fn strings_brackets_and_escapes_hide_the_characters_that_end_a_piece() {
    assert_eq!(
        decls(r#"a:"}{;";b:url(x;y}z);c:'\'';d:e\;f;g:[;]"#).0,
        [r#"a:"}{;""#, "b:url(x;y}z)", r"c:'\''", r"d:e\;f", "g:[;]"]
    );
    assert_eq!(decls("a:url(a(b)c;d);e:f").0, ["a:url(a(b)c;d)", "e:f"]);
    // A close brace with nothing open is nothing in an attribute, and the text before it is not a declaration.
    let (d, r) = decls("a:1;};b:2}c:3");
    assert_eq!(d, ["a:1", "c:3"]);
    assert_eq!(names(&r, "invalid_declaration"), ["b:2"]);
}

#[test]
fn a_string_ends_at_any_line_end_and_the_declaration_with_it_is_left_out() {
    for end in ["\n", "\r", "\u{c}"] {
        let (d, r) = decls(&format!("a:\"x{end};c:d"));
        assert_eq!(d, ["c:d"], "{end:?}");
        assert_eq!(count(&r, "invalid_declaration"), 1);
    }
}

#[test]
fn a_comment_is_a_space_and_a_nul_is_the_replacement_character() {
    assert_eq!(decls("a:1px/**/2px;b:/**/c/**/").0, ["a:1px 2px", "b:c"]);
    assert_eq!(decls("a:1px /* x */ /* y */ 2px").0, ["a:1px 2px"]);
    assert_eq!(
        decls("a:x\0y;b:\"\0\"").0,
        ["a:x\u{fffd}y", "b:\"\u{fffd}\""]
    );
}

#[test]
fn text_that_ends_inside_something_keeps_what_came_before_and_says_so() {
    for text in ["a:\"c", "a:url(c", "a:c/* x", "a:[c"] {
        let (d, r) = decls(text);
        assert_eq!(d.len(), 1, "{text}");
        assert_eq!(count(&r, "unterminated"), 1, "{text}");
    }
    assert_eq!(decls("a:1;b:2").1.entries.len(), 0);
    assert_eq!(count(&decls("a:1;b:2;").1, "unterminated"), 0);
}

#[test]
fn white_space_does_not_count_against_the_cap_and_a_run_of_it_is_one_byte() {
    let pad = " ".repeat(1_300);
    let (d, r) = decls(&format!("{pad}x:1{pad};{pad}y:2{pad}"));
    assert_eq!(d, ["x:1", "y:2"]);
    assert!(r.entries.is_empty());
    assert_eq!(decls(&format!("x:1{}2", " ".repeat(5_000))).0, ["x:1 2"]);
    assert_eq!(decls(&format!("x:1{}2", "/**/".repeat(5_000))).0, ["x:1 2"]);
    assert_eq!(
        decls(&format!("x:1{}2", "\n\t \r".repeat(2_000))).0,
        ["x:1 2"]
    );
    // What is text is counted: a value of exactly the limit is kept and one byte more is not.
    let (d, r) = decls(&format!("a:{}", "v".repeat(1_000)));
    assert_eq!((d.len(), r.entries.len()), (1, 0));
    let (d, r) = decls(&format!("a:{}", "v".repeat(1_001)));
    assert_eq!(d.len(), 0);
    assert_eq!(names(&r, "value_too_long"), ["a"]);
}

#[test]
fn the_piece_cap_is_the_value_limit_plus_256_for_the_name_and_a_megabyte_is_not_kept() {
    // A name of 256 bytes and a value of the limit fit; a longer name and a value of the limit do not.
    let name = "n".repeat(255);
    assert_eq!(decls(&format!("{name}:{}", "v".repeat(1_000))).0.len(), 1);
    let name = "n".repeat(300);
    assert_eq!(decls(&format!("{name}:{}", "v".repeat(1_000))).0.len(), 0);
    let big = format!("a:{};b:2", "x".repeat(1 << 20));
    let (d, r) = decls(&big);
    assert_eq!(d, ["b:2"]);
    assert_eq!(names(&r, "value_too_long"), ["a"]);
    assert!(
        r.entries
            .iter()
            .all(|e| e.message.len() <= 240 && e.name.len() <= 80)
    );
    assert!(r.work <= 6 * big.len(), "{}", r.work);
}

#[test]
fn declarations_past_the_limit_are_left_out_and_counted_once() {
    let l = Limits {
        max_decls: 2,
        ..Limits::default()
    };
    let (d, r) = decls_with("a:1;b:2;c:3;d:4", &l);
    assert_eq!(d, ["a:1", "b:2"]);
    assert_eq!(count(&r, "declaration_limit"), 1);
    assert_eq!(r.entries[0].kind, EntryKind::Cut);
}

#[test]
fn the_byte_of_an_entry_is_the_one_given_plus_the_byte_in_the_value() {
    let mut r = html_import::css::Report::default();
    let _ = html_import::css::read_declarations("a:1; bad; b:2", 100, &Limits::default(), &mut r);
    // The byte is where the piece starts, past the white space before it.
    assert_eq!(r.entries[0].at, 100 + 5);
}
