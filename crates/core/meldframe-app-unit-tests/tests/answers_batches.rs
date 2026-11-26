//! The answers of the batch routes, checked in the app layer (audit finding T6, issue 323). The tables are those of the
//! TypeScript validators that moved here (`batches.test.ts` of `web-transport`), with the bounds of the server itself: a
//! body whose field is missing, mistyped or out of bound is the failed request `unavailable` and changes nothing, and a
//! field the app does not know is ignored. The shell passes the text of the answer through. The frames are in
//! `answers_hints.rs`.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::Action;
use serde_json::{Value, json};

mod batch_wire;
mod common;
use batch_wire::{bad_headers, failure, header, hint, list, page, review};

#[test]
fn a_list_answer_is_the_page_with_the_fields_the_app_knows() {
    let body = page(vec![
        header(json!({})),
        header(
            json!({"n": 2, "note": "why", "reviewedBy": "person:a", "reviewedAtMs": 9, "review": "needs_review"}),
        ),
    ]);
    let Some(Action::BatchesLoaded { page }) = list(200, &body.to_string()).0 else {
        panic!("a page");
    };
    assert_eq!(
        (
            page.batches.len(),
            page.has_more,
            page.unreviewed_count,
            page.can_review
        ),
        (2, true, 1, false)
    );
    assert_eq!(page.batches[0].actor, "agent:claude");
    assert_eq!(page.batches[1].note.as_deref(), Some("why"));
    assert_eq!(page.batches[1].reviewed_at_ms, Some(9));
}

#[test]
fn a_header_with_a_missing_mistyped_or_out_of_bound_field_fails_the_whole_page() {
    for over in bad_headers() {
        let body = page(vec![header(json!({})), header(over.clone())]).to_string();
        assert_eq!(list(200, &body), failure("unavailable"), "{over}");
    }
}

#[test]
fn the_edges_of_the_bounds_are_taken() {
    // The server's own ceilings, each one at its limit: a revision of 2^48, an actor of 128 bytes, a key prefix of 100
    // characters, a tool of 64, a note of 500 characters. A note of 500 two-byte characters is still 500 characters.
    let edge = json!({
        "n": 4294967295u64, "rev": 281474976710656u64, "actor": "a".repeat(128), "keyPrefix": "k".repeat(100), "tool": "t".repeat(64),
        "commands": 4294967295u64, "written": 0, "firstSeq": 9007199254740991u64, "note": "é".repeat(500),
        "reviewedBy": "p".repeat(128), "reviewedAtMs": 9007199254740991u64
    });
    assert!(matches!(
        list(200, &page(vec![header(edge)]).to_string()).0,
        Some(Action::BatchesLoaded { .. })
    ));
}

#[test]
fn a_page_that_is_wrong_in_shape_or_too_long_is_a_failed_request() {
    let bad: Vec<String> = vec![
        json!({"batches": [header(json!({}))], "hasMore": true, "unreviewedCount": 100001, "canReview": false}).to_string(),
        json!({"batches": [header(json!({}))], "hasMore": true, "unreviewedCount": null, "canReview": false}).to_string(),
        json!({"batches": null, "hasMore": true, "unreviewedCount": 1, "canReview": false}).to_string(),
        "null".into(), "\"x\"".into(), "[]".into(), "{}".into(), "<html>".into(), "".into(),
        json!({"batches": [header(json!({}))], "hasMore": 1, "unreviewedCount": 1, "canReview": false}).to_string(),
        json!({"batches": [header(json!({}))], "hasMore": true, "unreviewedCount": -1, "canReview": false}).to_string(),
        json!({"batches": [header(json!({}))], "hasMore": true, "unreviewedCount": 4294967296u64, "canReview": false}).to_string(),
        json!({"batches": [header(json!({}))], "hasMore": true, "unreviewedCount": 1}).to_string(),
        json!({"batches": 3, "hasMore": true, "unreviewedCount": 1, "canReview": false}).to_string(),
        json!({"batches": [null], "hasMore": true, "unreviewedCount": 1, "canReview": false}).to_string(),
        page((0..101).map(|_| header(json!({}))).collect()).to_string(),
    ];
    for body in bad {
        assert_eq!(list(200, &body), failure("unavailable"), "{body}");
    }
    assert!(matches!(
        list(
            200,
            &page((0..100).map(|_| header(json!({}))).collect()).to_string()
        )
        .0,
        Some(Action::BatchesLoaded { .. })
    ));
}

#[test]
fn a_review_answer_is_unwrapped_to_the_row_alone() {
    let row = header(json!({"review": "accepted", "reviewedBy": "p", "reviewedAtMs": 1}));
    let Some(Action::BatchReviewed { batch }) = review(
        200,
        &json!({"batch": row, "unreviewedCount": 0}).to_string(),
    )
    .0
    else {
        panic!("a row");
    };
    assert_eq!((batch.n, batch.reviewed_by.as_deref()), (3, Some("p")));
    for body in [
        json!({"batch": null, "unreviewedCount": 0}).to_string(),
        header(json!({})).to_string(),
        "[]".into(),
        "<html>".into(),
        json!({"batch": header(json!({"n": 0})), "unreviewedCount": 0}).to_string(),
    ] {
        assert_eq!(review(200, &body), failure("unavailable"), "{body}");
    }
}

#[test]
fn a_refusal_carries_the_code_of_the_error_capped_to_a_short_word() {
    let err = |code: Value| json!({"error": {"code": code, "message": "m"}}).to_string();
    assert_eq!(list(403, &err(json!("forbidden"))), failure("forbidden"));
    assert_eq!(review(404, &err(json!("not_found"))), failure("not_found"));
    assert_eq!(
        review(429, &err(json!("rate_limited"))),
        failure("rate_limited")
    );
    for code in [
        json!("x".repeat(65)),
        json!(""),
        json!("Has Space"),
        json!("a\nb"),
        json!("<script>"),
        json!("ünï"),
        json!(7),
        json!(null),
    ] {
        assert_eq!(
            list(500, &err(code.clone())),
            failure("unavailable"),
            "{code}"
        );
    }
    assert_eq!(
        list(502, "<html>bad gateway</html>"),
        failure("unavailable")
    );
    assert_eq!(
        list(500, &json!({"error": 5}).to_string()),
        failure("unavailable")
    );
    assert_eq!(list(204, ""), failure("unavailable"));
}

#[test]
fn a_field_the_app_does_not_know_is_ignored_in_a_page_a_row_a_review_and_a_frame() {
    // The server may add a field: what is read is checked, what is not read is let be.
    let mut row = header(json!({"note": "why"}));
    row["tags"] = json!(["new"]);
    let mut body = page(vec![row.clone()]);
    body["extra"] = json!(2);
    let Some(Action::BatchesLoaded { page }) = list(200, &body.to_string()).0 else {
        panic!("a page with a field more is a page");
    };
    assert_eq!(page.batches[0].note.as_deref(), Some("why"));
    let review_body = json!({"batch": row, "unreviewedCount": 0, "third": true}).to_string();
    assert!(matches!(
        review(200, &review_body).0,
        Some(Action::BatchReviewed { .. })
    ));
    let mut f = row;
    f["type"] = json!("batch");
    f["room"] = json!("file_dev");
    f["unreviewedCount"] = json!(2);
    f["more"] = json!(1);
    assert!(matches!(hint(&f.to_string()).0, Some(Action::Frame { .. })));
}

#[test]
fn a_review_answer_needs_a_count_the_server_could_send() {
    let row = header(json!({"review": "accepted"}));
    let body = |count: Value| json!({"batch": row, "unreviewedCount": count}).to_string();
    for count in [json!(0), json!(100000)] {
        assert!(
            matches!(
                review(200, &body(count.clone())).0,
                Some(Action::BatchReviewed { .. })
            ),
            "{count}"
        );
    }
    for count in [
        json!(100001),
        json!(-1),
        json!("0"),
        json!(null),
        json!(1.5),
    ] {
        assert_eq!(
            review(200, &body(count.clone())),
            failure("unavailable"),
            "{count}"
        );
    }
    // No count at all, and no row at all.
    assert_eq!(
        review(200, &json!({"batch": row}).to_string()),
        failure("unavailable")
    );
    assert_eq!(
        review(200, &json!({"unreviewedCount": 0}).to_string()),
        failure("unavailable")
    );
}

#[test]
fn a_note_that_is_null_or_absent_is_no_note_and_a_null_in_a_required_field_is_refused() {
    let nulls = json!({"note": null, "reviewedBy": null, "reviewedAtMs": null});
    let body = page(vec![header(nulls)]).to_string();
    let Some(Action::BatchesLoaded { page: loaded }) = list(200, &body).0 else {
        panic!("a null is no value for a field that may be left out");
    };
    let h = &loaded.batches[0];
    assert_eq!(
        (&h.note, &h.reviewed_by, h.reviewed_at_ms),
        (&None, &None, None)
    );
    let body = page(vec![header(json!({"review": null}))]).to_string();
    assert_eq!(list(200, &body), failure("unavailable"));
}
