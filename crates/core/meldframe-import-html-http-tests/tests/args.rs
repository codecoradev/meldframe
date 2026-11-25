//! The arguments of `import_html`: each is checked whole before anything is parsed, and a value that is not what its
//! slot wants is refused with its reason, never cut or guessed (`docs/import-html.md` section 4.1).

mod common;
use common::*;
use serde_json::json;

#[test]
fn a_call_with_a_file_html_and_a_key_is_taken() {
    let a = parse(json!({})).unwrap();
    assert_eq!((a.file.as_str(), a.key.as_str()), ("f", "k1"));
    assert_eq!((a.dry_run, a.strict), (false, false));
    assert!(a.css.is_empty() && a.parent_id.is_none() && a.name.is_none() && a.width.is_none());
}

#[test]
fn every_argument_is_read() {
    let a = parse(
        json!({"css": "p{margin:0}", "parentId": "frame-1", "name": "Hero", "width": 800,
        "dryRun": true, "strict": true, "note": "pull in the hero"}),
    )
    .unwrap();
    assert_eq!(
        (a.css.as_str(), a.parent_id.as_deref(), a.name.as_deref()),
        ("p{margin:0}", Some("frame-1"), Some("Hero"))
    );
    assert_eq!((a.width, a.dry_run, a.strict), (Some(800.0), true, true));
    assert_eq!(a.note.unwrap().as_str(), "pull in the hero");
}

#[test]
fn a_call_without_a_key_or_with_two_spellings_is_refused() {
    let mut a = args_of("<div>x</div>", json!({}));
    a.remove("idempotencyKey");
    let e = meldframe_http_html_import::Args::parse(a).unwrap_err();
    assert_eq!(code(&e), "invalid");
    assert!(message(&e).contains("idempotencyKey"), "{e}");
    let e = parse(json!({"idempotency_key": "k2"})).unwrap_err();
    assert_eq!(code(&e), "invalid");
    assert!(message(&e).contains("not both"), "{e}");
}

#[test]
fn the_snake_case_key_is_taken_alone() {
    let mut a = args_of("<div>x</div>", json!({"idempotency_key": "k2"}));
    a.remove("idempotencyKey");
    assert_eq!(
        meldframe_http_html_import::Args::parse(a).unwrap().key,
        "k2"
    );
}

#[test]
fn the_key_is_at_most_87_characters_because_the_digest_is_added() {
    assert_eq!(meldframe_http_html_import::MAX_IMPORT_KEY, 87);
    assert!(parse(json!({"idempotencyKey": "k".repeat(87)})).is_ok());
    for key in [
        "k".repeat(88),
        String::new(),
        "a\nb".into(),
        "a\u{7}b".into(),
    ] {
        let e = parse(json!({ "idempotencyKey": key })).unwrap_err();
        assert_eq!(code(&e), "invalid", "{key:?}");
    }
    // Counted in characters, not bytes.
    assert!(parse(json!({"idempotencyKey": "\u{e9}".repeat(87)})).is_ok());
}

#[test]
fn a_bad_file_name_is_refused() {
    for file in ["", "a/b", "a\\b", "..", ".", "  ", "a\nb", "\u{200b}x"] {
        let e = parse(json!({ "file": file })).unwrap_err();
        assert_eq!(code(&e), "invalid", "{file:?}");
    }
    assert!(parse(json!({"file": "x".repeat(128)})).is_ok());
    assert_eq!(
        code(&parse(json!({"file": "x".repeat(129)})).unwrap_err()),
        "invalid"
    );
}

#[test]
fn an_unknown_argument_or_a_value_of_the_wrong_type_is_bad_request() {
    for extra in [
        json!({"ifSeq": 3}),
        json!({"atomic": false}),
        json!({"part": 1}),
        json!({"width": "1440"}),
        json!({"dryRun": "yes"}),
        json!({"dryRun": 1}),
        json!({"strict": "true"}),
        json!({"name": 7}),
        json!({"parentId": ["a"]}),
        json!({"css": 5}),
        json!({"note": false}),
        json!({"html": 5}),
    ] {
        let e = parse(extra.clone()).unwrap_err();
        assert_eq!(code(&e), "bad_request", "{extra}");
    }
    let mut a = args_of("", json!({}));
    a.remove("html");
    assert_eq!(
        code(&meldframe_http_html_import::Args::parse(a).unwrap_err()),
        "bad_request"
    );
}

#[test]
fn a_width_that_is_not_a_size_is_refused() {
    for w in [
        json!(0),
        json!(-1),
        json!(-0.5),
        json!(100_000.5),
        json!(1e9),
    ] {
        let e = parse(json!({ "width": w })).unwrap_err();
        assert_eq!(code(&e), "E_OUT_OF_RANGE", "{w}");
        assert!(message(&e).contains("100000"), "{e}");
    }
    for w in [json!(0.5), json!(1), json!(1440), json!(100_000)] {
        assert!(parse(json!({ "width": w })).is_ok(), "{w}");
    }
}

#[test]
fn a_name_that_is_not_a_name_is_refused() {
    let bad = [
        String::new(),
        "   ".into(),
        "x".repeat(257),
        "a\nb".into(),
        "a\tb".into(),
        "a\u{202e}b".into(),
        "a\u{200b}b".into(),
        "a\u{feff}b".into(),
    ];
    for name in bad {
        let e = parse(json!({ "name": name })).unwrap_err();
        assert_eq!(code(&e), "invalid", "{name:?}");
    }
    assert!(parse(json!({"name": "x".repeat(256)})).is_ok());
    // 256 characters of two bytes each.
    assert!(parse(json!({"name": "\u{e9}".repeat(256)})).is_ok());
}

#[test]
fn a_parent_id_is_an_id_or_it_is_refused() {
    for p in ["", &"p".repeat(65), "a\nb"] {
        let e = parse(json!({ "parentId": p })).unwrap_err();
        assert_eq!(code(&e), "invalid", "{p:?}");
    }
    assert!(parse(json!({"parentId": "p".repeat(64)})).is_ok());
}

#[test]
fn a_note_follows_the_rules_of_apply_commands() {
    for note in [
        "x".repeat(501),
        "a\nb\nc\nd\ne".into(),
        "a\u{200b}b".into(),
        "  ".into(),
        "a\u{7}b".into(),
    ] {
        let e = parse(json!({ "note": note })).unwrap_err();
        assert_eq!(code(&e), "invalid", "{note:?}");
    }
    assert!(parse(json!({"note": "x".repeat(500)})).is_ok());
    assert!(parse(json!({"note": ""})).unwrap().note.is_none());
}

#[test]
fn the_size_limit_counts_bytes_of_html_and_css_together() {
    const MAX: usize = 262_144;
    // 'e acute' is two bytes: fewer characters than the limit, more bytes.
    let html = "\u{e9}".repeat(MAX / 2 + 1);
    let e = meldframe_http_html_import::Args::parse(args_of(&html, json!({}))).unwrap_err();
    assert_eq!(code(&e), "E_IMPORT_HTML_TOO_LARGE");
    assert!(message(&e).contains(&(MAX + 2).to_string()), "{e}");
    let at = |html: usize, css: usize| {
        meldframe_http_html_import::Args::parse(args_of(
            &"a".repeat(html),
            json!({"css": "b".repeat(css)}),
        ))
    };
    assert!(at(MAX, 0).is_ok());
    assert!(at(MAX - 10, 10).is_ok());
    assert_eq!(
        code(&at(MAX - 10, 11).unwrap_err()),
        "E_IMPORT_HTML_TOO_LARGE"
    );
    assert_eq!(code(&at(1, MAX).unwrap_err()), "E_IMPORT_HTML_TOO_LARGE");
}
