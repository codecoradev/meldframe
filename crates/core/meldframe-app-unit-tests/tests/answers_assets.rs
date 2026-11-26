//! The answer of the asset list route, checked in the app layer (audit finding T6, issue 323). The table is that of
//! `assets.test.ts` of `web-transport`, which moved here: a page that is wrong in shape, bound or content is the failed
//! request `unavailable`, and a refusal carries the code of the error, capped to a short word.

// Under WASM these tests run through wasm-bindgen-test (`just wasm-test`), from the same source as the native run.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app::{Action, App};
use meldframe_app_model::Wire;
use meldframe_app_parts::answers::action_of;
use serde_json::{Value, json};

mod common;
use common::Out;

fn id(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

fn row(n: u8, over: Value) -> Value {
    let mut r = json!({"id": id(n), "bytes": 120 + n as u64, "mime": "image/png"});
    for (k, v) in over.as_object().cloned().unwrap_or_default() {
        if v == json!("<absent>") {
            r.as_object_mut().unwrap().remove(&k);
        } else {
            r[k] = v;
        }
    }
    r
}

fn ok(rows: Vec<Value>, next: Value) -> Value {
    json!({"assets": rows, "next": next})
}

fn answer(after: Option<String>, status: u16, body: &str) -> Out {
    Out(action_of(Wire::Assets {
        request: 7,
        after,
        status,
        body: body.into(),
    }))
}

fn failed(code: &str) -> Out {
    Out(Some(Action::AssetsRequestFailed {
        request: 7,
        code: code.into(),
    }))
}

#[test]
fn a_list_answer_is_assets_loaded_with_the_cursor_it_was_asked_with() {
    let body = ok(vec![row(1, json!({})), row(2, json!({}))], json!(id(2))).to_string();
    let Some(Action::AssetsLoaded {
        request,
        after,
        page,
    }) = answer(None, 200, &body).0
    else {
        panic!("a page");
    };
    assert_eq!(
        (request, after, page.assets.len(), page.next),
        (7, None, 2, Some(id(2)))
    );
    let Some(Action::AssetsLoaded { after, page, .. }) = answer(
        Some(id(2)),
        200,
        &ok(vec![row(3, json!({}))], json!(null)).to_string(),
    )
    .0
    else {
        panic!("a page");
    };
    assert_eq!((after, page.next), (Some(id(2)), None));
    assert!(
        matches!(
            answer(None, 200, &ok(vec![], json!(null)).to_string()).0,
            Some(Action::AssetsLoaded { .. })
        ),
        "an empty page is a page"
    );
}

#[test]
fn a_page_that_is_wrong_in_shape_bound_or_content_is_a_failed_request() {
    let upper = id(171).to_uppercase();
    let bad: Vec<Value> = vec![
        json!(null),
        json!("x"),
        json!([]),
        json!({}),
        json!({"assets": "x", "next": null}),
        ok(vec![json!(null)], json!(null)),
        ok(vec![json!("x")], json!(null)),
        ok(vec![row(1, json!({"id": "ABC"}))], json!(null)),
        ok(vec![row(1, json!({"id": upper}))], json!(null)),
        ok(
            vec![row(1, json!({"id": format!("{}0", id(1))}))],
            json!(null),
        ),
        ok(vec![row(1, json!({"id": "../../etc/passwd"}))], json!(null)),
        ok(vec![row(1, json!({"bytes": -1}))], json!(null)),
        ok(vec![row(1, json!({"bytes": 1.5}))], json!(null)),
        ok(vec![row(1, json!({"bytes": "5"}))], json!(null)),
        ok(
            vec![row(1, json!({"bytes": 9007199254740992u64}))],
            json!(null),
        ),
        ok(vec![row(1, json!({"mime": "image/svg+xml"}))], json!(null)),
        ok(vec![row(1, json!({"mime": "text/html"}))], json!(null)),
        ok(vec![row(1, json!({"mime": "<absent>"}))], json!(null)),
        ok(vec![row(1, json!({}))], json!("nope")),
        ok(vec![row(1, json!({}))], json!(5)),
        ok(vec![row(1, json!({}))], json!(id(171).to_uppercase())),
        ok((0..201).map(|i| row(i, json!({}))).collect(), json!(null)),
        // A cursor that is not the last id of the page, or that sits on a page with nothing.
        ok(vec![row(1, json!({})), row(2, json!({}))], json!(id(1))),
        ok(vec![], json!(id(1))),
        // A null where a value is needed is not a value.
        ok(vec![row(1, json!({"mime": null}))], json!(null)),
        ok(vec![row(1, json!({"id": null}))], json!(null)),
        ok(vec![row(1, json!({"bytes": null}))], json!(null)),
        json!({"assets": null, "next": null}),
    ];
    for body in bad {
        assert_eq!(
            answer(None, 200, &body.to_string()),
            failed("unavailable"),
            "{}",
            body.to_string().chars().take(80).collect::<String>()
        );
    }
    for text in ["", "<html>", "not json"] {
        assert_eq!(answer(None, 200, text), failed("unavailable"), "{text}");
    }
    assert!(matches!(
        answer(
            None,
            200,
            &ok((0..200).map(|i| row(i, json!({}))).collect(), json!(null)).to_string()
        )
        .0,
        Some(Action::AssetsLoaded { .. })
    ));
}

#[test]
fn a_refusal_carries_the_code_of_the_error_capped_to_a_short_word() {
    let err = |code: Value| json!({"error": {"code": code, "message": "m"}}).to_string();
    assert_eq!(
        answer(None, 401, &err(json!("unauthenticated"))),
        failed("unauthenticated")
    );
    assert_eq!(
        answer(
            None,
            503,
            &json!({"error": {"code": "unavailable", "retryable": true}}).to_string()
        ),
        failed("unavailable")
    );
    assert_eq!(
        answer(None, 500, &err(json!("Not A Code!"))),
        failed("unavailable")
    );
    assert_eq!(
        answer(None, 502, "<html>bad gateway</html>"),
        failed("unavailable")
    );
}

#[test]
fn the_app_takes_the_raw_page_while_the_picker_is_open_and_ignores_a_bad_one() {
    let mut a = App::new(
        "user:me",
        "c0",
        1,
        vec![serde_json::from_value(json!({"id": "r1", "type": "rect"})).unwrap()],
        0,
    )
    .unwrap();
    a.dispatch(Action::Connected { now_ms: 0 });
    a.dispatch(Action::Select {
        ids: vec!["r1".into()],
    });
    let fx = a.dispatch(Action::OpenAssets { id: "r1".into() });
    assert!(!fx.is_empty());
    let request = a.state().assets.request;
    a.dispatch(Action::Wire {
        wire: Wire::Assets {
            request,
            after: None,
            status: 200,
            body: ok(vec![row(1, json!({}))], json!(null)).to_string(),
        },
    });
    assert_eq!(a.state().assets.items.len(), 1);
    // A body that is not the server's ends the request as failed; what was read stays.
    a.dispatch(Action::Wire {
        wire: Wire::Assets {
            request,
            after: None,
            status: 200,
            body: "<html>".into(),
        },
    });
    assert_eq!(a.state().assets.items.len(), 1);
}

#[test]
fn a_field_the_app_does_not_know_is_ignored_in_a_page_and_in_a_row() {
    let mut page = ok(
        vec![row(1, json!({"name": "logo", "width": 4}))],
        json!(id(1)),
    );
    page["extra"] = json!(1);
    let Some(Action::AssetsLoaded { page, .. }) = answer(None, 200, &page.to_string()).0 else {
        panic!("a page the server may have added a field to is still a page");
    };
    assert_eq!(page.assets.len(), 1);
    assert_eq!(page.assets[0].id, id(1));
}

#[test]
fn the_picker_says_unavailable_for_a_page_that_is_not_the_servers_and_invalid_for_one_that_does_not_follow()
 {
    let mut a = App::new(
        "user:me",
        "c0",
        1,
        vec![serde_json::from_value(json!({"id": "r1", "type": "rect"})).unwrap()],
        0,
    )
    .unwrap();
    a.dispatch(Action::Connected { now_ms: 0 });
    a.dispatch(Action::Select {
        ids: vec!["r1".into()],
    });
    a.dispatch(Action::OpenAssets { id: "r1".into() });
    let load = |a: &mut App, after: Option<String>, body: String| {
        let request = a.state().assets.request;
        a.dispatch(Action::Wire {
            wire: Wire::Assets {
                request,
                after,
                status: 200,
                body,
            },
        });
    };
    load(
        &mut a,
        None,
        ok(vec![row(1, json!({}))], json!(id(1))).to_string(),
    );
    assert_eq!(a.state().assets.error, None);
    // The server's own words were `unavailable` before this check moved here, and still are: the picker says the same.
    a.dispatch(Action::LoadMoreAssets {});
    load(
        &mut a,
        Some(id(1)),
        ok(vec![row(2, json!({"bytes": -1}))], json!(null)).to_string(),
    );
    assert_eq!(a.state().assets.error.as_deref(), Some("unavailable"));
    // A page that is valid but does not follow what is held is the app's `invalid`.
    a.dispatch(Action::RetryAssets {});
    load(
        &mut a,
        Some(id(9)),
        ok(vec![row(3, json!({}))], json!(null)).to_string(),
    );
    assert_eq!(a.state().assets.error.as_deref(), Some("invalid"));
}
