//! `import_html` over MCP against the real server: a page becomes nodes in one atomic call, with the answer the note
//! describes (`docs/import-html.md` section 4.2), placed as the call says, and written through the same gate as
//! `apply_commands`.

mod common;
mod harness;
use harness::*;
use serde_json::{Value, json};

#[tokio::test]
async fn a_page_becomes_nodes_in_one_call_and_the_answer_says_what_was_written() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", HERO, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    let b = &r.body;
    assert_eq!(
        (
            b["dryRun"].clone(),
            b["replayed"].clone(),
            b["commands"].clone()
        ),
        (json!(false), json!(false), json!(3))
    );
    assert!(b["seq"].as_u64().unwrap() >= 3, "{b}");
    let ids = ids(&r);
    assert_eq!(b["root"], json!(ids[0]));
    let types: Vec<&Value> = b["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| &n["type"])
        .collect();
    assert_eq!(types, [&json!("frame"), &json!("text"), &json!("text")]);
    let names: Vec<&str> = b["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(names[0], "Hero section");
    assert!(
        b["nodes"][0]["source"]
            .as_str()
            .unwrap()
            .starts_with("section"),
        "{b}"
    );
    for n in b["nodes"].as_array().unwrap() {
        assert!(
            n["approximated"].is_array() && n["unsupported"].is_array(),
            "{n}"
        );
    }
    assert_eq!(b["batch"], json!({"n": 1, "note": "none"}));
    assert_eq!(b["idempotencyKey"], json!("k1"));

    // The file holds what the answer lists, under the root, in the order of the text.
    let got = nodes(&c, &ids.iter().map(String::as_str).collect::<Vec<_>>()).await;
    assert_eq!(got.len(), 3);
    let by = |id: &str| got.iter().find(|n| n["id"] == id).unwrap();
    let (root, h1, p) = (by(&ids[0]), by(&ids[1]), by(&ids[2]));
    assert_eq!(root["fill"], json!("#112233"));
    assert_eq!(
        (root["gap"].as_f64(), root["padding"].as_f64()),
        (Some(8.0), Some(16.0))
    );
    assert_eq!(
        (h1["text"].clone(), p["text"].clone()),
        (json!("Hello"), json!("World"))
    );
    assert_eq!(
        (h1["parentId"].clone(), p["parentId"].clone()),
        (json!(ids[0]), json!(ids[0]))
    );
    assert!(root["parentId"].is_null());
    assert!(
        h1["pos"].as_str().unwrap() < p["pos"].as_str().unwrap(),
        "source order"
    );
    assert_eq!(
        (root["x"].clone(), root["y"].clone()),
        (json!(0.0), json!(0.0))
    );
    assert_eq!(head(&c).await.1, 1, "one top-level frame");
}

#[tokio::test]
async fn the_root_goes_to_the_right_of_the_others_and_never_on_one() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    // A child that sticks out to the right does not count: only the top-level frames do.
    let seed = vec![
        json!({"type": "addNode", "node": {"id": "a", "type": "frame", "name": "A", "x": 10, "y": 5, "w": 800, "h": 400}}),
        json!({"type": "addNode", "node": {"id": "b", "type": "rect", "name": "B", "parentId": "a", "x": 700, "y": 0, "w": 500, "h": 50}}),
    ];
    apply(&c, "seed", seed).await;
    let first = import(&c, "k1", HERO, json!({"width": 500})).await;
    let root1 = nodes(&c, &[first.body["root"].as_str().unwrap()])
        .await
        .remove(0);
    assert_eq!(
        root1["x"],
        json!(850.0),
        "40 px past the right edge of the frame"
    );
    let second = import(&c, "k2", HERO, json!({"width": 500})).await;
    let root2 = nodes(&c, &[second.body["root"].as_str().unwrap()])
        .await
        .remove(0);
    let right = root1["x"].as_f64().unwrap() + root1["w"].as_f64().unwrap();
    assert_eq!(root2["x"].as_f64().unwrap(), right + 40.0);
    assert_eq!(
        second.body["batch"]["n"],
        json!(3),
        "the seed is batch 1 and the first import batch 2: one numbering for every writer"
    );
}

#[tokio::test]
async fn a_parent_id_puts_the_page_at_the_origin_of_a_frame_of_the_file() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let seed = vec![
        json!({"type": "addNode", "node": {"id": "p", "type": "frame", "name": "Page", "w": 900, "h": 600}}),
        json!({"type": "addNode", "node": {"id": "t", "type": "text", "name": "Label", "parentId": "p", "text": "x"}}),
    ];
    apply(&c, "seed", seed).await;
    let r = import(&c, "k1", HERO, json!({"parentId": "p"})).await;
    assert!(!r.is_error, "{}", r.body);
    let root = nodes(&c, &[r.body["root"].as_str().unwrap()])
        .await
        .remove(0);
    assert_eq!(
        (
            root["parentId"].clone(),
            root["x"].clone(),
            root["y"].clone()
        ),
        (json!("p"), json!(0.0), json!(0.0))
    );
    assert_eq!(head(&c).await.1, 1, "no new top-level frame");

    let before = head(&c).await;
    for (parent, want) in [("nope", "E_PARENT_NOT_FOUND"), ("t", "E_PARENT_NOT_FRAME")] {
        let r = import(&c, "k2", HERO, json!({"parentId": parent})).await;
        assert!(r.is_error, "{}", r.body);
        let e = &r.body["error"];
        assert_eq!(
            (e["code"].clone(), e["index"].clone()),
            (json!(want), json!(0)),
            "{e}"
        );
        assert!(
            e["source"].as_str().unwrap().starts_with("section"),
            "the element it came from: {e}"
        );
    }
    assert_eq!(head(&c).await, before, "a refused call writes nothing");
}

#[tokio::test]
async fn a_command_the_file_refuses_names_its_index_and_source_and_nothing_is_written() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    // A chain of 32 frames: the deepest is at depth 31, so a root under it is at 32 and its child at 33, over the limit.
    let chain: Vec<Value> = (0..32)
        .map(|i| {
            let mut n = json!({"id": format!("d{i}"), "type": "frame", "name": format!("Level {i}"), "w": 100, "h": 100});
            if i > 0 {
                n["parentId"] = json!(format!("d{}", i - 1));
            }
            json!({"type": "addNode", "node": n})
        })
        .collect();
    apply(&c, "chain", chain).await;
    let before = head(&c).await;
    let html = "<body style='margin:0'><div data-name='a' style='background:#eee;padding:4px'>\
        <div data-name='b' style='background:#ddd;padding:4px'><div data-name='c' style='background:#ccc;padding:4px'>x</div></div></div></body>";
    let r = import(&c, "k1", html, json!({"parentId": "d31"})).await;
    assert!(r.is_error, "{}", r.body);
    let e = &r.body["error"];
    assert_eq!(e["code"], json!("E_TOO_DEEP"), "{e}");
    assert_eq!(
        e["index"],
        json!(1),
        "the second command is the first that is too deep: {e}"
    );
    assert!(e["source"].as_str().unwrap().starts_with("div"), "{e}");
    assert_eq!(e["retryable"], json!(false));
    assert_eq!(head(&c).await, before);
}

#[tokio::test]
async fn a_note_is_kept_on_the_batch_and_the_batch_numbers_run_on() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", HERO, json!({"note": "pull in the hero"})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "kept"}));
    let page = w
        .store
        .list(
            "ws_1",
            "f",
            &Query {
                before: None,
                after: None,
                limit: 10,
            },
        )
        .unwrap();
    let h = &page.headers[0];
    assert_eq!((h.n, h.commands, h.written, h.atomic), (1, 3, 3, true));
    assert_eq!(h.note.as_ref().unwrap().as_str(), "pull in the hero");
    let digest = h
        .key_prefix
        .strip_prefix("k1:")
        .expect("the batch is the keys `{key}:{digest}`");
    assert!(
        digest.len() == 12 && digest.chars().all(|c| c.is_ascii_hexdigit()),
        "{digest}"
    );
    // The ops are named `{key}:{digest}:{i}`: that prefix is what batch_status and a revert find the batch by.
    let status = call(
        &c,
        "batch_status",
        json!({"file": "f", "idempotencyKey": h.key_prefix, "count": 3}),
    )
    .await;
    assert_eq!(
        (status.body["state"].clone(), status.body["applied"].clone()),
        (json!("applied"), json!(3))
    );
    let other = import(&c, "k2", HERO, json!({})).await;
    assert_eq!(other.body["batch"], json!({"n": 2, "note": "none"}));
}

#[tokio::test]
async fn a_server_that_keeps_no_notes_refuses_a_note_and_writes_nothing() {
    let w = start_with(Setup {
        notes: false,
        ..Setup::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", HERO, json!({"note": "x"})).await;
    assert!(r.is_error);
    assert_eq!(r.body["error"]["code"], json!("invalid"));
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("keeps no notes"),
        "{}",
        r.body
    );
    assert_eq!(head(&c).await.1, 0);
    let ok = import(&c, "k2", HERO, json!({})).await;
    assert!(!ok.is_error, "{}", ok.body);
    assert_eq!(ok.body["batch"], Value::Null);
}

#[tokio::test]
async fn a_tool_that_is_not_under_the_auto_policy_serves_nothing() {
    for policy in [None, Some(Policy::Ask), Some(Policy::Deny)] {
        let w = start_with(Setup {
            policy,
            ..Setup::default()
        })
        .await;
        let c = connect(&w.h, AGENT).await;
        let r = import(&c, "k1", HERO, json!({})).await;
        assert!(r.is_error);
        assert_eq!(r.body["error"]["code"], json!("forbidden"), "{policy:?}");
        assert_eq!(head(&c).await.1, 0);
    }
}

#[tokio::test]
async fn the_gate_counts_every_command_against_the_quota_and_a_refused_call_writes_nothing() {
    // Room for one import of three commands, not for two.
    let w = start_with(Setup {
        quota: Some(4),
        ..Setup::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    assert!(!import(&c, "k1", HERO, json!({})).await.is_error);
    let r = import(&c, "k2", HERO, json!({})).await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(
        r.body["error"]["code"],
        json!("quota_ops_per_minute"),
        "{}",
        r.body
    );
    assert_eq!(head(&c).await.1, 1, "the refused call wrote nothing");
}

#[tokio::test]
async fn a_workspace_sees_only_its_own_file() {
    let w = start().await;
    let (a, o) = (connect(&w.h, AGENT).await, connect(&w.h, OTHER).await);
    assert!(!import(&a, "k1", HERO, json!({})).await.is_error);
    assert_eq!(
        head(&o).await.1,
        0,
        "the same file name in another workspace is another file"
    );
    let r = import(&o, "k1", HERO, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["batch"]["n"], json!(1));
}
