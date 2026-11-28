//! `GET /v1/files/{file}/batches` through the real router (ADR-0021, slice 5): the order, the bounds of `limit`,
//! `before` and `after`, the counters, and who can read what.

mod world;
use serde_json::{Value, json};
use world::*;

async fn get(w: &World, token: Option<&str>, path: &str) -> (u16, Value) {
    let url = format!("http://{}{path}", w.h.addr);
    let mut req = reqwest::Client::new().get(url);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    let r = req.send().await.unwrap();
    (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
}

fn numbers(body: &Value) -> Vec<u64> {
    let rows = body["batches"].as_array().unwrap();
    rows.iter().map(|b| b["n"].as_u64().unwrap()).collect()
}

async fn write(w: &World, n: usize) {
    let c = connect(&w.h, AGENT).await;
    for i in 1..=n {
        let note = json!({"note": format!("batch {i}")});
        let args = apply(
            "f",
            &format!("k{i}"),
            vec![frame(&format!("n{i}"), "N")],
            note,
        );
        assert!(!call(&c, "apply_commands", args).await.is_error);
    }
}

#[tokio::test]
async fn a_page_is_newest_first_with_the_counters_and_the_whole_row() {
    let w = start(true, false).await;
    write(&w, 3).await;
    let (status, body) = get(&w, Some(PERSON), "/v1/files/f/batches").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(numbers(&body), [3, 2, 1]);
    assert_eq!(body["hasMore"], false);
    assert_eq!(body["unreviewedCount"], 3);
    assert_eq!(body["canReview"], false);
    let row = &body["batches"][2];
    assert_eq!(row["actor"], "agent:alpha");
    assert_eq!(row["keyPrefix"], "k1");
    assert_eq!(row["note"], "batch 1");
    assert_eq!(row["review"], "unreviewed");
    assert_eq!(
        (
            row["commands"].clone(),
            row["written"].clone(),
            row["atomic"].clone()
        ),
        (json!(1), json!(1), json!(true))
    );
    assert!(
        row.get("reviewedBy").is_none() && row["rev"] == 1 && row["atMs"].as_u64().unwrap() > 0
    );
}

#[tokio::test]
async fn before_after_and_limit_bound_the_page() {
    let w = start(true, false).await;
    write(&w, 5).await;
    let page = |q: &'static str| {
        let w = &w;
        async move { get(w, Some(PERSON), &format!("/v1/files/f/batches{q}")).await }
    };
    let (_, b) = page("?limit=2").await;
    assert_eq!(
        (numbers(&b), b["hasMore"].clone()),
        (vec![5, 4], json!(true))
    );
    let (_, b) = page("?before=4&limit=2").await;
    assert_eq!(
        (numbers(&b), b["hasMore"].clone()),
        (vec![3, 2], json!(true))
    );
    let (_, b) = page("?before=2").await;
    assert_eq!((numbers(&b), b["hasMore"].clone()), (vec![1], json!(false)));
    let (_, b) = page("?after=3").await;
    assert_eq!(numbers(&b), [5, 4]);
    let (_, b) = page("?after=1&before=5").await;
    assert_eq!(numbers(&b), [4, 3, 2]);
    // The count is the file's, not the page's.
    assert_eq!(b["unreviewedCount"], 5);
    let (_, b) = page("?limit=100").await;
    assert_eq!(numbers(&b).len(), 5);
    for bad in [
        "?limit=0",
        "?limit=101",
        "?limit=x",
        "?before=-1",
        "?after=1.5",
        "?before=",
    ] {
        let (status, b) = page(bad).await;
        assert_eq!(
            (status, b["error"]["code"].clone()),
            (400, json!("invalid")),
            "{bad}"
        );
    }
}

/// A malformed query string is the same JSON `invalid` as a bad number, not a plain-text 400 of the framework.
#[tokio::test]
async fn a_malformed_query_string_is_the_json_invalid_of_the_other_routes() {
    let w = start(true, false).await;
    write(&w, 1).await;
    for bad in [
        "?limit=%ZZ",
        "?limit=%FF",
        "?%",
        "?before=%E0%A4%A",
        "?limit=1&limit",
        "?a=%00&limit=%",
    ] {
        let (status, b) = get(&w, Some(PERSON), &format!("/v1/files/f/batches{bad}")).await;
        let ok = status == 200 && b["batches"].is_array();
        let json_invalid = status == 400 && b["error"]["code"] == "invalid";
        assert!(ok || json_invalid, "{bad}: {status} {b}");
        if status != 200 {
            assert_eq!(b["error"]["retryable"], false, "{bad}");
        }
    }
    // A bad value of a known name is always refused, in whatever spelling it comes.
    for bad in ["?limit=%ZZ", "?limit=%FF", "?before=%E0%A4%A", "?after=%"] {
        let (status, b) = get(&w, Some(PERSON), &format!("/v1/files/f/batches{bad}")).await;
        assert_eq!(
            (status, b["error"]["code"].clone()),
            (400, json!("invalid")),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn the_default_page_is_fifty() {
    let w = start(true, false).await;
    write(&w, 52).await;
    let (_, b) = get(&w, Some(PERSON), "/v1/files/f/batches").await;
    assert_eq!((numbers(&b).len(), b["hasMore"].clone()), (50, json!(true)));
    assert_eq!(b["batches"][0]["n"], 52);
}

#[tokio::test]
async fn another_workspace_and_an_unknown_file_see_an_empty_list() {
    let w = start(true, false).await;
    write(&w, 2).await;
    let empty = json!({"batches": [], "hasMore": false, "unreviewedCount": 0, "canReview": false});
    let (status, b) = get(&w, Some(OTHER), "/v1/files/f/batches").await;
    assert_eq!((status, b), (200, empty.clone()));
    let (status, b) = get(&w, Some(PERSON), "/v1/files/nothing/batches").await;
    assert_eq!((status, b), (200, empty));
}

#[tokio::test]
async fn a_call_needs_a_token_and_a_valid_file_name() {
    let w = start(true, false).await;
    let (status, b) = get(&w, None, "/v1/files/f/batches").await;
    assert_eq!(
        (status, b["error"]["code"].clone()),
        (401, json!("unauthenticated"))
    );
    let (status, _) = get(&w, Some("not-a-token-at-all-1"), "/v1/files/f/batches").await;
    assert_eq!(status, 401);
    let long = "x".repeat(129);
    let (status, b) = get(&w, Some(PERSON), &format!("/v1/files/{long}/batches")).await;
    assert_eq!(
        (status, b["error"]["code"].clone()),
        (400, json!("invalid"))
    );
}
