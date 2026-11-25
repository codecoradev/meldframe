//! What an agent reads: the entry of `docs/mcp-tools.json` is what the server lists, the codes are in
//! `docs/error-codes.md`, and the tool text and the skill say what the note requires them to say
//! (`docs/import-html.md` sections 2.2, 2.3 and 4.3).

mod common;
mod harness;
use harness::*;
use meldframe_http_html_import::{DESCRIPTION, MAX_IMPORT_KEY, TOOL, input_schema};
use serde_json::{Value, json};

const CATALOGUE: &str = include_str!("../../../../docs/mcp-tools.json");
const CODES: &str = include_str!("../../../../docs/error-codes.md");
const SKILL: &str = include_str!("../../../../skills/meldframe/SKILL.md");
const NEW_CODES: [&str; 5] = [
    "E_IMPORT_HTML_TOO_LARGE",
    "E_IMPORT_HTML_TOO_MANY",
    "E_IMPORT_HTML_TOO_MANY_ELEMENTS",
    "E_IMPORT_HTML_EMPTY",
    "E_IMPORT_HTML_UNSUPPORTED",
];

fn entry() -> Value {
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    let list = file["tools"].as_array().unwrap();
    list.iter()
        .find(|t| t["name"] == TOOL)
        .expect("import_html is in docs/mcp-tools.json")
        .clone()
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[tokio::test]
async fn the_catalogue_entry_is_what_a_client_lists() {
    let w = start().await;
    let c = connect(&w.h, AGENT).await;
    let live = c.list_all_tools().await.unwrap();
    let live = live
        .iter()
        .find(|t| t.name == TOOL)
        .expect("the tool is listed");
    let e = entry();
    assert_eq!(e["tier"], json!("change"));
    assert_eq!(e["description"], live.description.as_deref().unwrap());
    assert_eq!(
        e["inputSchema"],
        serde_json::to_value(&live.input_schema).unwrap()
    );
    assert_eq!(e["description"], json!(DESCRIPTION));
    assert_eq!(e["inputSchema"], input_schema());
    assert_eq!(
        live.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(false)
    );
    let file: Value = serde_json::from_str(CATALOGUE).unwrap();
    assert_eq!(file["batchLimit"], json!(100));
}

#[test]
fn every_code_of_the_tool_has_its_row_and_the_tool_text_names_it() {
    for code in NEW_CODES {
        assert!(
            CODES.contains(&format!("| `{code}` |")),
            "{code} is in docs/error-codes.md"
        );
        assert!(DESCRIPTION.contains(code), "the tool text names {code}");
    }
    assert_eq!(CODES.matches("| `E_IMPORT_HTML_").count(), NEW_CODES.len());
}

#[test]
fn the_tool_text_says_what_the_note_requires() {
    let t = flat(DESCRIPTION);
    for phrase in [
        // Nothing is followed, and the agent resolves what a browser would.
        "<link>",
        "@import",
        "url()",
        "src is never followed",
        "put their rules in css",
        // The Tailwind export imports back faithfully only with the css it returns.
        "only together with the css it returns",
        // The recipe for an agent with a browser is a suggestion and has not been measured.
        "already computed",
        "a suggestion",
        "has not been measured",
        // The limits, and what an agent does at each.
        "262144 bytes",
        "at most 100 commands",
        "one section at a time",
        "parentId",
        // No ifSeq, no undo.
        "no ifSeq",
        "ask the person to revert the batch",
        "dryRun true",
        "strict true",
        "idempotencyKey",
    ] {
        assert!(t.contains(phrase), "the tool text says `{phrase}`");
    }
    assert!(
        t.contains(&format!("1 to {MAX_IMPORT_KEY} characters")),
        "the key limit in the text is the code's"
    );
    assert!(DESCRIPTION.is_ascii());
    assert!(!DESCRIPTION.contains('\u{2014}') && !DESCRIPTION.contains('\u{2013}'));
}

#[test]
fn the_skill_tells_an_agent_how_to_import_html() {
    // The section on importing, not the whole skill: `upload_asset` and `export_frame` have sections of their own.
    let all = flat(SKILL);
    let start = all
        .find("## Pulling a page of code onto the canvas")
        .expect("a section on import_html");
    let s = &all[start..];
    let s = &s[..s[3..].find("## ").map_or(s.len(), |i| i + 3)];
    for phrase in [
        "`import_html`",
        "`css`",
        "`html`",
        "`idempotencyKey`",
        "one section at a time",
        "`dryRun`",
        "`strict`",
        "`unsupported`",
        "`assets`",
        "`upload_asset`",
        "`parentId`",
        "`export_frame`",
        "`E_IMPORT_HTML_TOO_MANY`",
        "`E_IMPORT_HTML_UNSUPPORTED`",
        "revert the batch",
        "already computed",
        "has not been measured",
        "never followed",
        "only together with the `css` it returned",
    ] {
        assert!(s.contains(phrase), "the section says {phrase}");
    }
}
