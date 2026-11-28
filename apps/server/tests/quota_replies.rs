//! The HTTP replies of the workspace quotas (ADR-0014): a wait is 429 with `Retry-After` in whole seconds, rounded
//! up and never zero; a call that cannot fit as sent is 413 without it. The code and `retryable` match the refusal.

use axum::response::IntoResponse;
use meldframe_host::Refusal;
use meldframe_http::{Failure, Reply};
use serde_json::Value;

/// The status, the `Retry-After` header and the JSON body of the reply to a refusal.
fn reply(r: Refusal) -> (u16, Option<String>, Value) {
    let res = Reply::failure(&Failure::Refused(r)).into_response();
    let status = res.status().as_u16();
    let retry = res
        .headers()
        .get("retry-after")
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(axum::body::to_bytes(res.into_body(), usize::MAX))
        .unwrap();
    (status, retry, serde_json::from_slice(&bytes).unwrap())
}

#[test]
fn ops_per_minute_is_429_with_retry_after_rounded_up_to_a_second() {
    let (status, retry, body) = reply(Refusal::RateExceeded {
        limit: 5,
        retry_after_ms: 1_500,
    });
    assert_eq!((status, retry.as_deref()), (429, Some("2")));
    assert_eq!(body["error"]["code"], "quota_ops_per_minute");
    assert_eq!(body["error"]["retryable"], true);
    let (_, retry, _) = reply(Refusal::RateExceeded {
        limit: 5,
        retry_after_ms: 1,
    });
    assert_eq!(
        retry.as_deref(),
        Some("1"),
        "under a second still asks for one"
    );
}

#[test]
fn batch_too_large_and_storage_are_413_without_retry_after() {
    for (r, code) in [
        (Refusal::BatchTooLarge { limit: 100 }, "batch_too_large"),
        (Refusal::StorageExceeded { limit: 10 }, "quota_storage"),
    ] {
        let (status, retry, body) = reply(r);
        assert_eq!((status, retry), (413, None), "{code}");
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["error"]["retryable"], false, "{code}");
    }
}
