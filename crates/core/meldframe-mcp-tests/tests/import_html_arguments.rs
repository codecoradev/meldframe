//! `import_html` over MCP, the other half: the arguments are checked before anything is read or written, the codes of
//! the importer are the documented ones, hostile text stays text, the call log keeps no content, and the tool is
//! listed with its text and schema.

mod import_html;
use import_html::*;
use serde_json::json;

#[tokio::test]
async fn the_arguments_are_checked_before_anything_is_read_or_written() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let cases = [
        (
            json!({"html": CARD, "idempotencyKey": "k", "idempotency_key": "k"}),
            "invalid",
        ),
        (
            json!({"html": CARD, "idempotencyKey": "k", "ifSeq": 3}),
            "bad_request",
        ),
        (
            json!({"html": CARD, "idempotencyKey": "k", "width": 0}),
            "invalid",
        ),
        (
            json!({"html": CARD, "idempotencyKey": "k", "width": 1.0e9}),
            "invalid",
        ),
        (
            json!({"html": CARD, "idempotencyKey": "k", "name": "a\u{202e}b"}),
            "invalid",
        ),
        (
            json!({"html": CARD, "idempotencyKey": "k\u{200b}"}),
            "invalid",
        ),
        (
            json!({"html": CARD, "idempotencyKey": "k", "unknown": 1}),
            "bad_request",
        ),
        (json!({"html": 7, "idempotencyKey": "k"}), "bad_request"),
    ];
    for (args, code) in cases {
        let mut a = json!({"file": "f"});
        a.as_object_mut()
            .unwrap()
            .extend(args.as_object().unwrap().clone());
        let r = call(&c, "import_html", a.clone()).await;
        assert_eq!(r.body["error"]["code"], code, "{a}: {}", r.body);
    }
    let bad_file = call(
        &c,
        "import_html",
        json!({"file": "a/b", "html": CARD, "idempotencyKey": "k"}),
    )
    .await;
    assert_eq!(bad_file.body["error"]["code"], "invalid");
    assert_eq!(
        w.h.spy
            .atomic_submits
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(count(&tree(&c).await), 0);
}

#[tokio::test]
async fn the_codes_of_the_importer_are_the_documented_ones() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let big = format!("<div>{}</div>", "a".repeat(262_144));
    let many_elements = "<i></i>".repeat(5_001);
    for (html, code) in [
        (big.as_str(), "E_IMPORT_HTML_TOO_LARGE"),
        (many_elements.as_str(), "E_IMPORT_HTML_TOO_MANY_ELEMENTS"),
        ("<script>1</script>", "E_IMPORT_HTML_EMPTY"),
        ("just words", "E_IMPORT_HTML_EMPTY"),
    ] {
        let r = import(&c, "k", html, json!({})).await;
        assert_eq!(
            r.body["error"]["code"],
            code,
            "{}",
            &r.body.to_string()[..200.min(r.body.to_string().len())]
        );
        assert_eq!(r.body["error"]["retryable"], false);
        assert!(
            CODES.contains(&format!("| `{code}` |")),
            "{code} has no row in docs/error-codes.md"
        );
    }
    for code in ["E_IMPORT_HTML_TOO_MANY", "E_IMPORT_HTML_UNSUPPORTED"] {
        assert!(CODES.contains(&format!("| `{code}` |")), "{code}");
    }
    assert_eq!(count(&tree(&c).await), 0);
}

#[tokio::test]
async fn nothing_is_run_or_followed_and_hostile_text_stays_text() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let html = "<div style=\"width:300px\"><script>fetch('https://evil.test')</script><link rel=stylesheet href=\"https://evil.test/a.css\"><img id=i style=\"width:5px;height:5px\" src=\"javascript:alert(1)\"><p style=\"margin:0\">a\u{202e}b\u{200b}c</p><div data-name=\"N\u{202e}ame\" style=\"height:3px\"></div></div>";
    let r = import(&c, "k", html, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    assert_eq!(r.body["assets"][0]["src"], "javascript:alert(1)");
    let rows = r.body["nodes"].as_array().unwrap();
    let text = rows.iter().find(|n| n["type"] == "text").unwrap();
    // The text of a node is file content, as written; a name the importer made is clean.
    let stored = node(&c, text["id"].as_str().unwrap()).await;
    assert_eq!(stored["text"], "a\u{202e}b\u{200b}c");
    assert!(rows.iter().any(|n| n["name"] == "Name"), "{rows:?}");
    let dump = r.body.to_string();
    assert!(
        !dump.contains("evil.test") && !dump.contains('\u{202e}'),
        "{dump}"
    );
}

#[tokio::test]
async fn the_call_log_has_the_tool_and_no_content() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let before = w.h.spy.server.calls(100).len();
    let _ = import(&c, "k", CARD, json!({})).await;
    let _ = import(&c, "k2", &many(101), json!({})).await;
    let log = w.h.spy.server.calls(100);
    let mine: Vec<_> = log[before..]
        .iter()
        .filter(|e| e.tool == "import_html")
        .collect();
    // A write is logged twice, the submit of the gate (the op was admitted) and the call of the tool; a refused call once,
    // by the tool: so two calls, one that wrote and one that was refused, make three entries.
    assert_eq!(mine.len(), 3, "{log:?}");
    assert!(mine[..2].iter().all(|e| e.file == "f" && e.outcome == "ok"));
    assert_eq!(mine[2].outcome, "E_IMPORT_HTML_TOO_MANY");
    assert!(
        !format!("{log:?}").contains("Some text"),
        "an entry has no content"
    );
}

#[tokio::test]
async fn the_tool_is_listed_with_its_text_and_schema() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let t = tools.iter().find(|t| t.name == "import_html").unwrap();
    let d = t.description.as_deref().unwrap();
    for needle in [
        "262144",
        "100 commands",
        "E_IMPORT_HTML_TOO_MANY",
        "idempotencyKey",
        "strict",
        "dryRun",
        "ifSeq",
        "parentId",
    ] {
        assert!(d.contains(needle), "{needle}");
    }
    assert!(d.is_ascii());
    let props = t.input_schema["properties"].as_object().unwrap();
    for k in [
        "file",
        "html",
        "css",
        "parentId",
        "name",
        "width",
        "idempotencyKey",
        "idempotency_key",
        "note",
        "dryRun",
        "strict",
    ] {
        assert!(props.contains_key(k), "{k}");
    }
    assert!(!props.contains_key("ifSeq"));
    assert_eq!(
        t.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(false)
    );
}

#[tokio::test]
async fn a_parent_id_that_cannot_be_an_id_is_not_found_by_the_gate() {
    // The schema has a pattern for it and the file refuses what is not one of its nodes: no check of the tool's own.
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k", CARD, json!({"parentId": "a b"})).await;
    assert_eq!(r.body["error"]["code"], "E_PARENT_NOT_FOUND", "{}", r.body);
    assert_eq!(count(&tree(&c).await), 0);
}
