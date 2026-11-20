//! The `MELDFRAME_REVIEWERS` parser (ADR-0021, "Who may set it: deny by default"): exact match, trimmed, empty entries
//! ignored, a bad entry or more than 32 an error, unset means nobody.

use meldframe_http_activity::{MAX_REVIEWERS, Reviewers};

#[test]
fn entries_are_trimmed_and_empty_ones_ignored() {
    let r = Reviewers::parse(" user:bea ,, agent:dev,  ,\t").unwrap();
    assert_eq!(r.len(), 2);
    assert!(r.contains("user:bea") && r.contains("agent:dev"));
}

#[test]
fn the_match_is_exact_and_case_sensitive_on_the_whole_string() {
    let r = Reviewers::parse("agent:x,user:bea").unwrap();
    for no in [
        "Agent:x",
        "agent:",
        "agent:xx",
        "user:be",
        "x",
        "",
        "user:bea ",
        "person:x",
    ] {
        assert!(!r.contains(no), "{no:?} is not listed");
    }
}

#[test]
fn nothing_listed_means_nobody() {
    for spec in ["", "   ", ",,, ,"] {
        let r = Reviewers::parse(spec).unwrap();
        assert!(
            r.is_empty() && !r.contains("agent:dev") && !r.contains(""),
            "{spec:?}"
        );
    }
    assert_eq!(Reviewers::default(), Reviewers::parse("").unwrap());
}

#[test]
fn a_bad_entry_is_an_error_and_is_not_dropped() {
    let long = "a".repeat(129);
    for spec in [
        "user:bea,bad\u{200B}actor",
        "ok,x\u{7}y",
        "ok,\u{202E}evil",
        long.as_str(),
    ] {
        let e = Reviewers::parse(spec).unwrap_err();
        assert!(e.contains("not an actor"), "{e}");
        assert!(
            !e.contains("evil") && !e.contains("aaa"),
            "no entry is echoed: {e}"
        );
    }
    assert!(Reviewers::parse(&"a".repeat(128)).is_ok());
}

#[test]
fn more_than_the_most_entries_is_an_error() {
    let n = |k: usize| {
        (0..k)
            .map(|i| format!("agent:{i}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    assert_eq!(
        Reviewers::parse(&n(MAX_REVIEWERS)).unwrap().len(),
        MAX_REVIEWERS
    );
    let e = Reviewers::parse(&n(MAX_REVIEWERS + 1)).unwrap_err();
    assert!(e.contains("32"), "{e}");
    // Empty entries do not count toward the limit.
    let padded = format!("{},,,", n(MAX_REVIEWERS));
    assert!(Reviewers::parse(&padded).is_ok());
}
