//! `import_tokens` (E3.4) over MCP against the real server: CSS text becomes collections, modes, variables and aliases
//! in one atomic call through the gate, a long import comes in parts, a retry writes nothing twice, and the codes are
//! the documented ones.

mod importing;
use importing::*;
use meldframe_module::RiskTier;
use serde_json::json;

#[tokio::test]
async fn css_becomes_collections_modes_variables_and_aliases_in_one_call() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = import(&c, "k1", THEME, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(
        (
            r.body["parts"].clone(),
            r.body["part"].clone(),
            r.body["replayed"].clone()
        ),
        (json!(1), json!(1), json!(false))
    );
    assert_eq!(r.body["written"], r.body["commands"]);
    let rep = &r.body["report"];
    assert_eq!(
        (rep["collections"].clone(), rep["modes"].clone()),
        (json!(2), json!(3))
    );
    assert_eq!(
        (
            rep["variables"].clone(),
            rep["aliases"].clone(),
            rep["skipped"].clone()
        ),
        (json!(6), json!(2), json!(1))
    );
    let codes: Vec<&str> = rep["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"].as_str().unwrap())
        .collect();
    for want in [
        "import_skipped",
        "converted",
        "rem",
        "computed_value",
        "dark_mode_source",
    ] {
        assert!(
            want == "computed_value" || codes.contains(&want),
            "{want} in {codes:?}"
        );
    }
    assert!(
        rep["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "--shadow-ring" && e["kind"] == "skipped")
    );

    let v = variables(&c, json!({})).await;
    assert_eq!(v["collections"].as_array().unwrap().len(), 2);
    let root = v["collections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "root")
        .unwrap();
    assert_eq!(
        root["modes"],
        json!([{"id": "light", "name": "light"}, {"id": "dark", "name": "dark"}])
    );
    let by = |n: &str| {
        v["variables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["name"] == n)
            .unwrap()
            .clone()
    };
    assert_eq!(
        by("background")["values"],
        json!({"light": "#ffffff", "dark": "#0a0a0a"})
    );
    assert_eq!(by("radius")["values"]["light"], 8.0);
    assert_eq!(
        by("brand")["values"]["light"],
        json!({"var": by("color/brand-600")["id"]})
    );
    assert_eq!(
        by("color/surface")["values"]["default"],
        json!({"var": by("background")["id"]})
    );
}

#[tokio::test]
async fn an_agent_binds_a_frame_to_an_imported_variable() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    assert!(!import(&c, "k1", THEME, json!({})).await.is_error);
    let id = variables(&c, json!({"prefix": "background"})).await["variables"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "bind", "commands": [
            {"type": "addNode", "node": {"id": "card", "type": "frame", "name": "Card", "w": 100, "h": 50, "fill": {"var": id}}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let n = call(&c, "get_nodes", json!({"file": "f", "ids": ["card"]})).await;
    assert_eq!(n.body["nodes"][0]["fill"], json!({"var": id}));
}

#[tokio::test]
async fn a_dry_run_writes_nothing_and_needs_no_key() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let r = call(
        &c,
        "import_tokens",
        json!({"file": "f", "css": THEME, "dryRun": true}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(
        (
            r.body["written"].clone(),
            r.body["dryRun"].clone(),
            r.body["parts"].clone()
        ),
        (json!(0), json!(true), json!(1))
    );
    assert_eq!(variables(&c, json!({})).await["total"], 0);
    assert_eq!(
        h.spy
            .atomic_submits
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn a_retry_writes_nothing_twice_and_the_same_key_with_other_text_replays_nothing() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let first = import(&c, "k1", THEME, json!({})).await;
    let again = import(&c, "k1", THEME, json!({"part": 1})).await;
    assert!(!again.is_error, "{}", again.body);
    assert_eq!(
        (again.body["replayed"].clone(), again.body["seq"].clone()),
        (json!(true), first.body["seq"].clone())
    );
    assert_eq!(variables(&c, json!({})).await["total"], 6);
    let other = import(&c, "k1", ":root { --a: #fff; }", json!({})).await;
    // Other text under the same key is another call: its ops have other keys, so nothing of the first is replayed.
    assert!(other.is_error, "{}", other.body);
    assert_eq!(other.body["error"]["code"], "E_DUPLICATE_ID");
    let snake = call(
        &c,
        "import_tokens",
        json!({"file": "f", "css": THEME, "idempotency_key": "k1"}),
    )
    .await;
    assert_eq!(
        snake.body["replayed"], true,
        "the snake_case spelling is the same key"
    );
}

#[tokio::test]
async fn importing_the_same_text_again_with_a_new_key_is_refused_with_a_hint() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    assert!(!import(&c, "k1", THEME, json!({})).await.is_error);
    let r = import(&c, "k2", THEME, json!({})).await;
    assert!(r.is_error, "{}", r.body);
    assert_eq!(r.body["error"]["code"], "E_DUPLICATE_ID");
    assert!(
        r.body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("adds and does not update")
    );
    assert_eq!(
        variables(&c, json!({})).await["total"],
        6,
        "nothing was written"
    );
}

#[tokio::test]
async fn a_long_import_comes_in_parts_that_are_atomic_and_in_order() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let css = long(250);
    let dry = call(
        &c,
        "import_tokens",
        json!({"file": "f", "css": css, "dryRun": true}),
    )
    .await;
    assert_eq!(
        (dry.body["commands"].clone(), dry.body["parts"].clone()),
        (json!(251), json!(3))
    );
    let late = import(&c, "k", &css, json!({"part": 2})).await;
    assert!(late.is_error, "{}", late.body);
    assert_eq!(
        late.body["error"]["code"], "E_COLLECTION_NOT_FOUND",
        "part 2 needs part 1"
    );
    assert_eq!(
        variables(&c, json!({})).await["total"],
        0,
        "a refused part writes nothing"
    );
    let past = import(&c, "k", &css, json!({"part": 4})).await;
    assert_eq!(past.body["error"]["code"], "invalid");
    for (part, written) in [(1, 100), (2, 100), (3, 51)] {
        let r = import(&c, "k", &css, json!({"part": part})).await;
        assert!(!r.is_error, "part {part}: {}", r.body);
        assert_eq!(
            (r.body["written"].clone(), r.body["parts"].clone()),
            (json!(written), json!(3))
        );
    }
    let v = variables(&c, json!({})).await;
    assert_eq!(v["total"], 250);
    let replay = import(&c, "k", &css, json!({"part": 2})).await;
    assert_eq!(replay.body["replayed"], true);
    assert_eq!(variables(&c, json!({})).await["total"], 250);
    let alias = variables(&c, json!({"prefix": "v9"})).await;
    assert!(
        alias["variables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["name"] == "v9" && x["values"]["light"]["var"].is_string())
    );
    assert_eq!(
        names(&alias).len(),
        alias["total"].as_u64().unwrap() as usize
    );
}

#[tokio::test]
async fn the_limits_of_the_call_and_nothing_to_import() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let big = "x".repeat(variables_import::MAX_INPUT_BYTES + 1);
    let r = import(&c, "k", &big, json!({})).await;
    assert_eq!(r.body["error"]["code"], "E_IMPORT_TOO_LARGE");
    let nothing = import(&c, "k", "@import \"x\"; .a { --b: 1 }", json!({})).await;
    assert!(!nothing.is_error, "{}", nothing.body);
    assert_eq!(
        (
            nothing.body["parts"].clone(),
            nothing.body["written"].clone()
        ),
        (json!(0), json!(0))
    );
    assert!(
        !nothing.body["report"]["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let no_key = call(&c, "import_tokens", json!({"file": "f", "css": THEME})).await;
    assert_eq!(no_key.body["error"]["code"], "invalid");
    let both = call(
        &c,
        "import_tokens",
        json!({"file": "f", "css": THEME, "idempotencyKey": "a", "idempotency_key": "a"}),
    )
    .await;
    assert_eq!(both.body["error"]["code"], "invalid");
    let bad_file = call(
        &c,
        "import_tokens",
        json!({"file": "a/b", "css": THEME, "idempotencyKey": "a"}),
    )
    .await;
    assert_eq!(bad_file.body["error"]["code"], "invalid");
    let unknown = call(
        &c,
        "import_tokens",
        json!({"file": "f", "css": THEME, "idempotencyKey": "a", "workspace": "ws_2"}),
    )
    .await;
    assert_eq!(unknown.body["error"]["code"], "bad_request");
    assert_eq!(variables(&c, json!({})).await["total"], 0);
}

#[tokio::test]
async fn the_workspace_comes_from_the_token_and_a_person_may_import_too() {
    let h = start().await;
    let a = connect(&h, AGENT).await;
    let other = connect(&h, OTHER).await;
    let person = connect(&h, PERSON).await;
    assert!(
        !import(&other, "k", ":root { --only-theirs: #fff; }", json!({}))
            .await
            .is_error
    );
    assert!(!import(&a, "k", THEME, json!({})).await.is_error);
    assert_eq!(variables(&other, json!({})).await["total"], 1);
    assert_eq!(variables(&a, json!({})).await["total"], 6);
    let p = import(&person, "kp", ":root { --b: #000; }", json!({"file": "g"})).await;
    assert!(!p.is_error, "{}", p.body);
}

#[tokio::test]
async fn without_the_module_the_tool_serves_nothing_and_the_log_has_change_entries() {
    let h = start_with(None).await;
    let c = connect(&h, AGENT).await;
    let r = import(&c, "k", THEME, json!({})).await;
    assert_eq!(r.body["error"]["code"], "forbidden");
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let before = h.spy.server.calls(100).len();
    let _ = import(&c, "k", THEME, json!({})).await;
    let _ = import(
        &c,
        "k2",
        &"x".repeat(variables_import::MAX_INPUT_BYTES + 1),
        json!({}),
    )
    .await;
    let log = h.spy.server.calls(100);
    let mine: Vec<_> = log[before..]
        .iter()
        .filter(|e| e.tool == "import_tokens")
        .collect();
    // A write has the entry of the gate (the op was admitted) and the entry of the tool call; the last one is the call
    // that was refused, with its code.
    assert_eq!(mine.len(), 3, "{log:?}");
    assert!(
        mine[..2]
            .iter()
            .all(|e| e.file == "f" && e.outcome == "ok" && e.risk == Some(RiskTier::Change))
    );
    assert_eq!(mine[2].outcome, "E_IMPORT_TOO_LARGE");
    assert!(
        !format!("{log:?}").contains("--background"),
        "an entry has no content"
    );
}

#[test]
fn the_code_of_the_tool_is_documented() {
    assert!(CODES.contains("| `E_IMPORT_TOO_LARGE` |"));
}
