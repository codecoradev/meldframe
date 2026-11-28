//! The review setter through the real router (ADR-0021, slice 7): `PUT /v1/files/{file}/batches/{n}/review` is for
//! the actors named in `MELDFRAME_REVIEWERS` and nobody else, takes a `rev`, goes out as a `batch` frame to every open
//! stream of the file, and is logged with its actor and outcome code and nothing more.

mod activity_support;
use activity_support::*;
use meldframe_http_live::LiveSource;
use meldframe_protocol::ServerMsg;
use meldframe_server::StreamId;
use serde_json::{Value, json};

#[tokio::test]
async fn unset_refuses_every_caller_as_forbidden() {
    let w = served("", Some(Policy::Auto)).await;
    for token in EVERYONE {
        let (status, body) = set(&w, token, "accepted").await;
        assert_eq!((status, code(&body)), (403, "forbidden"), "{token}");
    }
    let h = header(&w);
    assert_eq!(
        (h.review.as_str(), h.rev, h.reviewed_by),
        ("unreviewed", 1, None)
    );
}

#[tokio::test]
async fn prefixes_and_near_misses_are_refused_unless_listed_by_exact_match() {
    let w = served(" agent:x ,, user:bea", Some(Policy::Auto)).await;
    for token in [CLAUDE, BIG_AGENT, BOT, PERSON_X, AGENT, OTHER] {
        let (status, body) = set(&w, token, "accepted").await;
        assert_eq!((status, code(&body)), (403, "forbidden"), "{token}");
    }
    assert_eq!(header(&w).review.as_str(), "unreviewed");
    let (status, body) = set(&w, AGENT_X, "accepted").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["batch"]["reviewedBy"], "agent:x");
    let (status, _) = set(&w, BEA, "needs_review").await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn the_policy_of_the_registry_can_deny_the_setter_to_a_listed_actor() {
    for policy in [None, Some(Policy::Deny), Some(Policy::Ask)] {
        let w = served("user:bea", policy).await;
        let (status, body) = set(&w, BEA, "accepted").await;
        assert_eq!((status, code(&body)), (403, "forbidden"), "{policy:?}");
        assert_eq!(header(&w).rev, 1);
    }
}

#[tokio::test]
async fn a_listed_actor_sets_and_clears() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    let (status, body) = set(&w, BEA, "accepted").await;
    assert_eq!(status, 200, "{body}");
    let b = &body["batch"];
    assert_eq!(
        (b["review"].clone(), b["rev"].clone()),
        (json!("accepted"), json!(2))
    );
    assert_eq!(b["reviewedBy"], "user:bea");
    assert!(b["reviewedAtMs"].as_u64().unwrap() > 0);
    assert_eq!(
        (b["n"].clone(), b["actor"].clone()),
        (json!(1), json!("agent:alpha"))
    );
    assert_eq!(body["unreviewedCount"], 0);
    let (_, body) = set(&w, BEA, "needs_review").await;
    assert_eq!(
        (
            body["batch"]["review"].clone(),
            body["unreviewedCount"].clone()
        ),
        (json!("needs_review"), json!(1))
    );
    let (_, body) = set(&w, BEA, "unreviewed").await;
    // A clear names who cleared it, as "last changed by".
    assert_eq!(body["batch"]["review"], "unreviewed");
    assert_eq!(
        (
            body["batch"]["rev"].clone(),
            body["batch"]["reviewedBy"].clone()
        ),
        (json!(4), json!("user:bea"))
    );
    assert_eq!(header(&w).rev, 4);
}

#[tokio::test]
async fn the_list_says_can_review_only_to_an_actor_the_setter_would_let_in() {
    let get = |w: &World, token: &'static str| {
        let url = format!("http://{}/v1/files/f/batches", w.h.addr);
        let req = reqwest::Client::new().get(url).bearer_auth(token);
        async move { req.send().await.unwrap().json::<Value>().await.unwrap()["canReview"].clone() }
    };
    let w = served("user:bea", Some(Policy::Auto)).await;
    for (token, expected) in [(BEA, true), (AGENT, false), (CLAUDE, false), (OTHER, false)] {
        assert_eq!(get(&w, token).await, expected, "{token}");
    }
    // Listed but denied by the registry's policy: the setter refuses, so the list does not offer the controls.
    for policy in [None, Some(Policy::Deny), Some(Policy::Ask)] {
        let denied = served("user:bea", policy).await;
        assert_eq!(get(&denied, BEA).await, false, "{policy:?}");
        assert_eq!(set(&denied, BEA, "accepted").await.0, 403);
    }
    let none = served("", Some(Policy::Auto)).await;
    assert_eq!(get(&none, BEA).await, false);
}

#[tokio::test]
async fn two_editors_setting_at_once_end_with_the_higher_rev_on_both() {
    let w = served("user:bea,agent:x", Some(Policy::Auto)).await;
    let ps = [who("ws_1", "user:bea"), who("ws_1", "agent:x")];
    let streams: Vec<StreamId> = ps
        .iter()
        .map(|p| w.server.subscribe(p, "f", 0, 0).unwrap().id)
        .collect();
    let (a, b) = tokio::join!(set(&w, BEA, "accepted"), set(&w, AGENT_X, "needs_review"));
    assert!(a.0 == 200 && b.0 == 200, "{a:?} {b:?}");
    // Both differ from the state they found or from each other: each took a rev, so the file ends at rev 3.
    let end = header(&w);
    assert_eq!(end.rev, 3);
    for (p, id) in ps.iter().zip(streams) {
        let frames = batch_frames(&w, p, id);
        let mut rows: Vec<_> = frames
            .iter()
            .filter_map(|m| match m {
                ServerMsg::Batch {
                    rev,
                    review,
                    unreviewed_count,
                    reviewed_by,
                    ..
                } => Some((*rev, review.clone(), *unreviewed_count, reviewed_by.clone())),
                _ => None,
            })
            .collect();
        // Frames may arrive in either order; the app keeps the row of the highest rev, and that is the stored header.
        rows.sort_by_key(|r| r.0);
        assert_eq!(
            rows.iter().map(|r| r.0).collect::<Vec<_>>(),
            [2, 3],
            "{}",
            p.actor
        );
        let last = rows.last().unwrap();
        assert_eq!(
            (last.1.as_str(), last.2),
            (
                end.review.as_str(),
                u32::from(end.review.as_str() != "accepted")
            )
        );
        assert_eq!(last.3, end.reviewed_by);
        assert_ne!(rows[0].1, last.1, "the two writers set different states");
    }
}

#[tokio::test]
async fn an_unknown_n_is_not_found_and_a_bad_n_is_invalid() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    let body = json!({"state": "accepted"}).to_string();
    for n in ["2", "0", "4294967295"] {
        let path = format!("/v1/files/f/batches/{n}/review");
        let (status, b) = put_to(&w, Some(BEA), &path, &body).await;
        assert_eq!((status, code(&b)), (404, "not_found"), "{n}");
    }
    for n in ["x", "-1", "1.5", "4294967296", "99999999999999999999"] {
        let path = format!("/v1/files/f/batches/{n}/review");
        let (status, b) = put_to(&w, Some(BEA), &path, &body).await;
        assert_eq!((status, code(&b)), (400, "invalid"), "{n}");
    }
    // A file with no batches has none to review either.
    let (status, b) = put_to(&w, Some(BEA), "/v1/files/nowhere/batches/1/review", &body).await;
    assert_eq!((status, code(&b)), (404, "not_found"));
    assert_eq!(header(&w).rev, 1);
}

#[tokio::test]
async fn a_bad_state_or_body_is_refused_and_changes_nothing() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    for body in [
        r#"{"state":"done"}"#,
        r#"{"state":"Accepted"}"#,
        r#"{"state":""}"#,
        r#"{"state":null}"#,
        r#"{"state":7}"#,
        r#"{}"#,
        r#"{"state":"accepted","by":"user:eve"}"#,
    ] {
        let (status, b) = put_to(&w, Some(BEA), PATH, body).await;
        assert_eq!((status, code(&b)), (400, "invalid"), "{body}");
    }
    for body in ["", "not json", "[]", "\"accepted\""] {
        let (status, b) = put_to(&w, Some(BEA), PATH, body).await;
        assert_eq!((status, code(&b)), (400, "bad_request"), "{body:?}");
    }
    let big = format!(r#"{{"state":"{}"}}"#, "a".repeat(5000));
    assert_eq!(put_to(&w, Some(BEA), PATH, &big).await.0, 413);
    let (status, b) = put_to(
        &w,
        Some(BEA),
        "/v1/files/f%01/batches/1/review",
        r#"{"state":"accepted"}"#,
    )
    .await;
    assert_eq!((status, code(&b)), (400, "invalid"));
    assert_eq!(header(&w).rev, 1);
    // No token is no answer about the batch.
    assert_eq!(
        put_to(&w, None, PATH, r#"{"state":"accepted"}"#).await.0,
        401
    );
}

#[tokio::test]
async fn the_same_state_is_a_200_no_op_with_no_new_rev_and_no_frame() {
    let w = served("user:bea,agent:x", Some(Policy::Auto)).await;
    let p = who("ws_1", "user:bea");
    let id = w.server.subscribe(&p, "f", 0, 0).unwrap().id;
    // A fresh batch is unreviewed already.
    let (status, body) = set(&w, BEA, "unreviewed").await;
    assert_eq!((status, body["batch"]["rev"].clone()), (200, json!(1)));
    let first = set(&w, BEA, "accepted").await.1;
    // Another reviewer setting the same state changes neither the rev nor who and when.
    let again = set(&w, AGENT_X, "accepted").await.1;
    assert_eq!(first["batch"], again["batch"]);
    assert_eq!(
        (
            again["batch"]["rev"].clone(),
            again["batch"]["reviewedBy"].clone()
        ),
        (json!(2), json!("user:bea"))
    );
    let revs: Vec<u64> = batch_frames(&w, &p, id)
        .iter()
        .filter_map(|m| match m {
            ServerMsg::Batch { rev, .. } => Some(*rev),
            _ => None,
        })
        .collect();
    assert_eq!(revs, [2], "one change; the no-ops sent nothing");
}

#[tokio::test]
async fn another_workspaces_batch_is_not_found_even_to_a_listed_actor() {
    let w = served("user:bea,agent:gamma", Some(Policy::Auto)).await;
    let (status, b) = set(&w, OTHER, "accepted").await;
    assert_eq!((status, code(&b)), (404, "not_found"));
    assert_eq!(header(&w).rev, 1);
    // The other workspace's own file of the same name has its own batch 1.
    let c = connect(&w.h, OTHER).await;
    let args = apply("f", "k9", vec![frame("z", "Z")], json!({}));
    assert!(!call(&c, "apply_commands", args).await.is_error);
    let (status, b) = set(&w, OTHER, "needs_review").await;
    assert_eq!(
        (status, b["batch"]["actor"].clone()),
        (200, json!("agent:gamma"))
    );
    assert_eq!(header(&w).review.as_str(), "unreviewed");
}

#[tokio::test]
async fn every_open_stream_of_the_file_gets_the_frame_and_no_other_workspace_does() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    let readers = [
        who("ws_1", "user:bea"),
        who("ws_1", "agent:alpha"),
        who("ws_1", "agent:x"),
    ];
    let ids: Vec<StreamId> = readers
        .iter()
        .map(|p| w.server.subscribe(p, "f", 0, 0).unwrap().id)
        .collect();
    let second = w.server.subscribe(&readers[0], "f", 0, 0).unwrap().id;
    let stranger = who("ws_2", "agent:gamma");
    let sid = w.server.subscribe(&stranger, "f", 0, 0).unwrap().id;
    let other_file = w.server.subscribe(&readers[0], "g", 0, 0).unwrap().id;
    assert_eq!(set(&w, BEA, "needs_review").await.0, 200);
    let all = readers.iter().zip(ids).chain([(&readers[0], second)]);
    for (p, id) in all {
        let last = batch_frames(&w, p, id).pop().expect("a frame");
        let ServerMsg::Batch {
            room,
            n,
            rev,
            actor,
            review,
            reviewed_by,
            reviewed_at_ms,
            unreviewed_count,
            note,
            ..
        } = last
        else {
            unreachable!()
        };
        assert_eq!(
            (room.as_str(), n, rev, actor.as_str()),
            ("f", 1, 2, "agent:alpha")
        );
        assert_eq!(
            (review.as_str(), reviewed_by.as_deref(), unreviewed_count),
            ("needs_review", Some("user:bea"), 1)
        );
        assert!(reviewed_at_ms.is_some_and(|t| t > 0));
        assert_eq!(note.as_deref(), Some("n"));
    }
    let mine = |p: &Principal, f: &str, id| w.server.poll(p, f, id, 0, 100_000).len();
    assert_eq!(mine(&stranger, "f", sid), 0);
    assert_eq!(mine(&readers[0], "g", other_file), 0);
}

#[tokio::test]
async fn the_call_log_names_the_actor_and_the_outcome_and_holds_no_state_or_number() {
    let w = served("user:bea", Some(Policy::Auto)).await;
    let steps = [
        (BEA, PATH, r#"{"state":"accepted"}"#, "ok"),
        (
            BEA,
            "/v1/files/f/batches/9/review",
            r#"{"state":"needs_review"}"#,
            "not_found",
        ),
        (BEA, PATH, r#"{"state":"maybe"}"#, "invalid"),
        (CLAUDE, PATH, r#"{"state":"needs_review"}"#, "forbidden"),
    ];
    for (token, path, body, _) in steps {
        put_to(&w, Some(token), path, body).await;
    }
    let log = w.server.calls(100);
    let mine: Vec<_> = log.iter().filter(|e| e.tool == "batch_review").collect();
    assert_eq!(mine.len(), 4, "{log:?}");
    for (e, (token, _, _, outcome)) in mine.iter().zip(steps) {
        let actor = if token == BEA { "user:bea" } else { "claude" };
        assert_eq!(
            (
                e.actor.as_str(),
                e.outcome.as_str(),
                e.file.as_str(),
                e.workspace.as_str()
            ),
            (actor, outcome, "f", "ws_1")
        );
        assert_eq!(e.risk, Some(meldframe_module::RiskTier::Change));
        let text = format!("{e:?}");
        assert!(
            !text.contains("accepted") && !text.contains("needs_review") && !text.contains("maybe"),
            "{text}"
        );
    }
    // A caller that did not authenticate has no entry.
    put_to(&w, None, PATH, "{}").await;
    assert_eq!(
        w.server
            .calls(100)
            .iter()
            .filter(|e| e.tool == "batch_review")
            .count(),
        4
    );
}

#[tokio::test]
async fn mcp_lists_no_setter_and_a_call_to_it_is_an_unknown_tool() {
    let w = served("agent:alpha,user:bea", Some(Policy::Auto)).await;
    let c = connect(&w.h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    assert!(
        tools
            .iter()
            .all(|t| !t.name.contains("review") || t.name == "design_review"),
        "{:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );
    let params = rmcp::model::CallToolRequestParams::new("batch_review").with_arguments(
        json!({"file": "f", "n": 1, "state": "accepted"})
            .as_object()
            .cloned()
            .unwrap(),
    );
    assert!(c.call_tool(params).await.is_err());
    assert_eq!(header(&w).review.as_str(), "unreviewed");
    // It is in the registry as a Change, so a deployment can deny it and the log has its tier.
    let server = meldframe_server::Server::assemble();
    let entry = server
        .registry()
        .agent_tool("batch_review")
        .expect("registered");
    assert_eq!(entry.tier, meldframe_module::RiskTier::Change);
}
