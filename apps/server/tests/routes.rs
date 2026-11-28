//! The asset routes: upload, download, the headers that make an image only an image, the workspace wall and the
//! limits. The router is the real one; the blob store is in memory.

mod http_common;
use assets::{BlobStore, MAX_ASSET_BYTES, MemoryBlobs, Usage, asset_id};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::{StreamExt, stream};
use http_common::*;
use meldframe_http::shared;
use std::sync::Arc;
use tower::ServiceExt;

fn app() -> Router {
    meldframe_http_assets::routes(
        shared(tokens(), Fake::ok(), config()),
        Arc::new(MemoryBlobs::default()),
    )
}

/// A png: the signature is all the store looks at.
fn png(tail: &[u8]) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    v.extend_from_slice(tail);
    v
}

fn upload(token: Option<&str>, bytes: Vec<u8>) -> Request<Body> {
    let mut r = Request::post("/v1/assets");
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    r.body(Body::from(bytes)).unwrap()
}

fn fetch(token: Option<&str>, id: &str) -> Request<Body> {
    let mut r = Request::get(format!("/v1/assets/{id}"));
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    r.body(Body::empty()).unwrap()
}

fn code(v: &serde_json::Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

#[tokio::test]
async fn an_upload_is_named_by_its_bytes_and_comes_back_whole_with_the_safe_headers() {
    let app = app();
    let bytes = png(b"pixels");
    let (status, body) = json(
        app.clone()
            .oneshot(upload(Some(TOKEN), bytes.clone()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["id"], asset_id(&bytes));
    assert_eq!(body["type"], "image/png");
    assert_eq!(body["bytes"], bytes.len());

    let res = app
        .oneshot(fetch(Some(TOKEN), &asset_id(&bytes)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let h = res.headers().clone();
    assert_eq!(h["content-type"], "image/png");
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert!(
        h["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("default-src 'none'")
    );
    assert!(h["cache-control"].to_str().unwrap().contains("private"));
    assert_eq!(h["vary"], "Authorization");
    let got = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    assert_eq!(got.as_ref(), bytes.as_slice());
}

#[tokio::test]
async fn the_same_bytes_are_the_same_asset() {
    let app = app();
    let bytes = png(b"same");
    let first = json(
        app.clone()
            .oneshot(upload(Some(TOKEN), bytes.clone()))
            .await
            .unwrap(),
    )
    .await;
    let second = json(app.oneshot(upload(Some(TOKEN), bytes)).await.unwrap()).await;
    assert_eq!(first.1["id"], second.1["id"]);
}

#[tokio::test]
async fn another_workspace_does_not_see_an_asset_even_with_its_id() {
    let app = app();
    let bytes = png(b"mine");
    let id = asset_id(&bytes);
    app.clone()
        .oneshot(upload(Some(TOKEN), bytes.clone()))
        .await
        .unwrap();
    let (status, body) = json(app.clone().oneshot(fetch(Some(OTHER), &id)).await.unwrap()).await;
    assert_eq!((status, code(&body)), (404, "asset_not_found"));
    // The other workspace can hold the same image on its own.
    let (status, _) = json(
        app.clone()
            .oneshot(upload(Some(OTHER), bytes))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        app.oneshot(fetch(Some(OTHER), &id)).await.unwrap().status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_token_is_needed_for_both_routes() {
    let app = app();
    let (status, body) = json(app.clone().oneshot(upload(None, png(b"x"))).await.unwrap()).await;
    assert_eq!((status, code(&body)), (401, "unauthenticated"));
    let (status, body) = json(app.oneshot(fetch(None, &asset_id(b"x"))).await.unwrap()).await;
    assert_eq!((status, code(&body)), (401, "unauthenticated"));
}

#[tokio::test]
async fn what_is_not_a_png_jpeg_webp_or_gif_is_refused() {
    let app = app();
    for (bytes, want, status_want) in [
        (
            b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec(),
            "asset_type_refused",
            415,
        ),
        (b"plain text".to_vec(), "asset_type_refused", 415),
        (Vec::new(), "invalid", 400),
    ] {
        let (status, body) = json(
            app.clone()
                .oneshot(upload(Some(TOKEN), bytes))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!((status, code(&body)), (status_want, want));
    }
}

#[tokio::test]
async fn an_id_that_is_not_a_hash_is_invalid_and_an_unknown_hash_is_not_found() {
    let app = app();
    let (status, body) = json(
        app.clone()
            .oneshot(fetch(Some(TOKEN), "not-a-hash"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((status, code(&body)), (400, "invalid"));
    let (status, body) = json(
        app.oneshot(fetch(Some(TOKEN), &asset_id(b"never uploaded")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((status, code(&body)), (404, "asset_not_found"));
}

#[tokio::test]
async fn an_upload_over_the_limit_is_refused_by_its_declared_length_and_by_its_stream() {
    let app = app();
    // Declared: refused before the body is read.
    let declared = Request::post("/v1/assets")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-length", MAX_ASSET_BYTES + 1)
        .body(Body::empty())
        .unwrap();
    let (status, body) = json(app.clone().oneshot(declared).await.unwrap()).await;
    assert_eq!((status, code(&body)), (413, "asset_too_large"));
    // Streamed without a length: read no further than the limit and one byte.
    let chunk = png(&vec![0u8; 1 << 20]);
    let chunks = (0..11).map(move |_| Ok::<_, std::io::Error>(chunk.clone()));
    let streamed = Request::post("/v1/assets")
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::from_stream(stream::iter(chunks)))
        .unwrap();
    let (status, body) = json(app.oneshot(streamed).await.unwrap()).await;
    assert_eq!((status, code(&body)), (413, "asset_too_large"));
}

fn app_with(blobs: Arc<dyn assets::BlobStore>, options: meldframe_http_assets::Options) -> Router {
    let usage = Usage::new(blobs.clone());
    meldframe_http_assets::routes_with(
        shared(tokens(), Fake::ok(), config()),
        blobs,
        usage,
        Arc::new(|_| (0, None)),
        options,
    )
}

#[tokio::test]
async fn the_disk_store_works_through_the_routes_and_keeps_the_file() {
    let dir = std::env::temp_dir().join(format!("meldframe-assets-routes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let app = app_with(
        Arc::new(assets::DiskBlobs::new(&dir)),
        meldframe_http_assets::Options::default(),
    );
    let bytes = png(b"on disk");
    let id = asset_id(&bytes);
    let (status, _) = json(
        app.clone()
            .oneshot(upload(Some(TOKEN), bytes.clone()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    let res = app.clone().oneshot(fetch(Some(TOKEN), &id)).await.unwrap();
    let got = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    assert_eq!(got.as_ref(), bytes.as_slice());
    // A file changed on disk is never served under the old id.
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .flat_map(|d| std::fs::read_dir(d.unwrap().path()).unwrap())
        .flat_map(|d| std::fs::read_dir(d.unwrap().path()).unwrap())
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(&file, png(b"changed")).unwrap();
    let (status, body) = json(app.oneshot(fetch(Some(TOKEN), &id)).await.unwrap()).await;
    assert_eq!((status, code(&body)), (422, "asset_corrupt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn an_upload_that_never_ends_times_out_and_a_full_set_of_places_is_unavailable() {
    let app = app_with(
        Arc::new(MemoryBlobs::default()),
        meldframe_http_assets::Options {
            uploads: 1,
            uploads_per_principal: 1,
            upload_timeout: std::time::Duration::from_millis(600),
        },
    );
    // The first upload sends one chunk and then nothing: it holds the only place.
    let first = Request::post("/v1/assets")
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::from_stream(
            stream::iter([Ok::<_, std::io::Error>(png(b"half"))]).chain(stream::pending()),
        ))
        .unwrap();
    let held = tokio::spawn({
        let app = app.clone();
        async move { json(app.oneshot(first).await.unwrap()).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let (status, body) = json(
        app.clone()
            .oneshot(upload(Some(TOKEN), png(b"second")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((status, code(&body)), (503, "unavailable"));
    let (status, body) = held.await.unwrap();
    assert_eq!((status, code(&body)), (408, "request_timeout"));
    // The place is free again.
    let (status, _) = json(
        app.oneshot(upload(Some(TOKEN), png(b"third")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
}

/// A router whose workspace holds `ops` bytes of ops and may hold `limit` bytes in all.
fn app_limited(ops: u64, limit: u64) -> (Router, Usage) {
    let blobs: Arc<dyn BlobStore> = Arc::new(MemoryBlobs::default());
    let usage = Usage::new(blobs.clone());
    let router = meldframe_http_assets::routes_with(
        shared(tokens(), Fake::ok(), config()),
        blobs,
        usage.clone(),
        Arc::new(move |_| (ops, Some(limit))),
        meldframe_http_assets::Options::default(),
    );
    (router, usage)
}

#[tokio::test]
async fn an_upload_that_does_not_fit_the_storage_limit_is_quota_storage_and_writes_nothing() {
    let first = png(b"first");
    let second = png(b"second image");
    // Room for the first image and the ops, not for both images.
    let (app, usage) = app_limited(100, 100 + first.len() as u64 + 5);
    let (status, _) = json(
        app.clone()
            .oneshot(upload(Some(TOKEN), first.clone()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(usage.used("ws_1"), Some(first.len() as u64));
    let (status, body) = json(
        app.clone()
            .oneshot(upload(Some(TOKEN), second.clone()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((status, code(&body)), (413, "quota_storage"));
    assert_eq!(
        usage.used("ws_1"),
        Some(first.len() as u64),
        "nothing was taken"
    );
    let (status, body) = json(
        app.oneshot(fetch(Some(TOKEN), &asset_id(&second)))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!((status, code(&body)), (404, "asset_not_found"));
}

#[tokio::test]
async fn an_image_the_workspace_holds_already_costs_nothing_even_at_the_limit() {
    let bytes = png(b"again");
    let (app, usage) = app_limited(0, bytes.len() as u64);
    for _ in 0..3 {
        let (status, _) = json(
            app.clone()
                .oneshot(upload(Some(TOKEN), bytes.clone()))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, 200);
    }
    assert_eq!(usage.used("ws_1"), Some(bytes.len() as u64), "counted once");
}

/// A store that cannot say what it holds.
struct Down;
impl BlobStore for Down {
    fn put(&self, _: &str, _: &[u8]) -> Result<assets::Stored, assets::AssetError> {
        Err(assets::AssetError::Unavailable("down".into()))
    }
    fn get(&self, _: &str, _: &str) -> Result<(assets::ImageType, Vec<u8>), assets::AssetError> {
        Err(assets::AssetError::Unavailable("down".into()))
    }
    fn used(&self, _: &str) -> Result<u64, assets::AssetError> {
        Err(assets::AssetError::Unavailable("down".into()))
    }
    fn list(&self, _: &str, _: Option<&str>, _: usize) -> Result<assets::Page, assets::AssetError> {
        Err(assets::AssetError::Unavailable("down".into()))
    }
}

#[tokio::test]
async fn a_store_that_cannot_say_what_it_holds_makes_an_upload_unavailable_and_retryable() {
    let blobs: Arc<dyn BlobStore> = Arc::new(Down);
    let app = meldframe_http_assets::routes_with(
        shared(tokens(), Fake::ok(), config()),
        blobs.clone(),
        Usage::new(blobs),
        Arc::new(|_| (0, Some(1 << 30))),
        meldframe_http_assets::Options::default(),
    );
    let (status, body) = json(app.oneshot(upload(Some(TOKEN), png(b"x"))).await.unwrap()).await;
    assert_eq!((status, code(&body)), (503, "unavailable"));
    assert_eq!(body["error"]["retryable"], true);
}
