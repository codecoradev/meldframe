//! `import_html` over MCP: the limits at their edges, who may collide with whom (two actors, two workspaces), a dry
//! run beside the call that writes, and what the tool text does not promise.

mod import_html;
use import_html::*;
use serde_json::json;

#[tokio::test]
async fn a_key_of_the_longest_length_still_has_its_batch_and_the_next_length_is_refused() {
    // The batch header holds the key and the digest (13 characters) as its key prefix, which is at most 100.
    let w = start_with(Opts {
        notes: true,
        ..Opts::default()
    })
    .await;
    let c = connect(&w.h, AGENT).await;
    let longest = "k".repeat(87);
    let r = import(&c, &longest, CARD, json!({"note": "the longest key"})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["batch"], json!({"n": 1, "note": "kept"}));
    assert_eq!(headers(&w, "ws_1", "f").len(), 1);
    for key in ["k".repeat(88), "k".repeat(100), "\u{e9}".repeat(44)] {
        let r = import(&c, &key, CARD, json!({"note": "too long"})).await;
        assert_eq!(r.body["error"]["code"], "invalid", "{}", key.len());
        assert!(r.body["error"]["message"].as_str().unwrap().contains("87"));
    }
    assert_eq!(
        headers(&w, "ws_1", "f").len(),
        1,
        "nothing was written by a refused key"
    );
}

#[tokio::test]
async fn two_actors_of_one_workspace_with_the_same_key_and_html_each_get_their_own_nodes() {
    let w = start().await;
    let (a, b) = (connect(&w.h, AGENT).await, connect(&w.h, PERSON).await);
    let one = import(&a, "same", CARD, json!({})).await;
    let two = import(&b, "same", CARD, json!({})).await;
    assert!(!one.is_error && !two.is_error, "{} {}", one.body, two.body);
    assert_ne!(one.body["root"], two.body["root"]);
    assert_eq!(count(&tree(&a).await), 10);
}

#[tokio::test]
async fn another_workspace_with_the_same_key_text_and_file_name_does_not_touch_the_first() {
    let w = start().await;
    let (a, o) = (connect(&w.h, AGENT).await, connect(&w.h, OTHER).await);
    let one = import(&a, "same", CARD, json!({})).await;
    let before = tree(&a).await;
    let two = import(&o, "same", CARD, json!({})).await;
    assert!(!two.is_error, "{}", two.body);
    assert_eq!(two.body["replayed"], false);
    assert_eq!(count(&tree(&a).await), count(&before));
    assert_eq!(tree(&a).await["seq"], one.body["seq"]);
    assert_eq!(count(&tree(&o).await), 5);
}

#[tokio::test]
async fn a_parent_of_another_workspace_is_not_found_for_a_call_and_for_a_dry_run() {
    let w = start().await;
    let (a, o) = (connect(&w.h, AGENT).await, connect(&w.h, OTHER).await);
    let seed = call(
        &a,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "s", "commands": [frame("theirs", "Theirs")]}),
    )
    .await;
    assert!(!seed.is_error, "{}", seed.body);
    let real = import(&o, "k", CARD, json!({"parentId": "theirs"})).await;
    assert_eq!(
        real.body["error"]["code"], "E_PARENT_NOT_FOUND",
        "{}",
        real.body
    );
    let dry = call(
        &o,
        "import_html",
        json!({"file": "f", "html": CARD, "dryRun": true, "parentId": "theirs"}),
    )
    .await;
    assert_eq!(
        dry.body["error"]["code"], "E_PARENT_NOT_FOUND",
        "{}",
        dry.body
    );
    // A dry run with no parent places the root in the file of its own workspace: nothing there, so at 0.
    let own = call(
        &o,
        "import_html",
        json!({"file": "f", "html": CARD, "dryRun": true}),
    )
    .await;
    assert!(!own.is_error, "{}", own.body);
    assert_eq!(count(&tree(&a).await), 1);
    assert_eq!(count(&tree(&o).await), 0);
}

#[tokio::test]
async fn the_input_cap_is_256_kib_of_html_and_css_together_counted_in_bytes() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let base = "<div style=\"width:10px\"><p>x</p></div><!--";
    let pad = |n: usize| format!("{base}{}-->", "a".repeat(n - base.len() - 3));
    let exact = pad(262_144);
    assert_eq!(exact.len(), 262_144);
    let r = import(&c, "k1", &exact, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    let over = pad(262_145);
    let r = import(&c, "k2", &over, json!({})).await;
    assert_eq!(r.body["error"]["code"], "E_IMPORT_HTML_TOO_LARGE");
    // The css counts: the same html with one byte of css is over.
    let r = import(&c, "k3", &exact, json!({"css": "a"})).await;
    assert_eq!(r.body["error"]["code"], "E_IMPORT_HTML_TOO_LARGE");
}

#[tokio::test]
async fn a_dry_run_with_a_key_makes_the_ids_of_the_call_that_writes_and_one_after_it_is_a_replay() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let dry = call(
        &c,
        "import_html",
        json!({"file": "f", "html": CARD, "dryRun": true, "idempotencyKey": "k"}),
    )
    .await;
    let real = import(&c, "k", CARD, json!({})).await;
    assert_eq!(dry.body["root"], real.body["root"]);
    assert_eq!(dry.body["nodes"], real.body["nodes"]);
    let after = call(
        &c,
        "import_html",
        json!({"file": "f", "html": CARD, "dryRun": true, "idempotencyKey": "k"}),
    )
    .await;
    assert!(!after.is_error, "{}", after.body);
    assert_eq!(
        (after.body["dryRun"].clone(), after.body["replayed"].clone()),
        (json!(true), json!(true))
    );
    // Without a key the ids come from the key `dry-run`, so they are not those of a call that writes.
    let keyless = call(
        &c,
        "import_html",
        json!({"file": "f", "html": CARD, "dryRun": true}),
    )
    .await;
    assert_ne!(keyless.body["root"], real.body["root"]);
}

#[tokio::test]
async fn a_number_the_file_refuses_is_refused_by_the_gate_naming_the_element() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(
        &c,
        "k",
        "<div style=\"width:300px\"><div id=big style=\"width:200000px;height:5px\"></div></div>",
        json!({}),
    )
    .await;
    assert_eq!(r.body["error"]["code"], "E_OUT_OF_RANGE", "{}", r.body);
    assert_eq!(r.body["error"]["source"], "div#2");
    assert_eq!(count(&tree(&c).await), 0);
}

#[tokio::test]
async fn one_hug_text_is_named_in_the_estimate_warning_in_the_singular() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(
        &c,
        "k",
        "<div style=\"width:300px\"><p style=\"margin:0;white-space:nowrap\">one line</p></div>",
        json!({}),
    )
    .await;
    let warnings = r.body["warnings"].as_array().unwrap();
    let est = warnings
        .iter()
        .find(|x| x["code"] == "text_size_estimated")
        .unwrap();
    assert!(
        est["message"]
            .as_str()
            .unwrap()
            .starts_with("1 text box gets its size"),
        "{est}"
    );
}

#[tokio::test]
async fn the_tool_text_does_not_promise_a_whole_revert() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let t = tools.iter().find(|t| t.name == "import_html").unwrap();
    let d = t.description.as_deref().unwrap();
    assert!(!d.contains("revert it in one step"));
    assert!(
        !d.contains("empty root frame may stay"),
        "every frame the import made stays, not only the root"
    );
    assert!(
        d.contains("its texts and shapes are reverted, and the frames it made stay, empty"),
        "a revert takes the texts and shapes and leaves every frame"
    );
    assert!(d.contains("87 characters"), "the key is at most 87");
    assert!(
        d.contains("dry-run"),
        "a dry run with no key makes other ids"
    );
}

#[tokio::test]
async fn strict_counts_the_properties_a_full_list_left_out_and_the_row_says_how_many() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let props: String = (0..50).map(|i| format!("zz-prop-{i}:1;")).collect();
    let html = format!(
        "<style>body{{margin:0}}</style><div style=\"width:300px\"><div id=a style=\"width:5px;height:5px;{props}\"></div></div>"
    );
    let strict = import(&c, "k", &html, json!({"strict": true})).await;
    assert_eq!(strict.body["error"]["code"], "E_IMPORT_HTML_UNSUPPORTED");
    assert!(
        strict.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("50 properties"),
        "{}",
        strict.body
    );
    let loose = import(&c, "k2", &html, json!({})).await;
    let row = loose.body["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "a")
        .unwrap()
        .clone();
    assert_eq!(row["unsupported"].as_array().unwrap().len(), 32);
    assert_eq!(
        row["omitted"], 18,
        "the lines left out of the two lists of the row"
    );
}
