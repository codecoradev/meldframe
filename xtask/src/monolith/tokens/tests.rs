use super::*;

fn texts(src: &str) -> Vec<String> {
    tokenize(src).into_iter().map(|t| t.text).collect()
}

fn texts_of(toks: Vec<Tok>) -> Vec<String> {
    toks.into_iter().map(|t| t.text).collect()
}

#[test]
fn comments_are_dropped_and_literals_are_one_token() {
    let got = texts("let a = \"x // y\"; // gone\n/* a /* nested */ b */ let b = 'c';");
    assert_eq!(
        got,
        [
            "let",
            "a",
            "=",
            "\"x // y\"",
            ";",
            "let",
            "b",
            "=",
            "'c'",
            ";"
        ]
    );
}

#[test]
fn lifetimes_are_not_char_literals() {
    assert_eq!(
        texts("fn f<'a>(x: &'a str, c: char) { c == '\\'' }"),
        [
            "fn", "f", "<", "'a", ">", "(", "x", ":", "&", "'a", "str", ",", "c", ":", "char", ")",
            "{", "c", "=", "=", "'\\''", "}"
        ]
    );
}

#[test]
fn raw_and_byte_strings_are_one_token() {
    let got = texts("let a = r#\"q\"x\"#; let b = b\"y\"; let c = b'z';");
    assert_eq!(
        got,
        [
            "let",
            "a",
            "=",
            "r#\"q\"x\"#",
            ";",
            "let",
            "b",
            "=",
            "b\"y\"",
            ";",
            "let",
            "c",
            "=",
            "b'z'",
            ";"
        ]
    );
}

#[test]
fn lines_count_from_one_through_comments_and_strings() {
    let toks = tokenize("a\n/* x\ny */ b\n\"s\ns\" c");
    let lines: Vec<usize> = toks.iter().map(|t| t.line).collect();
    assert_eq!(lines, [1, 3, 4, 5]);
}

#[test]
fn a_function_is_its_tokens_without_the_name() {
    let a = functions(&tokenize("pub fn one(x: u8) -> u8 { x + 1 }"));
    let b = functions(&tokenize("fn two(x: u8) -> u8 { x + 1 }"));
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name, "one");
    assert_eq!(a[0].line, 1);
    assert_eq!(a[0].tokens, b[0].tokens);
    assert!(!a[0].tokens.contains(&"one".to_string()));
}

#[test]
fn declarations_without_a_body_and_function_pointers_are_skipped() {
    let src = "trait T { fn a(&self) -> u8; fn b(&self) -> u8 { 1 } }\n\
               type P = fn(u8) -> u8;";
    let fns = functions(&tokenize(src));
    let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["b"]);
}

#[test]
fn methods_are_found_and_a_nested_function_belongs_to_its_outer_function() {
    let src = "impl X { fn m(&self) { fn inner() { 1 } inner(); } fn n(&self) {} }";
    let names: Vec<String> = functions(&tokenize(src))
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(names, ["m", "n"]);
}

#[test]
fn items_under_cfg_test_and_test_functions_are_removed() {
    let src = "fn keep() {}\n#[cfg(test)]\nmod tests { fn t() {} }\n\
               #[test]\nfn also() {}\n#[cfg(test)] use x::y;\nfn last() {}";
    let stripped = strip_tests(tokenize(src));
    let names: Vec<String> = functions(&stripped).into_iter().map(|f| f.name).collect();
    assert_eq!(names, ["keep", "last"]);
    assert!(!texts_of(stripped).contains(&"use".to_string()));
}

#[test]
fn find_compares_hex_literals_and_unicode_escapes_without_regard_to_case() {
    let src = "let a = 0x200b..=0x200F; let b = '\\u{feff}'; let c = \"\\u{00ad}\"; let d = b'a';";
    let toks = tokenize(src);
    assert_eq!(find(&toks, &tokenize("0x200B..=0x200f")), [1]);
    assert_eq!(find(&toks, &tokenize("'\\u{FEFF}'")), [1]);
    assert_eq!(find(&toks, &tokenize("\"\\u{00AD}\"")), [1]);
    // Characters and bytes outside an escape stay case sensitive.
    assert!(find(&toks, &tokenize("b'A'")).is_empty());
    assert!(find(&toks, &tokenize("0x200C")).is_empty());
}

#[test]
fn an_underscore_in_a_pattern_stands_for_any_one_identifier() {
    let toks = tokenize("a(hex.len() == 3);\nb(self.len() == 3);\nc(0.len() == 3);");
    let pat = tokenize("_.len() == 3");
    assert_eq!(find(&toks, &pat), [1, 2]);
    // It is one identifier: not a literal, and not nothing.
    assert!(find(&tokenize("(.len() == 3)"), &pat).is_empty());
    // Two of them need not be the same name.
    let toks = tokenize("|c| d.is_control()");
    assert_eq!(find(&toks, &tokenize("|_| _.is_control()")), [1]);
}

#[test]
fn a_file_that_ends_in_the_middle_of_an_attribute_does_not_panic() {
    for src in [
        "fn a() {}\n#[",
        "#",
        "#!",
        "#![",
        "#[cfg(test",
        "fn a() {}\n#[test]",
    ] {
        let _ = strip_tests(tokenize(src));
    }
    let kept = strip_tests(tokenize("fn a() {}\n#["));
    assert_eq!(functions(&kept).len(), 1);
}

#[test]
fn cfg_all_with_test_is_test_code_and_other_cfgs_are_not() {
    for (attr, stripped) in [
        ("#[cfg(all(test, feature = \"x\"))]", true),
        ("#[cfg(all(feature = \"x\", test))]", true),
        ("#[cfg(all(unix, test, windows))]", true),
        ("#[cfg(test)]", true),
        ("#[cfg(any(test, feature = \"x\"))]", false),
        ("#[cfg(not(test))]", false),
        ("#[cfg(all(unix, not(test)))]", false),
        ("#[cfg(feature = \"test\")]", false),
    ] {
        let src = format!("{attr}\nfn gone() {{}}\nfn kept() {{}}");
        let names: Vec<String> = functions(&strip_tests(tokenize(&src)))
            .into_iter()
            .map(|f| f.name)
            .collect();
        let expect: &[&str] = if stripped {
            &["kept"]
        } else {
            &["gone", "kept"]
        };
        assert_eq!(names, expect, "{attr}");
    }
}

#[test]
fn an_inner_cfg_test_attribute_empties_the_file() {
    assert!(strip_tests(tokenize("#![cfg(test)]\nfn a() {}")).is_empty());
}

#[test]
fn find_reports_the_line_of_each_match_and_ignores_whitespace() {
    let toks = tokenize("a\nx . b ( )\nz x.b()");
    let pat = tokenize("x.b()");
    assert_eq!(find(&toks, &pat), [2, 3]);
    assert!(find(&toks, &tokenize("q")).is_empty());
    assert!(find(&toks, &[]).is_empty());
}
