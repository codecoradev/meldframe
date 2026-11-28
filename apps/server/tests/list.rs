//! `GET /v1/assets`: the assets of the workspace of the token, a page at a time. The router is the real one; the blob
//! store is in memory. It is a read like `GET /v1/assets/{id}`: the token and the call slots, no content in the answer,
//! no entry in the call log (the REST asset routes make none), and never another workspace's assets.

mod http_common;
use assets::{BlobStore, MemoryBlobs, asset_id};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_common::*;
use meldframe_http::shared;
use std::sync::Arc;
use tower::ServiceExt;

fn app_with(blobs: Arc<dyn BlobStore>) -> (Router, Arc<Fake>) {
    let fake = Fake::ok();
    let router = meldframe_http::assets::routes(shared(tokens(), fake.clone(), config()), blobs);
    (router, fake)
}

fn png(n: usize) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    v.extend_from_slice(format!("image {n}").as_bytes());
    v
}

fn list(token: Option<&str>, query: &str) -> Request<Body> {
    let mut r = Request::get(format!("/v1/assets{query}"));
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    r.body(Body::empty()).unwrap()
}

fn ids(body: &serde_json::Value) -> Vec<String> {
    body["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["id"].as_str().unwrap().to_owned())
        .collect()
}

fn filled(n: usize) -> (Router, Arc<Fake>, Vec<String>) {
    let blobs = Arc::new(MemoryBlobs::default());
    let mut want: Vec<String> = (0..n)
        .map(|i| blobs.put("ws_1", &png(i)).unwrap().id)
        .collect();
    want.sort();
    let (app, fake) = app_with(blobs);
    (app, fake, want)
}

#[tokio::test]
async fn the_list_names_each_asset_by_id_size_and_type_and_carries_no_content() {
    let (app, fake, want) = filled(3);
    let (status, body) = json(app.oneshot(list(Some(TOKEN), "")).await.unwrap()).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(ids(&body), want);
    assert!(body["next"].is_null(), "{body}");
    let first = &body["assets"][0];
    assert_eq!(first["mime"], "image/png");
    let mine = (0..3)
        .map(png)
        .find(|b| asset_id(b) == first["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(first["bytes"], mine.len());
    let keys: Vec<&String> = first.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        ["bytes", "id", "mime"],
        "nothing but the three fields"
    );
    // No bytes of any image in the answer, and nothing was submitted or logged.
    assert!(!body.to_string().contains("PNG") && !body.to_string().contains("image 0"));
    assert_eq!(fake.calls(), 0);
}

#[tokio::test]
async fn a_page_has_a_cursor_that_walks_to_the_end_and_the_last_page_has_none() {
    let (app, _, want) = filled(5);
    let mut walked = Vec::new();
    let mut query = "?limit=2".to_owned();
    for _ in 0..5 {
        let (status, body) = json(
            app.clone()
                .oneshot(list(Some(TOKEN), &query))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        walked.extend(ids(&body));
        match body["next"].as_str() {
            Some(next) => query = format!("?limit=2&after={next}"),
            None => break,
        }
    }
    assert_eq!(walked, want);
}

#[tokio::test]
async fn the_default_page_is_50_and_the_most_is_200() {
    let (app, _, _) = filled(205);
    let (_, body) = json(app.clone().oneshot(list(Some(TOKEN), "")).await.unwrap()).await;
    assert_eq!((ids(&body).len(), body["next"].is_string()), (50, true));
    let (_, body) = json(
        app.clone()
            .oneshot(list(Some(TOKEN), "?limit=200"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((ids(&body).len(), body["next"].is_string()), (200, true));
    let (status, body) = json(app.oneshot(list(Some(TOKEN), "?limit=5000")).await.unwrap()).await;
    assert_eq!(
        (status, ids(&body).len()),
        (200, 200),
        "past the most is the most"
    );
}

#[tokio::test]
async fn another_workspace_sees_none_of_it_and_an_empty_one_has_an_empty_list() {
    let (app, _, _) = filled(2);
    let (status, body) = json(app.oneshot(list(Some(OTHER), "")).await.unwrap()).await;
    assert_eq!(status, 200);
    assert_eq!((ids(&body), body["next"].is_null()), (vec![], true));
}

#[tokio::test]
async fn a_cursor_of_another_workspace_gives_nothing_of_it_either() {
    let blobs = Arc::new(MemoryBlobs::default());
    let mut mine: Vec<String> = (0..6)
        .map(|i| blobs.put("ws_1", &png(i)).unwrap().id)
        .collect();
    mine.sort();
    // An id that only workspace 2 holds and that falls inside the order of workspace 1's ids, so the cursor
    // splits the list and the test cannot pass by an empty or a whole answer.
    let theirs_bytes = (100..)
        .map(png)
        .find(|b| {
            let id = asset_id(b);
            id > mine[0] && id < mine[5]
        })
        .unwrap();
    let theirs = blobs.put("ws_2", &theirs_bytes).unwrap().id;
    let (app, _) = app_with(blobs);
    let (status, body) = json(
        app.oneshot(list(Some(TOKEN), &format!("?after={theirs}")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let want: Vec<String> = mine.iter().filter(|id| **id > theirs).cloned().collect();
    assert!(!want.is_empty() && want.len() < mine.len());
    assert_eq!(
        ids(&body),
        want,
        "the cursor only places itself in my order"
    );
    assert!(!ids(&body).contains(&theirs));
}

#[tokio::test]
async fn a_token_is_needed() {
    let (app, _, _) = filled(1);
    let (status, body) = json(app.oneshot(list(None, "")).await.unwrap()).await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (401, Some("unauthenticated"))
    );
}

#[tokio::test]
async fn a_bad_limit_or_cursor_is_invalid_and_reaches_no_store() {
    let (app, _, _) = filled(1);
    for q in [
        "?limit=0",
        "?limit=-1",
        "?limit=many",
        "?limit=",
        "?after=../../etc",
        "?after=ABC",
        "?after=",
    ] {
        let (status, body) = json(app.clone().oneshot(list(Some(TOKEN), q)).await.unwrap()).await;
        assert_eq!(
            (status, body["error"]["code"].as_str()),
            (400, Some("invalid")),
            "{q}"
        );
    }
}

struct Down;
impl BlobStore for Down {
    fn put(&self, _: &str, _: &[u8]) -> Result<assets::Stored, assets::AssetError> {
        Err(assets::AssetError::Unavailable("secret path".into()))
    }
    fn get(&self, _: &str, _: &str) -> Result<(assets::ImageType, Vec<u8>), assets::AssetError> {
        Err(assets::AssetError::Unavailable("secret path".into()))
    }
    fn used(&self, _: &str) -> Result<u64, assets::AssetError> {
        Err(assets::AssetError::Unavailable("secret path".into()))
    }
    fn list(&self, _: &str, _: Option<&str>, _: usize) -> Result<assets::Page, assets::AssetError> {
        Err(assets::AssetError::Unavailable("secret path".into()))
    }
}

#[tokio::test]
async fn a_store_that_fails_is_unavailable_and_retryable_without_its_text() {
    let (app, _) = app_with(Arc::new(Down));
    let res = app.oneshot(list(Some(TOKEN), "")).await.unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let (_, body) = json(res).await;
    assert_eq!(body["error"]["code"], "unavailable");
    assert_eq!(body["error"]["retryable"], true);
    assert!(!body.to_string().contains("secret path"));
}

#[tokio::test]
async fn paging_over_the_disk_store_walks_a_shard_one_asset_at_a_time() {
    let dir = std::env::temp_dir().join(format!(
        "meldframe-assets-list-routes-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let disk = assets::DiskBlobs::new(&dir);
    // Four assets in one shard directory (ids that share their first two characters), made by hand, and one in another.
    let mut want: Vec<String> = (0..4)
        .map(|i| format!("cd{i:02x}{}", "0".repeat(60)))
        .collect();
    for id in &want {
        let path = disk.path_of("ws_1", id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, png(7)).unwrap();
    }
    want.push(disk.put("ws_1", &png(901)).unwrap().id);
    want.sort();
    let (app, _) = app_with(Arc::new(disk));
    let mut walked = Vec::new();
    let mut query = "?limit=1".to_owned();
    for _ in 0..want.len() + 1 {
        let (status, body) = json(
            app.clone()
                .oneshot(list(Some(TOKEN), &query))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(ids(&body).len(), 1, "{query}");
        walked.extend(ids(&body));
        match body["next"].as_str() {
            Some(next) => query = format!("?limit=1&after={next}"),
            None => break,
        }
    }
    assert_eq!(walked, want);
    std::fs::remove_dir_all(&dir).unwrap();
}
