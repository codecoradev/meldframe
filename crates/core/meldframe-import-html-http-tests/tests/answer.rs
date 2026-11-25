//! What the answer of `import_html` says about what was not carried: a refused property is on its node and in the
//! report, an image is listed and not fetched, what makes no node is listed, and the limits and `strict` write
//! nothing (`docs/import-html.md` sections 2.3 and 4.2).

mod common;
mod harness;
use harness::*;
use serde_json::{Value, json};

const RICH: &str = "<body style='margin:0'><div style='width:300px;background:#eee'>\
    <p style='line-height:2'>Hi there</p><img src='https://x.test/a.png' width=40 height=30>\
    <div style='display:none'>gone</div><input type=checkbox><span class=hero>x</span></div></body>";

fn node_of<'a>(r: &'a Called, source_starts: &str) -> &'a Value {
    let list = r.body["nodes"].as_array().unwrap();
    list.iter()
        .find(|n| n["source"].as_str().unwrap().starts_with(source_starts))
        .unwrap_or_else(|| panic!("a node from {source_starts}: {}", r.body))
}

#[tokio::test]
async fn a_refused_property_is_on_its_node_and_in_the_report() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", RICH, json!({})).await;
    assert!(!r.is_error, "{}", r.body);
    let p = node_of(&r, "p");
    let lost = p["unsupported"].as_array().unwrap();
    assert!(
        lost.iter().any(|u| u["property"] == "line-height"
            && u["code"] == "refused_not_modelled"
            && u["value"] == "2"),
        "{p}"
    );
    let entries = r.body["report"]["entries"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|e| e["kind"] == "refused" && e["name"] == "line-height"),
        "{}",
        r.body["report"]
    );
    assert_eq!(r.body["report"]["omitted"], json!(0));
    assert!(
        r.body["report"]["counts"]["display_none"].as_u64().unwrap() >= 1,
        "{}",
        r.body["report"]
    );
}

#[tokio::test]
async fn an_image_is_listed_under_assets_and_nothing_is_fetched() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", RICH, json!({})).await;
    let img = node_of(&r, "img");
    assert_eq!(img["type"], json!("rect"));
    assert_eq!(
        r.body["assets"],
        json!([{"node": img["id"], "use": "image", "src": "https://x.test/a.png"}])
    );
}

#[tokio::test]
async fn what_makes_no_node_and_what_no_rule_names_are_listed() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", RICH, json!({})).await;
    let skipped = r.body["skipped"].as_array().unwrap();
    let has = |code: &str, tag: &str| {
        skipped
            .iter()
            .any(|s| s["code"] == code && s["source"] == tag)
    };
    assert!(has("display_none", "div"), "{skipped:?}");
    assert!(has("element_skipped", "input"), "{skipped:?}");
    let entries = r.body["report"]["entries"].as_array().unwrap();
    assert!(
        entries.iter().any(|e| e["kind"] == "listed"
            && e["code"] == "class_unresolved"
            && e["name"] == "hero"),
        "{entries:?}"
    );
    // Nothing was lost by a class that no rule names: the page still has all its nodes.
    assert!(ids(&r).len() >= 4);
}

#[tokio::test]
async fn strict_fails_on_a_refusal_and_writes_nothing() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let r = import(&c, "k1", RICH, json!({"strict": true})).await;
    assert!(r.is_error, "{}", r.body);
    let e = &r.body["error"];
    assert_eq!(e["code"], json!("E_IMPORT_HTML_UNSUPPORTED"));
    assert!(
        e["refused"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["name"] == "line-height"),
        "{e}"
    );
    assert_eq!(head(&c).await, (0, 0));
    // The same text without strict is written, with the same refusals listed on its nodes.
    assert!(!import(&c, "k1", RICH, json!({})).await.is_error);
    // Strict on text with nothing refused is written.
    let clean = import(
        &c,
        "k2",
        "<body style='margin:0'><div style='background:#eee;padding:4px'>fine</div></body>",
        json!({"strict": true}),
    )
    .await;
    assert!(!clean.is_error, "{}", clean.body);
}

#[tokio::test]
async fn the_limits_and_an_empty_text_are_refused_before_anything_is_written() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let one = "<div style='width:10px;height:10px;background:#eee'></div>";
    let cases: [(String, &str); 4] = [
        ("<i>x</i>".repeat(40_000), "E_IMPORT_HTML_TOO_LARGE"),
        (
            format!("<div id=r>{}</div>", one.repeat(120)),
            "E_IMPORT_HTML_TOO_MANY",
        ),
        ("<i></i>".repeat(5_001), "E_IMPORT_HTML_TOO_MANY_ELEMENTS"),
        ("<script>1</script>".into(), "E_IMPORT_HTML_EMPTY"),
    ];
    for (html, want) in cases {
        let r = import(&c, "k1", &html, json!({})).await;
        assert!(r.is_error, "{want}: {}", r.body);
        assert_eq!(r.body["error"]["code"], json!(want), "{}", r.body);
        assert_eq!(r.body["error"]["retryable"], json!(false));
    }
    assert_eq!(head(&c).await, (0, 0));
}

#[tokio::test]
async fn a_call_with_bad_arguments_is_refused_with_the_code_of_its_kind() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let cases = [
        (
            json!({"file": "f", "html": "<p>x</p>", "idempotencyKey": "k", "ifSeq": 1}),
            "bad_request",
        ),
        (
            json!({"file": "f", "html": "<p>x</p>", "idempotencyKey": "k", "width": "wide"}),
            "bad_request",
        ),
        (json!({"file": "f", "html": "<p>x</p>"}), "invalid"),
        (
            json!({"file": "a/b", "html": "<p>x</p>", "idempotencyKey": "k"}),
            "invalid",
        ),
        (
            json!({"file": "f", "html": "<p>x</p>", "idempotencyKey": "k", "width": -3}),
            "E_OUT_OF_RANGE",
        ),
        (
            json!({"file": "f", "html": "<p>x</p>", "idempotencyKey": "k", "name": "a\u{202e}b"}),
            "invalid",
        ),
    ];
    for (args, want) in cases {
        let r = call(&c, "import_html", args.clone()).await;
        assert!(r.is_error, "{args}");
        assert_eq!(r.body["error"]["code"], json!(want), "{args}: {}", r.body);
    }
    assert_eq!(head(&c).await, (0, 0));
}
