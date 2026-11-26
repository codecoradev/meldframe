//! `import_html` (E4.4) over MCP against the real server, with a real client: HTML becomes nodes in one atomic call
//! through the gate, a retried key writes nothing twice, a refused call writes nothing, the 100-command bound, a parent
//! that does not exist, the batch with its note, and what the result says about every node.

mod import_html;
use import_html::*;
use serde_json::{Value, json};

#[tokio::test]
async fn html_becomes_a_tree_of_nodes_in_one_call_and_get_tree_shows_it() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", CARD, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    let b = &r.body;
    assert_eq!(
        (
            b["dryRun"].clone(),
            b["replayed"].clone(),
            b["written"].clone()
        ),
        (json!(false), json!(false), b["commands"].clone())
    );
    // The card, its heading, its text and its button with the label inside.
    assert_eq!(b["commands"], 5);
    let rows = b["nodes"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0]["id"], b["root"]);
    assert_eq!(
        (rows[0]["name"].clone(), rows[0]["type"].clone()),
        (json!("card"), json!("frame"))
    );
    assert_eq!(rows[0]["source"], "div#1");
    assert!(rows[1]["id"].as_str().unwrap().starts_with("imp-"));
    let t = tree(&c).await;
    assert_eq!(t["seq"], b["seq"]);
    assert_eq!(t["roots"][0]["id"], b["root"]);
    assert_eq!(t["roots"][0]["childCount"], 3);
    assert_eq!(count(&t), 5);
    let root = node(&c, b["root"].as_str().unwrap()).await;
    assert_eq!(
        (
            root["type"].clone(),
            root["layout"].clone(),
            root["w"].clone(),
            root["fill"].clone()
        ),
        (
            json!("frame"),
            json!("column"),
            json!(332.0),
            json!("#f5f5f5")
        ),
        "300 px of content, 16 px of padding on each side (content-box)"
    );
    assert_eq!(root["parentId"], Value::Null);
}

#[tokio::test]
async fn each_node_says_what_it_could_not_take_and_the_assets_are_listed() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let html = "<div id=a style=\"width:300px\"><p id=p style=\"margin:0;letter-spacing:2px\">Hi</p><img id=pic style=\"width:10px;height:10px\" src=\"https://x.test/a.png\"><div id=d style=\"display:none\">x</div></div>";
    let r = import(&c, "k", html, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    let rows = r.body["nodes"].as_array().unwrap();
    let p = rows.iter().find(|n| n["name"] == "p").unwrap();
    assert_eq!(
        p["unsupported"],
        json!([{"property": "letter-spacing", "value": "2px", "code": "refused_not_modelled"}])
    );
    assert_eq!(r.body["assets"][0]["src"], "https://x.test/a.png");
    assert_eq!(r.body["assets"][0]["use"], "image");
    assert_eq!(
        r.body["skipped"],
        json!([{"source": "div#4", "code": "display_none"}])
    );
    assert!(r.body["report"]["counts"].is_object());
}

#[tokio::test]
async fn the_root_goes_to_the_right_of_the_frames_already_there() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let seed = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "seed", "commands": [frame("old", "Old")]}),
    )
    .await;
    assert!(!seed.is_error, "{}", seed.body);
    let one = import(&c, "k1", CARD, json!({})).await;
    let root = node(&c, one.body["root"].as_str().unwrap()).await;
    assert_eq!(
        (root["x"].clone(), root["y"].clone()),
        (json!(840.0), json!(0.0))
    );
    let two = import(&c, "k2", CARD, json!({})).await;
    assert_ne!(one.body["root"], two.body["root"], "another key, other ids");
    let again = node(&c, two.body["root"].as_str().unwrap()).await;
    assert_eq!(again["x"], json!(840.0 + 332.0 + 40.0));
}

#[tokio::test]
async fn a_retried_key_writes_nothing_twice_and_other_text_is_another_call() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let first = import(&c, "k", CARD, json!({})).await;
    let before = tree(&c).await;
    let again = import(&c, "k", CARD, json!({})).await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(
        (
            again.body["replayed"].clone(),
            again.body["root"].clone(),
            again.body["seq"].clone()
        ),
        (
            json!(true),
            first.body["root"].clone(),
            first.body["seq"].clone()
        )
    );
    let after = tree(&c).await;
    assert_eq!(
        (count(&after), after["seq"].clone()),
        (count(&before), before["seq"].clone())
    );
    // The same key with other text is a new call with its own ids.
    let other = import(
        &c,
        "k",
        "<div style=\"width:50px\"><p>x</p></div>",
        json!({}),
    )
    .await;
    assert!(!other.is_error, "{}", other.body);
    assert_eq!(other.body["replayed"], false);
    assert_ne!(other.body["root"], first.body["root"]);
    assert_eq!(count(&tree(&c).await), 5 + 2);
}

#[tokio::test]
async fn a_refusal_in_the_middle_writes_nothing_and_names_the_element() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let html = format!(
        "<div style=\"width:100px\"><p>fine</p><p id=huge>{}</p></div>",
        "x".repeat(20_001)
    );
    let r = import(&c, "k", &html, json!({})).await;
    assert!(r.is_error, "{}", r.body);
    let e = &r.body["error"];
    assert_eq!(e["code"], "E_TOO_LONG");
    assert_eq!(e["index"], 2);
    assert_eq!(e["source"], "p#3");
    assert_eq!(
        count(&tree(&c).await),
        0,
        "atomic: the first nodes are not written"
    );
}

#[tokio::test]
async fn one_hundred_commands_go_through_and_one_more_is_refused_with_nothing_written() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let ok = import(&c, "k100", &many(100), json!({})).await;
    assert!(!ok.is_error, "{}", ok.body);
    assert_eq!(ok.body["commands"], 100);
    assert_eq!(count(&tree(&c).await), 100);
    let big = import(&c, "k101", &many(101), json!({"parentId": ok.body["root"]})).await;
    assert!(big.is_error, "{}", big.body);
    assert_eq!(big.body["error"]["code"], "E_IMPORT_HTML_TOO_MANY");
    assert!(
        big.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("101")
    );
    assert_eq!(count(&tree(&c).await), 100);
}

#[tokio::test]
async fn a_parent_that_does_not_exist_or_is_not_a_frame_is_refused() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let rect = json!({"type": "addNode", "node": {"id": "r", "type": "rect", "name": "R", "w": 10, "h": 10}});
    for cmds in [vec![frame("p", "Parent")], vec![rect]] {
        let r = call(
            &c,
            "apply_commands",
            json!({"file": "f", "idempotencyKey": format!("seed-{}", cmds[0]["node"]["id"]), "commands": cmds}),
        )
        .await;
        assert!(!r.is_error, "{}", r.body);
    }
    let missing = import(&c, "k1", CARD, json!({"parentId": "nope"})).await;
    assert_eq!(
        missing.body["error"]["code"], "E_PARENT_NOT_FOUND",
        "{}",
        missing.body
    );
    assert_eq!(missing.body["error"]["index"], 0);
    assert_eq!(missing.body["error"]["source"], "div#1");
    let not_frame = import(&c, "k2", CARD, json!({"parentId": "r"})).await;
    assert_eq!(
        not_frame.body["error"]["code"], "E_PARENT_NOT_FRAME",
        "{}",
        not_frame.body
    );
    assert_eq!(count(&tree(&c).await), 2, "nothing was written");
    let ok = import(&c, "k3", CARD, json!({"parentId": "p"})).await;
    assert!(!ok.is_error, "{}", ok.body);
    let root = node(&c, ok.body["root"].as_str().unwrap()).await;
    assert_eq!(
        (
            root["parentId"].clone(),
            root["x"].clone(),
            root["y"].clone()
        ),
        (json!("p"), json!(0.0), json!(0.0))
    );
}

#[tokio::test]
async fn strict_refuses_a_call_with_unsupported_properties_and_lists_them() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let html =
        "<div style=\"width:100px;letter-spacing:2px\"><p style=\"line-height:1.4\">x</p></div>";
    let r = import(&c, "k", html, json!({"strict": true})).await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "E_IMPORT_HTML_UNSUPPORTED");
    let list = r.body["error"]["unsupported"].as_array().unwrap();
    let props: Vec<&str> = list
        .iter()
        .map(|l| l["property"].as_str().unwrap())
        .collect();
    assert_eq!(props, ["letter-spacing", "line-height"]);
    assert_eq!(count(&tree(&c).await), 0);
    let loose = import(&c, "k", html, json!({"strict": false})).await;
    assert!(!loose.is_error, "{}", loose.body);
    // A clean import passes strict.
    let clean = import(
        &c,
        "k2",
        "<div style=\"width:100px\"><p>x</p></div>",
        json!({"strict": true}),
    )
    .await;
    assert!(!clean.is_error, "{}", clean.body);
}

#[tokio::test]
async fn a_dry_run_writes_nothing_needs_no_key_and_shows_what_the_file_would_refuse() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = call(
        &c,
        "import_html",
        json!({"file": "f", "html": CARD, "dryRun": true}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(
        (
            r.body["dryRun"].clone(),
            r.body["written"].clone(),
            r.body["commands"].clone()
        ),
        (json!(true), json!(0), json!(5))
    );
    assert_eq!(r.body["nodes"].as_array().unwrap().len(), 5);
    assert!(r.body.get("seq").is_none() && r.body.get("batch").is_none());
    assert_eq!(count(&tree(&c).await), 0);
    let refused = call(
        &c,
        "import_html",
        json!({"file": "f", "html": CARD, "dryRun": true, "parentId": "nope"}),
    )
    .await;
    assert_eq!(
        refused.body["error"]["code"], "E_PARENT_NOT_FOUND",
        "{}",
        refused.body
    );
    assert_eq!(refused.body["error"]["source"], "div#1");
    // A write without a key is refused, a dry run is not.
    let nokey = call(&c, "import_html", json!({"file": "f", "html": CARD})).await;
    assert_eq!(nokey.body["error"]["code"], "invalid");
    assert_eq!(
        w.h.spy
            .atomic_submits
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn the_policy_decides_and_a_build_without_the_module_serves_nothing() {
    let w = start_with(Opts {
        policy: None,
        ..Opts::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k", CARD, json!({})).await;
    assert_eq!(r.body["error"]["code"], "forbidden");
    assert_eq!(
        w.h.spy
            .atomic_submits
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    let ask = start_with(Opts {
        policy: Some(Policy::Ask),
        ..Opts::default()
    })
    .await;
    let c = connect(&ask.h, AGENT).await;
    assert_eq!(
        import(&c, "k", CARD, json!({})).await.body["error"]["code"],
        "forbidden"
    );
}

#[tokio::test]
async fn a_quota_refuses_the_whole_call_and_nothing_is_written() {
    // 150 ops a minute: the first import of 100 commands lands, the second would be the 200th and is refused whole.
    let w = start_with(Opts {
        quota: Some(150),
        ..Opts::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    let first = import(&c, "k1", &many(100), json!({})).await;
    assert!(!first.is_error, "{}", first.body);
    let r = import(
        &c,
        "k2",
        &many(100),
        json!({"parentId": first.body["root"]}),
    )
    .await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "quota_ops_per_minute");
    assert_eq!(
        count(&tree(&c).await),
        100,
        "the refused call wrote nothing"
    );
    let replay = import(&c, "k1", &many(100), json!({})).await;
    assert_eq!(replay.body["replayed"], true, "a replay costs nothing");
}

#[tokio::test]
async fn the_import_is_one_numbered_batch_with_the_note_of_the_call() {
    let w = start_with(Opts {
        notes: true,
        ..Opts::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k", CARD, json!({"note": "Imported the pricing card"})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "kept"}));
    let list = headers(&w, "ws_1", "f");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].commands, 5);
    assert_eq!(
        list[0].note.as_ref().map(|n| n.as_str()),
        Some("Imported the pricing card")
    );
    // A retry is the same batch.
    let again = import(&c, "k", CARD, json!({"note": "Imported the pricing card"})).await;
    assert_eq!(again.body["replayed"], true);
    assert_eq!(headers(&w, "ws_1", "f").len(), 1);
    // A note that a server that keeps none cannot hold is refused, whole.
    let plain = start().await;
    let pc = connect(&plain.h, AGENT).await;
    let no = import(&pc, "k", CARD, json!({"note": "x"})).await;
    assert_eq!(no.body["error"]["code"], "invalid");
    assert_eq!(count(&tree(&pc).await), 0);
    // And a bad note is refused before anything is read.
    let bad = import(&c, "k2", CARD, json!({"note": "a\u{202e}b"})).await;
    assert_eq!(bad.body["error"]["code"], "invalid");
    assert_eq!(headers(&w, "ws_1", "f").len(), 1);
}
