//! The requests behind the effects of the app layer that are not an op: the batch list and review, the asset list and the
//! three checkpoint routes. The shell builds the path, the query and the body (the request side stays in the shell); the
//! answer is not read here, its status and text go back as an `Action::Wire` for the app layer to check (finding T6), and
//! a request that got no answer at all is the failure the app reads for that request.

use crate::http::Method;
use meldframe_app_model::checkpoint::CheckpointRequest;
use meldframe_app_model::wire::Wire;
use meldframe_app_model::{Action, Effect};
use meldframe_protocol::is_asset_id;
use serde_json::{Value, json};

/// The route's page size is 1 to 200.
const MAX_ASSET_PAGE: u32 = 200;

/// One request and what its answer, or the lack of one, is called.
pub struct Call {
    pub method: Method,
    pub path: String,
    pub body: Option<String>,
    /// The action for the status and the text of an answer, as the network gave them.
    pub answer: Box<dyn FnOnce(u16, String) -> Action + Send>,
    /// The action for a request that got no answer.
    pub failure: Action,
}

/// Percent-encode a path segment the way `encodeURIComponent` does (the unreserved characters stay).
pub fn segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for b in raw.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn wire(w: Wire) -> Action {
    Action::Wire { wire: w }
}

fn unavailable() -> String {
    "unavailable".into()
}

/// The request an effect stands for, or `None` for an effect that is not a request.
pub fn call(file: &str, effect: &Effect) -> Option<Call> {
    let files = format!("/v1/files/{}", segment(file));
    Some(match effect {
        Effect::FetchBatches {
            before_n,
            after_n,
            limit,
        } => {
            let mut query = Vec::new();
            if let Some(n) = before_n {
                query.push(format!("before={n}"));
            }
            if let Some(n) = after_n {
                query.push(format!("after={n}"));
            }
            query.push(format!("limit={limit}"));
            Call {
                method: Method::Get,
                path: format!("{files}/batches?{}", query.join("&")),
                body: None,
                answer: Box::new(|status, body| wire(Wire::Batches { status, body })),
                failure: Action::BatchRequestFailed {
                    code: unavailable(),
                },
            }
        }
        Effect::PutBatchReview { n, state } => Call {
            method: Method::Put,
            path: format!("{files}/batches/{n}/review"),
            body: Some(json!({ "state": state }).to_string()),
            answer: Box::new(|status, body| wire(Wire::BatchReview { status, body })),
            failure: Action::BatchRequestFailed {
                code: unavailable(),
            },
        },
        Effect::FetchAssets {
            after,
            limit,
            request,
        } => {
            let mut query = format!("limit={}", (*limit).clamp(1, MAX_ASSET_PAGE));
            // A cursor is an asset id (64 lower-case hex digits); anything else is left out.
            let cursor = after.as_deref().filter(|a| is_asset_id(a));
            if let Some(a) = cursor {
                query.push_str(&format!("&after={a}"));
            }
            let (request, after) = (*request, after.clone());
            Call {
                method: Method::Get,
                path: format!("/v1/assets?{query}"),
                body: None,
                answer: Box::new(move |status, body| {
                    wire(Wire::Assets {
                        request,
                        after,
                        status,
                        body,
                    })
                }),
                failure: Action::AssetsRequestFailed {
                    request,
                    code: unavailable(),
                },
            }
        }
        Effect::FetchCheckpoints {} => Call {
            method: Method::Get,
            path: format!("{files}/checkpoints"),
            body: None,
            answer: Box::new(|status, body| wire(Wire::Checkpoints { status, body })),
            failure: checkpoint_failed(CheckpointRequest::List {}),
        },
        Effect::PostCheckpoint {
            key,
            kind,
            label,
            about,
            seq,
        } => {
            let mut body = json!({ "key": key, "kind": kind, "seq": seq });
            for (name, value) in [("label", label), ("about", about)] {
                if let (Some(v), Value::Object(map)) = (value, &mut body) {
                    map.insert(name.into(), json!(v));
                }
            }
            let answered = key.clone();
            Call {
                method: Method::Post,
                path: format!("{files}/checkpoints"),
                body: Some(body.to_string()),
                answer: Box::new(move |status, body| {
                    wire(Wire::CheckpointPosted {
                        key: answered,
                        status,
                        body,
                    })
                }),
                failure: checkpoint_failed(CheckpointRequest::Post { key: key.clone() }),
            }
        }
        Effect::FetchCheckpointState { n } => {
            let n = *n;
            Call {
                method: Method::Get,
                path: format!("{files}/checkpoints/{n}/state"),
                body: None,
                answer: Box::new(move |status, body| {
                    wire(Wire::CheckpointState { n, status, body })
                }),
                failure: checkpoint_failed(CheckpointRequest::State { n }),
            }
        }
        _ => return None,
    })
}

fn checkpoint_failed(request: CheckpointRequest) -> Action {
    Action::CheckpointRequestFailed {
        request,
        code: unavailable(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meldframe_app_model::activity::ReviewState;

    fn path(e: Effect) -> (Method, String, Option<String>) {
        let c = call("my file/é", &e).expect("a request");
        (c.method, c.path, c.body)
    }

    #[test]
    fn a_file_name_is_one_encoded_segment() {
        assert_eq!(segment("my file/é"), "my%20file%2F%C3%A9");
        assert_eq!(segment("file_dev-1.x"), "file_dev-1.x");
    }

    #[test]
    fn the_batch_routes_leave_out_a_bound_that_is_none() {
        let (m, p, _) = path(Effect::FetchBatches {
            before_n: None,
            after_n: Some(4),
            limit: 50,
        });
        assert_eq!(
            (m, p.as_str()),
            (
                Method::Get,
                "/v1/files/my%20file%2F%C3%A9/batches?after=4&limit=50"
            )
        );
        let (m, p, b) = path(Effect::PutBatchReview {
            n: 3,
            state: ReviewState::NeedsReview,
        });
        assert_eq!(
            (m, p.as_str()),
            (Method::Put, "/v1/files/my%20file%2F%C3%A9/batches/3/review")
        );
        assert_eq!(b.as_deref(), Some(r#"{"state":"needs_review"}"#));
    }

    #[test]
    fn the_asset_list_holds_the_limit_and_takes_only_an_asset_id_as_cursor() {
        let id = "a".repeat(64);
        let fetch = |after: Option<String>, limit| Effect::FetchAssets {
            after,
            limit,
            request: 1,
        };
        assert_eq!(path(fetch(None, 0)).1, "/v1/assets?limit=1");
        assert_eq!(
            path(fetch(Some(id.clone()), 900)).1,
            format!("/v1/assets?limit=200&after={id}")
        );
        assert_eq!(path(fetch(Some("../x".into()), 5)).1, "/v1/assets?limit=5");
    }

    #[test]
    fn a_checkpoint_post_names_its_key_in_the_answer_and_the_failure() {
        use meldframe_app_model::checkpoint::CheckpointKind;
        let e = Effect::PostCheckpoint {
            key: "k1".into(),
            kind: CheckpointKind::Manual,
            label: Some("before".into()),
            about: None,
            seq: 7,
        };
        let c = call("f", &e).expect("a request");
        assert_eq!(
            c.body.as_deref(),
            Some(r#"{"key":"k1","kind":"manual","label":"before","seq":7}"#)
        );
        assert!(matches!(
            c.failure,
            Action::CheckpointRequestFailed { request: CheckpointRequest::Post { ref key }, .. } if key == "k1"
        ));
        assert!(matches!(
            (c.answer)(201, "{}".into()),
            Action::Wire { wire: Wire::CheckpointPosted { ref key, status: 201, .. } } if key == "k1"
        ));
    }

    #[test]
    fn an_effect_that_is_not_a_request_has_no_call() {
        assert!(call("f", &Effect::Render { nodes: vec![] }).is_none());
        assert!(call("f", &Effect::Subscribe { after_seq: 0 }).is_none());
    }
}
