//! `import_tokens` over MCP, the edges of a long import: other text under the same key, the index of a refusal, the
//! policy, the part of a dry run, the quota, and the text of the tool.

mod importing;
use importing::*;
use meldframe_module::Policy;
use serde_json::json;

#[tokio::test]
async fn the_same_key_with_other_css_never_replays_part_one_and_writes_part_two() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let first = long(250);
    // The same first 100 commands, other variables after them.
    let other = first.replace("--v200: 200px", "--v200: 999px");
    assert_ne!(first, other);
    assert!(!import(&c, "k", &first, json!({"part": 1})).await.is_error);
    let r = import(&c, "k", &other, json!({"part": 1})).await;
    assert!(
        r.is_error,
        "part 1 of the other text is not a replay: {}",
        r.body
    );
    assert_eq!(r.body["error"]["code"], "E_DUPLICATE_ID");
    assert_eq!(
        variables(&c, json!({})).await["total"],
        99,
        "nothing of part 2 was written from the other text"
    );
}

#[tokio::test]
async fn a_refused_part_names_the_command_over_the_whole_import_and_the_hint_says_how_to_finish() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let css = long(250);
    assert!(!import(&c, "k", &css, json!({"part": 1})).await.is_error);
    // A variable with the id of `v150`, which is the command at index 136 of the import (the plan puts an alias after its target, so the order is not the text order).
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "squat", "commands": [
            {"type": "addVariable", "variable": {"id": "var-root-v150", "collection": "col-root", "name": "squatter",
                "type": "number", "values": {"light": 1, "dark": 1}}}]}),
    )
    .await;
    assert!(r.is_error, "the root collection has one mode: {}", r.body);
    let r = call(
        &c,
        "apply_commands",
        json!({"file": "f", "idempotencyKey": "squat2", "commands": [
            {"type": "addVariable", "variable": {"id": "var-root-v150", "collection": "col-root", "name": "squatter",
                "type": "number", "values": {"light": 1}}}]}),
    )
    .await;
    assert!(!r.is_error, "{}", r.body);
    let part2 = import(&c, "k", &css, json!({"part": 2})).await;
    assert!(part2.is_error, "{}", part2.body);
    assert_eq!(part2.body["error"]["code"], "E_DUPLICATE_ID");
    assert_eq!(part2.body["error"]["index"], 136, "{}", part2.body);
    let msg = part2.body["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("same idempotencyKey") && msg.contains("apply_commands"),
        "{msg}"
    );
    assert!(!msg.contains("cannot run twice"), "{msg}");
}

#[tokio::test]
async fn only_an_auto_policy_serves_the_tool() {
    for policy in [Some(Policy::Ask), Some(Policy::Deny), None] {
        let h = start_with(policy).await;
        let c = connect(&h, AGENT).await;
        let r = import(&c, "k", THEME, json!({})).await;
        assert_eq!(r.body["error"]["code"], "forbidden", "{policy:?}");
        assert_eq!(variables(&c, json!({})).await["total"], 0);
    }
}

#[tokio::test]
async fn a_dry_run_checks_the_part_too() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    for part in [0, 9] {
        let r = call(
            &c,
            "import_tokens",
            json!({"file": "f", "css": THEME, "dryRun": true, "part": part}),
        )
        .await;
        assert_eq!(
            r.body["error"]["code"], "invalid",
            "part {part}: {}",
            r.body
        );
    }
    let ok = call(
        &c,
        "import_tokens",
        json!({"file": "f", "css": THEME, "dryRun": true, "part": 1}),
    )
    .await;
    assert!(!ok.is_error, "{}", ok.body);
}

#[tokio::test]
async fn a_quota_refusal_in_the_middle_of_an_import_leaves_the_parts_before_it() {
    // 150 ops a minute: part 1 (100 commands) lands, part 2 would be the 200th and is refused whole.
    let h = start_quota(Some(Policy::Auto), Some(150)).await;
    let c = connect(&h, AGENT).await;
    let css = long(250);
    let one = import(&c, "k", &css, json!({"part": 1})).await;
    assert!(!one.is_error, "{}", one.body);
    let two = import(&c, "k", &css, json!({"part": 2})).await;
    assert!(two.is_error, "{}", two.body);
    assert_eq!(two.body["error"]["code"], "quota_ops_per_minute");
    let v = variables(&c, json!({})).await;
    assert_eq!(
        v["total"], 99,
        "part 1 landed (the collection and 99 variables), part 2 wrote nothing"
    );
    let again = import(&c, "k", &css, json!({"part": 1})).await;
    assert_eq!(
        again.body["replayed"], true,
        "a replay costs nothing and the parts that landed are known"
    );
}

#[tokio::test]
async fn the_text_of_the_tool_names_the_quota_and_the_bytes() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let tools = c.list_all_tools().await.unwrap();
    let t = tools.iter().find(|t| t.name == "import_tokens").unwrap();
    let text = t.description.as_deref().unwrap();
    assert!(
        text.contains("6,000") && text.contains("counted in bytes"),
        "{text}"
    );
}

#[tokio::test]
async fn the_whole_report_comes_with_part_one_and_only_the_counts_with_the_later_parts() {
    let h = start().await;
    let c = connect(&h, AGENT).await;
    let css = long(150).replace("}", "--bad: calc(1px + 1px); }");
    let one = import(&c, "k", &css, json!({"part": 1})).await;
    assert!(!one.is_error, "{}", one.body);
    assert_eq!(one.body["report"]["entries"].as_array().unwrap().len(), 1);
    let two = import(&c, "k", &css, json!({"part": 2})).await;
    assert!(!two.is_error, "{}", two.body);
    assert_eq!(
        two.body["report"]["variables"],
        one.body["report"]["variables"]
    );
    assert!(
        two.body["report"]["entries"].is_null(),
        "{}",
        two.body["report"]
    );
    assert_eq!(two.body["report"]["seeEntriesInPart"], 1);
    let dry = call(
        &c,
        "import_tokens",
        json!({"file": "f", "css": css, "dryRun": true, "part": 2}),
    )
    .await;
    assert_eq!(
        dry.body["report"]["entries"].as_array().unwrap().len(),
        1,
        "a dry run has the whole report"
    );
    let tools = c.list_all_tools().await.unwrap();
    let t = tools.iter().find(|t| t.name == "import_tokens").unwrap();
    let text = t.description.as_deref().unwrap();
    assert!(
        text.contains("carry only its counts") && text.contains("32 KiB of text in the entries"),
        "{text}"
    );
}
