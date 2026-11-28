//! What the setter does not tell a caller it may not use, and how it reads `n` and the body (ADR-0021, slice 7): an
//! unlisted caller, or any caller when `MELDFRAME_REVIEWERS` is unset, is `forbidden` whatever it sends, so it cannot
//! probe which batches exist or what the route accepts; a listed caller gets `invalid` for a number that is not
//! plain digits, and the body follows the limits of the other routes.

mod activity_support;
use activity_support::*;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const GOOD: &str = r#"{"state":"accepted"}"#;

/// Every shape of a bad request a prober might send, as (path, body).
fn probes() -> Vec<(String, String)> {
    let at = |n: &str| format!("/v1/files/f/batches/{n}/review");
    let mut v = vec![
        // A number the file has not, one that is not a number, and numbers with other spellings of 1.
        (at("9"), GOOD.to_owned()),
        (at("0"), GOOD.to_owned()),
        (at("x"), GOOD.to_owned()),
        (at("+1"), GOOD.to_owned()),
        (at("01"), GOOD.to_owned()),
        (at("99999999999999999999"), GOOD.to_owned()),
        (
            "/v1/files/f%01/batches/1/review".to_owned(),
            GOOD.to_owned(),
        ),
        (
            "/v1/files/nowhere/batches/1/review".to_owned(),
            GOOD.to_owned(),
        ),
    ];
    for body in [
        "",
        "not json",
        "[]",
        r#"{"state":"done"}"#,
        r#"{"state":"accepted","by":"x"}"#,
        "{}",
    ] {
        v.push((PATH.to_owned(), body.to_owned()));
        v.push((at("x"), body.to_owned()));
    }
    v.push((
        PATH.to_owned(),
        format!(r#"{{"state":"{}"}}"#, "a".repeat(5000)),
    ));
    v
}

async fn assert_all_forbidden(w: &World, token: &str) {
    for (path, body) in probes() {
        let (status, b) = put_to(w, Some(token), &path, &body).await;
        assert_eq!(
            (status, code(&b)),
            (403, "forbidden"),
            "{token} {path} {body:.40}"
        );
    }
    assert_eq!(header(w).rev, 1);
}

#[tokio::test]
async fn an_unlisted_caller_is_forbidden_whatever_it_sends() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    for token in [CLAUDE, AGENT, OTHER, BIG_AGENT, PERSON_X] {
        assert_all_forbidden(&w, token).await;
    }
}

#[tokio::test]
async fn with_no_reviewers_every_caller_is_forbidden_whatever_it_sends() {
    let w = served("", Some(Policy::Auto)).await;
    for token in EVERYONE {
        assert_all_forbidden(&w, token).await;
    }
}

#[tokio::test]
async fn a_listed_caller_denied_by_policy_is_forbidden_whatever_it_sends() {
    let w = served("user:bea", Some(Policy::Deny)).await;
    assert_all_forbidden(&w, BEA).await;
}

#[tokio::test]
async fn a_number_is_plain_ascii_digits_without_a_leading_zero() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    for n in [
        "+1",
        "01",
        "001",
        " 1",
        "1 ",
        "1e0",
        "0x1",
        "%EF%BC%91",
        "%D9%A1",
        "-0",
        "1_0",
    ] {
        let path = format!("/v1/files/f/batches/{n}/review");
        let (status, b) = put_to(&w, Some(BEA), &path, GOOD).await;
        assert_eq!((status, code(&b)), (400, "invalid"), "{n}");
    }
    assert_eq!(header(&w).rev, 1, "no spelling of 1 reached the batch");
    let (status, b) = put_to(&w, Some(BEA), PATH, GOOD).await;
    assert_eq!((status, b["batch"]["n"].clone()), (200, json!(1)));
}

#[tokio::test]
async fn a_body_over_the_limit_is_too_large_and_a_slow_body_is_a_request_timeout() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    let big = format!(r#"{{"state":"{}"}}"#, "a".repeat(5000));
    let (status, b) = put_to(&w, Some(BEA), PATH, &big).await;
    assert_eq!((status, code(&b)), (413, "too_large"));
    // The headers promise 40 bytes and 10 arrive: the body never completes.
    let mut tcp = tokio::net::TcpStream::connect(w.h.addr).await.unwrap();
    let head = format!(
        "PUT {PATH} HTTP/1.1\r\nhost: x\r\nauthorization: Bearer {BEA}\r\ncontent-type: application/json\r\ncontent-length: 40\r\n\r\n{{\"state\":\""
    );
    tcp.write_all(head.as_bytes()).await.unwrap();
    let mut got = vec![0u8; 4096];
    let n = tokio::time::timeout(Duration::from_secs(5), tcp.read(&mut got))
        .await
        .unwrap()
        .unwrap();
    let text = String::from_utf8_lossy(&got[..n]).to_string();
    assert!(text.starts_with("HTTP/1.1 408"), "{text}");
    let body: Value = serde_json::from_str(text.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(code(&body), "request_timeout");
    assert_eq!(header(&w).rev, 1);
}
